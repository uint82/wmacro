mod blob;
mod gbm;
mod pixel;
mod portal;
mod state;
mod stream;

#[cfg(test)]
mod tests;

use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, mpsc};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use ashpd::desktop::screencast::{
    CursorMode, Screencast, SelectSourcesOptions, SourceType, StartCastOptions,
};
use ashpd::desktop::{PersistMode, Session};
use image::{DynamicImage, GrayImage, RgbaImage};
use once_cell::sync::Lazy;

use super::outputs::{OutputInfo, query_outputs};

use gbm::GbmSession;
use portal::{load_restore_token, match_output, save_restore_token};
use state::CaptureState;
use stream::pw_capture_thread;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const RETRY_COOLDOWN: Duration = Duration::from_secs(10);
const PORTAL_TIMEOUT: Duration = Duration::from_secs(20);

/// tile captured around a pixel that misses the retained fast path, and the retained-frame freshness bound.
const PIXEL_TILE_SIZE: i32 = 32;
const PIXEL_TILE_MARGIN: i32 = PIXEL_TILE_SIZE / 2;
const PIXEL_FRESHNESS_TTL: Duration = Duration::from_millis(100);

pub(crate) type ScreenPos = (i32, i32);
pub(crate) type ScreenSize = (u32, u32);

pub struct Capturer {
    requests: mpsc::Sender<state::CaptureRequest>,

    wake: OwnedFd,

    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    session: Option<Session<Screencast>>,
    output: OutputInfo,

    /// shared with the capture thread; the pixel fast path reads it directly.
    state: Arc<Mutex<state::CaptureState>>,
}

impl Capturer {
    pub fn new() -> Result<Self> {
        let (session, fd, node_id, stream_pos, stream_size, mapping_id, output, gbm) = ASYNC_RT
            .block_on(async {
                let portal_flow = async {
                    let screencast = Screencast::new()
                        .await
                        .context("failed to connect to the ScreenCast portal")?;
                    let session = screencast
                        .create_session(Default::default())
                        .await
                        .context("failed to create a ScreenCast session")?;

                    let mut use_token = load_restore_token();
                    let select_request = loop {
                        let mut opts = SelectSourcesOptions::default()
                            .set_sources(Some(SourceType::Monitor.into()))
                            .set_multiple(false)
                            .set_cursor_mode(CursorMode::Embedded);
                        if let Some(token) = &use_token {
                            opts = opts.set_restore_token(token.as_str());
                        } else {
                            opts = opts.set_persist_mode(PersistMode::ExplicitlyRevoked);
                        }
                        match screencast.select_sources(&session, opts).await {
                            Ok(req) => break req,
                            Err(_) if use_token.is_some() => {
                                log::warn!(
                                    "stored restore token was rejected, falling back to a fresh session"
                                );
                                use_token = None;
                                save_restore_token(None);
                            }
                            Err(e) => return Err(e).context("failed to select screencast sources"),
                        }
                    };
                    drop(select_request);

                    let start_request = screencast
                        .start(&session, None, StartCastOptions::default())
                        .await
                        .context("failed to start the ScreenCast session")?;
                    let streams = start_request
                        .response()
                        .context("ScreenCast session was rejected (permission denied?)")?;
                    let stream = streams
                        .streams()
                        .first()
                        .cloned()
                        .context("portal returned no stream")?;
                    if let Some(token) = streams.restore_token() {
                        log::info!("saving screencast restore token for future sessions");
                        save_restore_token(Some(token));
                    } else if use_token.is_some() {
                        save_restore_token(None);
                    }

                    let fd = screencast
                        .open_pipe_wire_remote(&session, Default::default())
                        .await
                        .context("failed to open the PipeWire remote fd")?;

                    let outputs = query_outputs().context("failed to enumerate outputs")?;
                    let output = match_output(
                        &outputs,
                        stream.size().map(|(w, h)| (w.max(0) as u32, h.max(0) as u32)),
                        stream.position(),
                        stream.mapping_id().map(str::to_string).as_deref(),
                    )
                    .context("failed to map the stream to an output")?;
                    let gbm = GbmSession::open().context("failed to initialize gbm")?;

                    Ok::<_, anyhow::Error>((
                        session,
                        fd,
                        stream.pipe_wire_node_id(),
                        stream.position(),
                        stream.size().map(|(w, h)| (w.max(0) as u32, h.max(0) as u32)),
                        stream.mapping_id().map(str::to_string),
                        output,
                        gbm,
                    ))
                };
                match tokio::time::timeout(PORTAL_TIMEOUT, portal_flow).await {
                    Ok(result) => result,
                    Err(_) => bail!(
                        "timed out waiting for the ScreenCast portal to respond ({}s)",
                        PORTAL_TIMEOUT.as_secs()
                    ),
                }
            })?;
        log::info!(
            "portal stream: node_id={node_id} pos={stream_pos:?} size={stream_size:?} mapping_id={mapping_id:?}"
        );
        log::info!(
            "using output {:?} at {:?} size {:?}",
            output.name,
            output.pos,
            output.size
        );

        let state = Arc::new(Mutex::new(CaptureState {
            output: Some(output.clone()),
            ..Default::default()
        }));
        let (tx, rx) = mpsc::channel();
        let mut wake_pipe = [0i32; 2];
        if unsafe { libc::pipe2(wake_pipe.as_mut_ptr(), libc::O_NONBLOCK | libc::O_CLOEXEC) } != 0 {
            bail!("pipe2 failed: {}", std::io::Error::last_os_error());
        }
        let wake = unsafe { OwnedFd::from_raw_fd(wake_pipe[1]) };
        let wake_rfd = wake_pipe[0];
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = stop.clone();
        let thread_state = state.clone();
        let thread = std::thread::spawn(move || {
            if let Err(e) =
                pw_capture_thread(fd, gbm, node_id, rx, wake_rfd, thread_stop, thread_state)
            {
                log::error!("capture thread failed: {e:#}");
            }
        });

        Ok(Capturer {
            requests: tx,
            wake,
            stop,
            thread: Some(thread),
            session: Some(session),
            output,
            state,
        })
    }

