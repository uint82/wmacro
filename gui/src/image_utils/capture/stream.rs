//! the PipeWire capture stream: format negotiation, the `process` callback
//! that reads frames on arrival, and the mainloop thread that serves requests.

use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use pipewire::buffer::Buffer as PwBuffer;
use pipewire::context::ContextBox;
use pipewire::keys;
use pipewire::main_loop::MainLoopBox;
use pipewire::properties::properties;
use pipewire::spa::buffer::DataType;
use pipewire::spa::param::format::{FormatProperties, MediaSubtype, MediaType};
use pipewire::spa::param::video::{VideoFormat, VideoInfoRaw};
use pipewire::spa::param::ParamType;
use pipewire::spa::pod::serialize::PodSerializer;
use pipewire::spa::pod::{object, property, ChoiceValue, Pod, Property, PropertyFlags, Value};
use pipewire::spa::support::system::IoFlags;
use pipewire::spa::utils::{
    Choice, ChoiceEnum, ChoiceFlags, Direction, Fraction, Rectangle, SpaTypes,
};
use pipewire::stream::{StreamBox, StreamFlags, StreamState};

use super::gbm::GbmSession;
use super::pixel::PixelFormat;
use super::state::{request_coverable, request_union, serve_requests, CaptureRequest, CaptureState};

/// AMD 64K-tiling DRM modifiers (DRM_FORMAT_MOD_AMD_64K | gfx9/gfx10 codes),
/// commonly used by wlroots-based portals; LINEAR is always included.
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

/// video.modifier property: a non-fixated choice, LINEAR default followed by
/// the common AMD 64K modifiers. not marked mandatory: on renegotiation the
/// portal may advertise a format without this property (SHM fallback), and a
/// mandatory filter property that is missing in the peer param rejects it.
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

/// builds one EnumFormat param per supported format. PipeWire 1.6.x portals
/// reject a single param advertising multiple formats via a Choice, so every
/// format is sent as its own fixed-format param (as FFmpeg's pw grab does).
/// the video.modifier + framerate properties are required by dmabuf portals.
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
            property!(
                FormatProperties::MediaType,
                Id,
                MediaType::Video
            ),
            property!(
                FormatProperties::MediaSubtype,
                Id,
                MediaSubtype::Raw
            ),
            property!(
                FormatProperties::VideoFormat,
                Id,
                fmt
            ),
            property!(
                FormatProperties::VideoSize,
                Choice,
                Range,
                Rectangle,
                Rectangle { width: 1920, height: 1080 },
                Rectangle { width: 1, height: 1 },
                Rectangle { width: 16384, height: 16384 }
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
        let values = PodSerializer::serialize(std::io::Cursor::new(Vec::new()), &Value::Object(obj))
            .map_err(|e| anyhow!("failed to serialize format pod: {e}"))?
            .0
            .into_inner();
        params.push(values);
    }
    Ok(params)
}

/// consumes one stream buffer: reads the bounding box of all pending requests
/// into `s.retained` and bumps the frame sequence. returns whether the frame
/// was captured.
fn read_buffer(gbm: &GbmSession, buffer: &mut PwBuffer<'_>, s: &mut CaptureState) -> bool {
    let Some((fw, fh)) = s.frame else { return false };
    let Some(fmt) = s.format else { return false };
    let Some(data) = buffer.datas_mut().first_mut() else { return false };

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

    let region = request_union(s, fw, fh);
    let read_start = Instant::now();
    let ok = match data.type_() {
        DataType::DmaBuf => {
            let raw_fd = data.fd();
            if raw_fd < 0 {
                false
            } else {
                gbm.read_frame(raw_fd, fmt.fourcc(), fw, fh, stride as u32, offset, s.modifier, region, &mut s.retained)
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
        log::debug!(
            "frame readback took {:.3}ms (region {}x{})",
            read_start.elapsed().as_secs_f64() * 1000.0,
            s.retained_w,
            s.retained_h
        );
    }
    ok
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

    // self-pipe so new requests wake the loop immediately instead of waiting
    // out the poll timeout below. the write end lives in `Capturer`.
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
            // always consume and release, even with no pending requests: the
            // portal produces frames in response to releases, so this keeps
            // the stream warm and a fresh frame in flight for the next
            // request (no per-request round trip when idle).
            let mut s = process_state.lock().unwrap();
            match stream.dequeue_buffer() {
                Some(mut buffer) if !s.pending.is_empty() => {
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
                log::error!("capture stream failed, dropping {} pending request(s): {err}", reqs.len());
                reqs
            };
            for req in pending {
                let _ = req.reply.try_send(Err(format!("capture stream failed: {err}")));
            }
            break;
        }
        while let Ok(req) = rx.try_recv() {
            state.lock().unwrap().pending.push(req);
        }
        let to_serve = {
            let mut s = state.lock().unwrap();
            if s.pending.is_empty() || s.retained_seq == 0 {
                None
            } else if s.retained_seq > s.last_served_seq {
                // a fresh frame arrived since the last serve. only requests
                // the retained buffer can answer are served now; the rest
                // stay pending until a frame covers them. every request
                // waits for a new frame: serving the retained frame after
                // idle would keep returning stale screen content.
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
                s.pending = keep;
                s.last_served_seq = s.retained_seq;
                Some(servable)
            } else {
                None
            }
        };
        if let Some(reqs) = to_serve.filter(|reqs| !reqs.is_empty()) {
            serve_requests(&state, reqs);
        }
        // a pending request that the retained buffer cannot answer needs a
        // fresher frame. the stream idles (with a buffer queued) when nothing
        // was consumed for a while and no `process` callback would fire, so
        // consume any queued buffer directly from the mainloop.
        let _ = {
            let mut s = state.lock().unwrap();
            if !s.pending.is_empty() && s.retained_seq == s.last_served_seq {
                if let Some(mut buffer) = stream.dequeue_buffer() {
                    let ok = read_buffer(&process_gbm, &mut buffer, &mut s);
                    log::debug!("capture: consumed an already-queued frame (ok={ok})");
                    ok
                } else {
                    false
                }
            } else {
                false
            }
        };
        // new requests wake the loop instantly via the self-pipe; this timeout
        // only bounds how often the stream is serviced while idle.
        mainloop.loop_().iterate(Duration::from_millis(5));
    }

    log::info!("capture thread shutting down");
    Ok(())
}
