//! screen capture backend.
//!
//! stack:
//! xdg-desktop-portal ScreenCast -> PipeWire stream -> DMABuf -> gbm CPU
//! readback. regions are cropped client-side from the captured output, which
//! keeps it portable across compositors (KDE, GNOME, Hyprland, Wayfire, ...).
//!
//! layout:
//! - [`portal`] - ScreenCast session, restore-token persistence, output matching
//! - [`stream`] - PipeWire stream, format negotiation, capture thread
//! - [`gbm`] - CPU readback of dma-bufs (see its TODO for the planned
//!   EGL/GL readback path)
//! - [`state`] - pending requests and the retained-frame serve path
//! - [`pixel`] - format mapping and luma conversion
//!
//! the portal session uses a persistent restore token ("wmacro"), so after the
//! first grant no permission dialog is shown. frames are copied inside the
//! PipeWire `process` callback while the buffer is still alive; the copy is
//! only made when at least one capture request is pending (copy-on-demand),
//! and only the bounding box of the requested regions is read back from the
//! dma-buf (region readback) instead of the whole screen.

mod gbm;
mod pixel;
mod portal;
mod state;
mod stream;

#[cfg(test)]
mod tests;

use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use ashpd::desktop::screencast::{
    CursorMode, Screencast, SelectSourcesOptions, SourceType, StartCastOptions,
};
use ashpd::desktop::{PersistMode, Session};
use image::{DynamicImage, GrayImage, RgbaImage};
use once_cell::sync::Lazy;

use super::outputs::{query_outputs, OutputInfo};

use gbm::GbmSession;
use portal::{load_restore_token, match_output, save_restore_token};
use state::CaptureState;
use stream::pw_capture_thread;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const RETRY_COOLDOWN: Duration = Duration::from_secs(10);
const PORTAL_TIMEOUT: Duration = Duration::from_secs(20);

/// logical position of an output on the desktop, in surface (logical) pixels.
pub(crate) type ScreenPos = (i32, i32);
/// logical size of an output, in surface (logical) pixels.
pub(crate) type ScreenSize = (u32, u32);

pub struct Capturer {
    requests: mpsc::Sender<state::CaptureRequest>,

    /// write end of the self-pipe that wakes the capture loop on new requests.
    wake: OwnedFd,

    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    session: Option<Session<Screencast>>,
    output: OutputInfo,
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
            if let Err(e) = pw_capture_thread(fd, gbm, node_id, rx, wake_rfd, thread_stop, thread_state) {
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
        })
    }

    pub fn output(&self) -> &OutputInfo {
        &self.output
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
            libc::write(self.wake.as_raw_fd(), byte.as_ptr() as *const libc::c_void, 1);
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

static GLOBAL_CAPTURER: Lazy<Mutex<GlobalBackend>> =
    Lazy::new(|| Mutex::new(GlobalBackend { capturer: None, last_attempt: None }));

fn global_capturer() -> Result<MutexGuard<'static, GlobalBackend>> {
    let mut g = GLOBAL_CAPTURER
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let cooldown_ok = g
        .last_attempt
        .is_none_or(|t| t.elapsed() > RETRY_COOLDOWN);
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

pub(crate) fn capture_region_native(left: i32, top: i32, width: i32, height: i32) -> Result<GrayImage> {
    let g = global_capturer()?;
    let c = g.capturer.as_ref().context("capture backend unavailable")?;
    c.capture_region(left, top, width, height)
}

pub(crate) fn capture_region_color_native(
    left: i32,
    top: i32,
    width: i32,
    height: i32,
) -> Result<RgbaImage> {
    let g = global_capturer()?;
    let c = g.capturer.as_ref().context("capture backend unavailable")?;
    c.capture_region_color(left, top, width, height)
}

/// full-output color snapshot plus the output's logical position and size,
/// used by the frozen-screen selection overlay.
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