    pub fn output(&self) -> &OutputInfo {
        &self.output
    }

    fn state(&self) -> &Arc<Mutex<state::CaptureState>> {
        &self.state
    }

    pub fn is_dead(&self) -> bool {
        self.thread
            .as_ref()
            .is_none_or(|handle| handle.is_finished())
    }

    fn request_region(
        &self,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        full: bool,
        color: bool,
    ) -> Result<DynamicImage> {
        let (tx, rx) = mpsc::sync_channel(1);
        self.requests
            .send(state::CaptureRequest {
                x,
                y,
                w,
                h,
                full,
                color,
                reply: tx,
            })
            .context("capture thread is not running")?;
        let byte = [1u8];
        unsafe {
            libc::write(
                self.wake.as_raw_fd(),
                byte.as_ptr() as *const libc::c_void,
                1,
            );
        }
        match rx.recv_timeout(REQUEST_TIMEOUT) {
            Ok(Ok(img)) => Ok(img),
            Ok(Err(e)) => bail!("capture failed: {e}"),
            Err(_) => bail!("timed out waiting for a frame from the capture thread"),
        }
    }

    pub fn capture_region(&self, x: i32, y: i32, w: i32, h: i32) -> Result<GrayImage> {
        Ok(self.request_region(x, y, w, h, false, false)?.into_luma8())
    }

    pub fn capture_all(&self) -> Result<GrayImage> {
        Ok(self.request_region(0, 0, 0, 0, true, false)?.into_luma8())
    }

    /// color (RGBA) copy of a region, used by the if-pixel-color feature.
    pub fn capture_region_color(&self, x: i32, y: i32, w: i32, h: i32) -> Result<RgbaImage> {
        Ok(self.request_region(x, y, w, h, false, true)?.into_rgba8())
    }
}

impl Drop for Capturer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
        if let Some(session) = self.session.take() {
            ASYNC_RT.block_on(async {
                let _ = session.close().await;
            });
        }
    }
}

static ASYNC_RT: Lazy<tokio::runtime::Runtime> = Lazy::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("failed to build the async runtime")
});

