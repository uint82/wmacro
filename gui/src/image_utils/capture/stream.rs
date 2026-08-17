//! the PipeWire capture stream: format negotiation, frame readback, and the serving mainloop.

use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow};
use pipewire::buffer::Buffer as PwBuffer;
use pipewire::context::ContextBox;
use pipewire::keys;
use pipewire::main_loop::MainLoopBox;
use pipewire::properties::properties;
use pipewire::spa::buffer::DataType;
use pipewire::spa::param::ParamType;
use pipewire::spa::param::format::{FormatProperties, MediaSubtype, MediaType};
use pipewire::spa::param::video::{VideoFormat, VideoInfoRaw};
use pipewire::spa::pod::serialize::PodSerializer;
use pipewire::spa::pod::{ChoiceValue, Pod, Property, PropertyFlags, Value, object, property};
use pipewire::spa::support::system::IoFlags;
use pipewire::spa::utils::{
    Choice, ChoiceEnum, ChoiceFlags, Direction, Fraction, Rectangle, SpaTypes,
};
use pipewire::stream::{StreamBox, StreamFlags, StreamState};

use super::gbm::GbmSession;
use super::pixel::PixelFormat;
use super::state::{
    CaptureRequest, CaptureState, request_coverable, request_union, serve_requests,
};

/// AMD 64K-tiling DRM modifiers (gfx9/gfx10 codes) used by wlroots-based portals; LINEAR is always included.
const AMD_MODIFIERS: [i64; 20] = [
    0x1000000000000000, // GFX9_64K_R_X
    0x1000000000000001, // GFX9_64K_S_X
    0x1000000000000002, // GFX9_64K_S_Y
    0x1000000000000003, // GFX9_64K_S_XY
    0x1000000000000004, // GFX9_64K_S_XY_2X1_V
    0x1000000000000005, // GFX9_64K_S_XY_2X1_V_XY
    0x1000000000000006, // GFX9_64K_D_XY
    0x1000000000000007, // GFX9_64K_R_XY_2X1_V
    0x1000000000000008, // GFX9_64K_R_XY_2X1_V_XY
    0x1000000000000009, // GFX9_64K_D_XY_2X1_V
    0x100000000000000a, // GFX9_64K_D_XY_2X1_V_XY
    0x100000000000000b, // GFX10_64K_R_X
    0x100000000000000c, // GFX10_64K_S_X
    0x100000000000000d, // GFX10_64K_S_Y
    0x100000000000000e, // GFX10_64K_S_XY
    0x100000000000000f, // GFX10_64K_S_XY_2X1_V
    0x1000000000000010, // GFX10_64K_S_XY_2X1_V_XY
    0x1000000000000011, // GFX10_64K_D_XY
    0x1000000000000012, // GFX10_64K_R_XY_2X1_V
    0x1000000000000013, // GFX10_64K_R_XY_2X1_V_XY
];

/// video.modifier: non-fixated choice (LINEAR default, then AMD 64K). not
/// mandatory: a mandatory filter property missing in the peer param rejects it.
fn modifier_property() -> Property {
    let mut alternatives = vec![0i64]; // LINEAR
    alternatives.extend_from_slice(&AMD_MODIFIERS);
    Property {
        key: FormatProperties::VideoModifier.as_raw(),
        flags: PropertyFlags::DONT_FIXATE,
        value: Value::Choice(ChoiceValue::Long(Choice(
            ChoiceFlags::empty(),
            ChoiceEnum::Enum {
                default: 0, // LINEAR
                alternatives,
            },
        ))),
    }
}

/// one EnumFormat param per format: PipeWire 1.6.x portals reject a single
/// param advertising multiple formats via a Choice (as FFmpeg's pw grab does).
fn build_format_params() -> Result<Vec<Vec<u8>>> {
    const FORMATS: [VideoFormat; 6] = [
        VideoFormat::BGRA,
        VideoFormat::BGRx,
        VideoFormat::RGBA,
        VideoFormat::RGBx,
        VideoFormat::ARGB,
        VideoFormat::ABGR,
    ];
    let mut params = Vec::with_capacity(FORMATS.len());
    for fmt in FORMATS {
        let obj = object!(
            SpaTypes::ObjectParamFormat,
            ParamType::EnumFormat,
            property!(FormatProperties::MediaType, Id, MediaType::Video),
            property!(FormatProperties::MediaSubtype, Id, MediaSubtype::Raw),
            property!(FormatProperties::VideoFormat, Id, fmt),
            property!(
                FormatProperties::VideoSize,
                Choice,
                Range,
                Rectangle,
                Rectangle {
                    width: 1920,
                    height: 1080
                },
                Rectangle {
                    width: 1,
                    height: 1
                },
                Rectangle {
                    width: 16384,
                    height: 16384
                }
            ),
            property!(
                FormatProperties::VideoFramerate,
                Choice,
                Range,
                Fraction,
                Fraction { num: 60, denom: 1 },
                Fraction { num: 0, denom: 1 },
                Fraction { num: 144, denom: 1 }
            ),
            modifier_property(),
        );
        let values =
            PodSerializer::serialize(std::io::Cursor::new(Vec::new()), &Value::Object(obj))
                .map_err(|e| anyhow!("failed to serialize format pod: {e}"))?
                .0
                .into_inner();
        params.push(values);
    }
    Ok(params)
}

/// consumes one stream buffer: reads the pending requests' bounding box into
/// `s.retained` and bumps the frame sequence.
fn read_buffer(gbm: &GbmSession, buffer: &mut PwBuffer<'_>, s: &mut CaptureState) -> bool {
    let Some((fw, fh)) = s.frame else {
        return false;
    };
    let Some(fmt) = s.format else { return false };
    let Some(data) = buffer.datas_mut().first_mut() else {
        return false;
    };

    let chunk_size = data.chunk().size();
    if chunk_size == 0 {
        return false;
    }
    let stride = if data.chunk().stride() > 0 {
        data.chunk().stride() as usize
    } else {
        fw as usize * 4
    };
    let offset = data.chunk().offset();

    // a `full` request reads the whole frame; in continuous mode an empty
    // pending set re-reads the last used region instead of the whole screen.
    let region = if s.pending.iter().any(|r| r.full) {
        None
    } else {
        request_union(s, fw, fh).or({
            if s.continuous && s.retained_w > 0 {
                Some((
                    s.retained_origin.0,
                    s.retained_origin.1,
                    s.retained_w,
                    s.retained_h,
                ))
            } else {
                None
            }
        })
    };
    let read_start = Instant::now();
    let ok = match data.type_() {
        DataType::DmaBuf => {
            let raw_fd = data.fd();
            if raw_fd < 0 {
                false
            } else {
                gbm.read_frame(
                    raw_fd,
                    fmt.fourcc(),
                    fw,
                    fh,
                    stride as u32,
                    offset,
                    s.modifier,
                    region,
                    &mut s.retained,
                )
                .map_err(|e| log::warn!("frame readback failed: {e:#}"))
                .is_ok()
            }
        }
        DataType::MemPtr => match data.data() {
            Some(d) => {
                let (rx0, ry0, rw, rh) = region.unwrap_or((0, 0, fw, fh));
                s.retained.clear();
                for row in 0..rh {
                    let src_off = (ry0 + row) as usize * stride + rx0 as usize * 4;
                    let need = rw as usize * 4;
                    if src_off + need > d.len() {
                        break;
                    }
                    s.retained.extend_from_slice(&d[src_off..src_off + need]);
                }
                true
            }
            None => false,
        },
        _ => false,
    };

    if ok {
        match region {
            Some((x0, y0, rw, rh)) => {
                s.retained_origin = (x0, y0);
                s.retained_stride = rw as usize * 4;
                s.retained_w = rw;
                s.retained_h = rh;
            }
            None => {
                s.retained_origin = (0, 0);
                s.retained_stride = fw as usize * 4;
                s.retained_w = fw;
                s.retained_h = fh;
            }
        }
        s.retained_fmt = fmt;
        s.seq += 1;
        s.retained_seq = s.seq;
        s.retained_at = Some(Instant::now());
        // continuous-mode idle reads run on every frame; keep them quiet.
        if s.pending.is_empty() {
            log::trace!(
                "frame readback took {:.3}ms (region {}x{}, continuous idle)",
                read_start.elapsed().as_secs_f64() * 1000.0,
                s.retained_w,
                s.retained_h
            );
        } else {
            log::debug!(
                "frame readback took {:.3}ms (region {}x{})",
                read_start.elapsed().as_secs_f64() * 1000.0,
                s.retained_w,
                s.retained_h
            );
        }
    }
    ok
}

/// serves coverable pending requests, at most once per retained frame
/// (`last_served_seq`); the rest stay pending for the next read's union.
fn serve_ready(state: &Arc<Mutex<CaptureState>>) -> bool {
    let mut s = state.lock().unwrap();
    if s.pending.is_empty() || s.retained_seq == 0 || s.retained_seq <= s.last_served_seq {
        return false;
    }
    let pending = std::mem::take(&mut s.pending);
    let mut servable = Vec::with_capacity(pending.len());
    let mut keep = Vec::with_capacity(pending.len());
    for req in pending {
        if request_coverable(&s, &req) {
            servable.push(req);
        } else {
            keep.push(req);
        }
    }
    if servable.is_empty() {
        s.pending = keep;
        return false;
    }
    s.pending = keep;
    s.last_served_seq = s.retained_seq;
    drop(s);
    serve_requests(state, servable);
    true
}