struct GlobalBackend {
    capturer: Option<Capturer>,
    last_attempt: Option<Instant>,
}

static GLOBAL_CAPTURER: Lazy<Mutex<GlobalBackend>> = Lazy::new(|| {
    Mutex::new(GlobalBackend {
        capturer: None,
        last_attempt: None,
    })
});

fn global_capturer() -> Result<MutexGuard<'static, GlobalBackend>> {
    let mut g = GLOBAL_CAPTURER
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let cooldown_ok = g.last_attempt.is_none_or(|t| t.elapsed() > RETRY_COOLDOWN);
    if g.capturer.as_ref().is_some_and(|c| c.is_dead()) {
        log::warn!("capture backend thread has exited; restarting");
        g.capturer = None;
    }
    if g.capturer.is_none() && cooldown_ok {
        g.last_attempt = Some(Instant::now());
        match std::panic::catch_unwind(Capturer::new) {
            Ok(Ok(c)) => {
                log::info!("capture backend started for output {:?}", c.output());
                g.capturer = Some(c);
            }
            Ok(Err(e)) => log::error!("failed to start capture backend: {e:#}"),
            Err(_) => log::error!("capture backend panicked during startup"),
        }
    }
    Ok(g)
}

pub(crate) fn capture_screen_native() -> Result<GrayImage> {
    let g = global_capturer()?;
    let c = g.capturer.as_ref().context("capture backend unavailable")?;
    c.capture_all()
}

pub(crate) fn capture_region_native(
    left: i32,
    top: i32,
    width: i32,
    height: i32,
) -> Result<GrayImage> {
    let g = global_capturer()?;
    let c = g.capturer.as_ref().context("capture backend unavailable")?;
    c.capture_region(left, top, width, height)
}

/// toggles continuous capture: while set, every arriving frame is read into
/// the retained buffer so pixel reads during macro playback skip the request
/// roundtrip. no-op while the backend is unavailable.
pub(crate) fn set_capture_continuous(on: bool) {
    let Ok(g) = GLOBAL_CAPTURER.lock() else {
        return;
    };
    if let Some(c) = g.capturer.as_ref() {
        c.state().lock().unwrap().continuous = on;
    }
}

/// single-pixel RGB read used by the if-pixel-color condition: served from
/// the retained buffer when it covers `(x, y)` and is younger than
/// [`PIXEL_FRESHNESS_TTL`], else a fresh [`PIXEL_TILE_SIZE`] tile request.
pub(crate) fn capture_pixel_color(x: i32, y: i32) -> Result<(u8, u8, u8)> {
    let g = global_capturer()?;
    let c = g.capturer.as_ref().context("capture backend unavailable")?;

    let s = c.state().lock().unwrap();
    if s.retained_at
        .is_some_and(|t| t.elapsed() < PIXEL_FRESHNESS_TTL)
        && let Some(px) = state::retained_pixel(&s, x, y)
    {
        return Ok(px);
    }
    drop(s);

    let ox = (x - PIXEL_TILE_MARGIN).max(0);
    let oy = (y - PIXEL_TILE_MARGIN).max(0);
    let img = c.capture_region_color(ox, oy, PIXEL_TILE_SIZE, PIXEL_TILE_SIZE)?;
    let lx = (x - ox) as u32;
    let ly = (y - oy) as u32;
    if lx < img.width() && ly < img.height() {
        let p = img.get_pixel(lx, ly);
        Ok((p[0], p[1], p[2]))
    } else {
        bail!("pixel ({x}, {y}) is outside the captured region")
    }
}

/// whether a color-region scan could be answered from the retained buffer.
enum RetainedScan {
    /// the scan ran; the found region's center and size `(cx, cy, w, h)` in logical coordinates if one qualified.
    Done(Option<(i32, i32, u32, u32)>),
    /// the retained buffer was stale or did not fully cover the region.
    Refresh,
}