pub(super) fn pw_capture_thread(
    fd: OwnedFd,
    gbm: GbmSession,
    node_id: u32,
    rx: mpsc::Receiver<CaptureRequest>,
    wake_rfd: RawFd,
    stop: Arc<AtomicBool>,
    state: Arc<Mutex<CaptureState>>,
) -> Result<()> {
    let mainloop = MainLoopBox::new(None)?;
    let context = ContextBox::new(mainloop.loop_(), None)?;
    let core = context.connect_fd(fd, None)?;

    // self-pipe so new requests wake the loop immediately; the write end lives in `Capturer`.
    let wake_fd = unsafe { OwnedFd::from_raw_fd(wake_rfd) };
    let _wake_source = mainloop.loop_().add_io(wake_fd, IoFlags::IN, |fd| {
        let mut byte = [0u8; 1];
        while unsafe { libc::read(fd.as_raw_fd(), byte.as_mut_ptr() as *mut _, 1) } > 0 {}
    });

    let stream = StreamBox::new(
        &core,
        "wmacro-capture",
        properties! {
            *keys::MEDIA_TYPE => "Video",
            *keys::MEDIA_CATEGORY => "Capture",
            *keys::NODE_NAME => "wmacro-capture",
        },
    )?;

    let listener_state = state.clone();
    let error_state = state.clone();
    let process_state = state.clone();
    let process_gbm = Arc::new(gbm);
    let callback_gbm = process_gbm.clone();
    let _listener = stream
        .add_local_listener_with_user_data(state.clone())
        .state_changed(move |_, _, _old, new| {
            if let StreamState::Error(msg) = new {
                log::error!("capture stream error: {msg}");
                if let Ok(mut s) = error_state.lock() {
                    s.stream_error = Some(msg.to_string());
                }
            }
        })
        .param_changed(move |_, _, id, param| {
            if id != ParamType::Format.as_raw() {
                return;
            }
            let Some(param) = param else { return };
            let mut info = VideoInfoRaw::default();
            if info.parse(param).is_err() {
                return;
            }
            let mut s = listener_state.lock().unwrap();
            if let Some(fmt) = PixelFormat::from_fourcc(info.format().as_raw()) {
                s.format = Some(fmt);
            }
            let size = info.size();
            if size.width > 0 && size.height > 0 {
                s.frame = Some((size.width, size.height));
            }
            s.modifier = info.modifier();
            log::info!(
                "capture negotiated format {:x} {}x{} modifier {:#x}",
                info.format().as_raw(),
                size.width,
                size.height,
                s.modifier
            );
        })
        .process(move |stream, _| {
            // always consume and release, even with no pending requests, so
            // the portal keeps producing and a fresh frame stays in flight.
            // while `continuous` is set, every frame is read into the retained buffer.
            let mut s = process_state.lock().unwrap();
            match stream.dequeue_buffer() {
                Some(mut buffer) if !s.pending.is_empty() || s.continuous => {
                    read_buffer(&callback_gbm, &mut buffer, &mut s);
                }
                Some(_) => {}
                None => {}
            }
        })
        .register()?;

    let format_bytes = build_format_params()?;
    let mut params: Vec<&Pod> = format_bytes
        .iter()
        .map(|bytes| Pod::from_bytes(bytes).context("failed to parse format pod"))
        .collect::<Result<_>>()?;
    stream.connect(
        Direction::Input,
        Some(node_id),
        StreamFlags::AUTOCONNECT | StreamFlags::MAP_BUFFERS,
        &mut params,
    )?;
    stream.set_active(true)?;
    log::info!("capture stream connected");

    while !stop.load(Ordering::Relaxed) {
        let stream_error = state.lock().unwrap().stream_error.clone();
        if let Some(err) = stream_error {
            let pending = {
                let mut s = state.lock().unwrap();
                let reqs = std::mem::take(&mut s.pending);
                log::error!(
                    "capture stream failed, dropping {} pending request(s): {err}",
                    reqs.len()
                );
                reqs
            };
            for req in pending {
                let _ = req
                    .reply
                    .try_send(Err(format!("capture stream failed: {err}")));
            }
            break;
        }
        while let Ok(req) = rx.try_recv() {
            state.lock().unwrap().pending.push(req);
        }

        // serve pass: 1. the retained frame is newer than the last serve, so
        // answer what it covers; unanswerable requests stay pending, since
        // serving the stale retained frame after idle would return stale
        // content. 2. otherwise a frame may already be queued with no
        // `process` callback firing, so consume it directly instead of the poll.
        if !serve_ready(&state) {
            let mut s = state.lock().unwrap();
            if !s.pending.is_empty()
                && s.retained_seq == s.last_served_seq
                && let Some(mut buffer) = stream.dequeue_buffer()
            {
                let ok = read_buffer(&process_gbm, &mut buffer, &mut s);
                log::debug!("capture: consumed an already-queued frame (ok={ok})");
                drop(s);
                if ok {
                    serve_ready(&state);
                }
            }
        }

        // new requests wake the loop instantly via the self-pipe; this timeout only bounds idle servicing.
        mainloop.loop_().iterate(Duration::from_millis(5));
    }

    log::info!("capture thread shutting down");
    Ok(())
}