/// finds the largest connected region of pixels within `tolerance` (0-100,
/// euclidean RGB distance) of (r, g, b) in `region` (global logical
/// coordinates; None = the whole output), ignoring regions narrower than
/// `min_width` or shorter than `min_height`; returns its center and size.
///
/// scans the retained buffer directly when fresh and fully covering, else
/// forces a fresh frame readback so the reply and scan agree on the geometry.
pub(crate) fn capture_color_region(
    region: Option<(i32, i32, i32, i32)>,
    r: u8,
    g: u8,
    b: u8,
    tolerance: u8,
    min_width: u32,
    min_height: u32,
) -> Result<Option<(i32, i32, u32, u32)>> {
    let guard = global_capturer()?;
    let c = guard
        .capturer
        .as_ref()
        .context("capture backend unavailable")?;

    let (x, y, w, h) = match region {
        Some(r) => r,
        None => {
            let (px, py) = c.output().pos;
            let (pw, ph) = c.output().size;
            (px, py, pw as i32, ph as i32)
        }
    };

    if let RetainedScan::Done(found) = scan_color_region_from_retained(
        c, x, y, w, h, r, g, b, tolerance, min_width, min_height, true,
    )? {
        return Ok(found);
    }

    // slow path: the retained buffer was stale or did not cover the region; retry once in case a concurrent request replaced it.
    for _ in 0..2 {
        c.capture_region_color(x, y, w, h)?;
        if let RetainedScan::Done(found) = scan_color_region_from_retained(
            c, x, y, w, h, r, g, b, tolerance, min_width, min_height, false,
        )? {
            return Ok(found);
        }
    }
    Ok(None)
}

#[allow(clippy::too_many_arguments)]
fn scan_color_region_from_retained(
    c: &Capturer,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    r: u8,
    g: u8,
    b: u8,
    tolerance: u8,
    min_width: u32,
    min_height: u32,
    require_fresh: bool,
) -> Result<RetainedScan> {
    let s = c.state().lock().unwrap();
    if require_fresh
        && !s
            .retained_at
            .is_some_and(|t| t.elapsed() < PIXEL_FRESHNESS_TTL)
    {
        return Ok(RetainedScan::Refresh);
    }
    let Some((vx0, vy0, vx1, vy1)) = state::visible_region_rect(&s, x, y, w, h) else {
        return Ok(RetainedScan::Done(None)); // region fully outside the output.
    };
    if !state::retained_rect_covered(&s, vx0, vy0, vx1, vy1) {
        return Ok(RetainedScan::Refresh);
    }
    let (snap, sw, sh) = state::retained_snapshot(&s, vx0, vy0, vx1, vy1);
    let fmt = s.retained_fmt;
    drop(s);

    let Some(region) = blob::largest_color_region(
        &snap,
        sw as usize * 4,
        sw,
        sh,
        fmt,
        r,
        g,
        b,
        tolerance,
        min_width,
        min_height,
    ) else {
        return Ok(RetainedScan::Done(None)); // scanned, nothing large enough.
    };

    // center and bounding box: snapshot local -> frame -> global logical.
    let s = c.state().lock().unwrap();
    let center = state::frame_to_logical(
        &s,
        vx0 + region.cx.round() as i64,
        vy0 + region.cy.round() as i64,
    );
    let p0 = state::frame_to_logical(&s, vx0 + i64::from(region.x0), vy0 + i64::from(region.y0));
    let p1 = state::frame_to_logical(&s, vx0 + i64::from(region.x1), vy0 + i64::from(region.y1));
    Ok(RetainedScan::Done(match (center, p0, p1) {
        (Some((cx, cy)), Some((x0, y0)), Some((x1, y1))) => {
            Some((cx, cy, (x1 - x0 + 1) as u32, (y1 - y0 + 1) as u32))
        }
        _ => None,
    }))
}

/// full-output color snapshot plus the output's logical position and size, for the frozen-screen selection overlay.
pub(crate) fn capture_output_color() -> Result<(RgbaImage, ScreenPos, ScreenSize)> {
    let g = global_capturer()?;
    let c = g.capturer.as_ref().context("capture backend unavailable")?;
    let (px, py) = c.output().pos;
    let (pw, ph) = c.output().size;
    if pw == 0 || ph == 0 {
        bail!("capture output reports an empty size");
    }
    let img = c.capture_region_color(px, py, pw as i32, ph as i32)?;
    Ok((img, (px, py), (pw, ph)))
}
