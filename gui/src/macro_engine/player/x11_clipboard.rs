use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use log::{debug, warn};
use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ConnectionExt as _, CreateWindowAux, EventMask, PropMode, SelectionNotifyEvent,
    SelectionRequestEvent, Window, WindowClass,
};
use x11rb::wrapper::ConnectionExt as _;

/// in-process X11 clipboard access via the core `CLIPBOARD` selection; Wayland
/// compositors do not mirror the Wayland selection into X11, so XWayland apps
/// keep pasting stale text. `set_text` spawns a detached serve thread; a newer
/// set replaces it and the old thread exits via `SelectionClear`.
pub struct X11Clipboard {
    display: String,
}

impl X11Clipboard {
    /// `None` when no X11 display is available (pure Wayland or headless).
    pub fn new() -> Option<Self> {
        match std::env::var("DISPLAY") {
            Ok(display) if !display.trim().is_empty() => Some(Self { display }),
            _ => None,
        }
    }

    /// takes ownership of the X11 `CLIPBOARD` selection on a background thread;
    /// fire-and-forget, failures only log.
    pub fn set_text(&self, text: &str) {
        let display = self.display.clone();
        let text = text.to_owned();
        // each serve thread gets a generation so a superseded one never retakes ownership.
        let generation = SET_GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
        std::thread::spawn(move || {
            if let Err(e) = serve_selection(&display, &text, generation) {
                warn!("SetClipboard: X11 serve thread failed: {e}");
            }
        });
    }

    /// reads the current X11 `CLIPBOARD` text, or `None`; blocks up to a few seconds.
    pub fn get_text(&self) -> Option<String> {
        match request_selection(&self.display, false) {
            Some(text) => Some(text),
            None => {
                // older apps only offer STRING (latin-1); retry with it.
                match request_selection(&self.display, true) {
                    Some(text) => {
                        debug!("GetClipboard: X11 fallback STRING target delivered text");
                        Some(text)
                    }
                    None => {
                        warn!("GetClipboard: X11 selection request failed");
                        None
                    }
                }
            }
        }
    }
}

const TIMEOUT: Duration = Duration::from_secs(2);

/// monotonically increasing id; a thread only retakes ownership while it is the newest setter.
static SET_GENERATION: AtomicU64 = AtomicU64::new(0);

/// how long after our own set we keep fighting the wlroots mirror; later losses are real copies.
const MIRROR_FIGHT_WINDOW: Duration = Duration::from_secs(2);
const MAX_RETAKE_ATTEMPTS: u32 = 8;

/// interned fresh per connection; atom ids are per-server so this is safe.
struct Atoms {
    clipboard: Atom,
    utf8_string: Atom,
    string: Atom,
    text_plain: Atom,
    text_plain_charset: Atom,
    text: Atom,
    targets: Atom,
    timestamp: Atom,
    multiple: Atom,
    integer: Atom,
    atom: Atom,
    property: Atom,
    incr: Atom,
}

impl Atoms {
    fn intern<C: Connection>(conn: &C) -> Result<Self, Box<dyn std::error::Error>> {
        let atom = |name: &[u8]| -> Result<Atom, Box<dyn std::error::Error>> {
            Ok(conn.intern_atom(false, name)?.reply()?.atom)
        };
        Ok(Self {
            clipboard: atom(b"CLIPBOARD")?,
            utf8_string: atom(b"UTF8_STRING")?,
            string: atom(b"STRING")?,
            text_plain: atom(b"text/plain")?,
            text_plain_charset: atom(b"text/plain;charset=utf-8")?,
            text: atom(b"TEXT")?,
            targets: atom(b"TARGETS")?,
            timestamp: atom(b"TIMESTAMP")?,
            multiple: atom(b"MULTIPLE")?,
            integer: atom(b"INTEGER")?,
            atom: atom(b"ATOM")?,
            property: atom(b"WMACRO_CLIPBOARD_DATA")?,
            incr: atom(b"INCR")?,
        })
    }

    fn text_targets(&self) -> [Atom; 5] {
        [
            self.utf8_string,
            self.text_plain,
            self.text_plain_charset,
            self.string,
            self.text,
        ]
    }
}

fn connect(display: &str) -> Result<(impl Connection, usize), Box<dyn std::error::Error>> {
    Ok(x11rb::connect(Some(display))?)
}

fn create_helper_window<C: Connection>(
    conn: &C,
    screen_num: usize,
) -> Result<Window, Box<dyn std::error::Error>> {
    let screen = &conn.setup().roots[screen_num];
    let win = conn.generate_id()?;
    conn.create_window(
        x11rb::NONE as u8,
        win,
        screen.root,
        0,
        0,
        1,
        1,
        0,
        WindowClass::INPUT_ONLY,
        0,
        &CreateWindowAux::new().event_mask(EventMask::PROPERTY_CHANGE | EventMask::EXPOSURE),
    )?;
    Ok(win)
}

/// owns `CLIPBOARD` until ownership is lost; re-asserts after our own set
/// because the wlroots mirror claims the selection and serves nothing.
fn serve_selection(
    display: &str,
    text: &str,
    generation: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let (conn, screen_num) = connect(display)?;
    let atoms = Atoms::intern(&conn)?;
    let win = create_helper_window(&conn, screen_num)?;
    conn.change_property8(
        PropMode::REPLACE,
        win,
        atoms.property,
        atoms.utf8_string,
        text.as_bytes(),
    )?;
    conn.set_selection_owner(win, atoms.clipboard, 0u32)?;
    conn.flush()?;
    debug!("SetClipboard: X11 ownership of CLIPBOARD taken");

    let started = Instant::now();
    let mut retakes = 0u32;
    loop {
        let event = conn.wait_for_event()?;
        match event {
            Event::SelectionRequest(request) => {
                if let Err(e) = serve_request(&conn, &atoms, win, &request, text.as_bytes()) {
                    warn!("SetClipboard: X11 serve request failed: {e}");
                }
            }
            Event::SelectionClear(clear) if clear.owner == win => {
                let superseded = generation != SET_GENERATION.load(Ordering::Relaxed);
                if superseded
                    || started.elapsed() > MIRROR_FIGHT_WINDOW
                    || retakes >= MAX_RETAKE_ATTEMPTS
                {
                    debug!("SetClipboard: X11 CLIPBOARD ownership lost (replaced)");
                    return Ok(());
                }
                retakes += 1;
                debug!(
                    "SetClipboard: X11 CLIPBOARD taken by the wlroots mirror, re-asserting ({retakes}/{MAX_RETAKE_ATTEMPTS})"
                );
                conn.set_selection_owner(win, atoms.clipboard, 0u32)?;
                conn.flush()?;
            }
            _ => {}
        }
    }
}

fn serve_request<C: Connection>(
    conn: &C,
    atoms: &Atoms,
    _owner: Window,
    request: &SelectionRequestEvent,
    text: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    // per ICCCM the requestor may pass NONE for the property, in which case we use the target itself.
    let property = if request.property != x11rb::NONE {
        request.property
    } else {
        request.target
    };
    let mut delivered = property != x11rb::NONE;
    debug!(
        "SetClipboard: X11 SelectionRequest target={} property={}",
        conn.get_atom_name(request.target)
            .ok()
            .and_then(|c| c.reply().ok())
            .map_or_else(
                || request.target.to_string(),
                |r| String::from_utf8_lossy(&r.name).into_owned()
            ),
        request.property
    );

    if request.target == atoms.targets {
        let mut targets: Vec<u32> = atoms
            .text_targets()
            .iter()
            .copied()
            .chain([atoms.targets, atoms.timestamp])
            .collect();
        targets.extend([atoms.multiple]);
        conn.change_property32(
            PropMode::REPLACE,
            request.requestor,
            property,
            atoms.atom,
            &targets,
        )?;
    } else if request.target == atoms.timestamp {
        conn.change_property32(
            PropMode::REPLACE,
            request.requestor,
            property,
            atoms.integer,
            &[request.time],
        )?;
    } else if request.target == atoms.utf8_string
        || request.target == atoms.text_plain
        || request.target == atoms.text_plain_charset
    {
        conn.change_property8(
            PropMode::REPLACE,
            request.requestor,
            property,
            atoms.utf8_string,
            text,
        )?;
    } else if request.target == atoms.string || request.target == atoms.text {
        // STRING is latin-1 by spec; GTK/browsers prefer UTF8_STRING anyway.
        conn.change_property8(
            PropMode::REPLACE,
            request.requestor,
            property,
            atoms.string,
            text,
        )?;
    } else {
        // unknown target (including MULTIPLE): refuse by sending NONE.
        delivered = false;
    }

    if !delivered {
        conn.delete_property(request.requestor, request.property)?;
    }
    conn.send_event(
        false,
        request.requestor,
        EventMask::NO_EVENT,
        SelectionNotifyEvent {
            response_type: x11rb::protocol::xproto::SELECTION_NOTIFY_EVENT,
            sequence: 0,
            time: request.time,
            requestor: request.requestor,
            selection: request.selection,
            target: request.target,
            property: if delivered { property } else { x11rb::NONE },
        },
    )?;
    conn.flush()?;
    Ok(())
}

/// asks the owner for the `CLIPBOARD` text; `prefer_string` uses the `STRING` target.
fn request_selection(display: &str, prefer_string: bool) -> Option<String> {
    let (conn, screen_num) = connect(display).ok()?;
    let atoms = Atoms::intern(&conn).ok()?;
    let win = create_helper_window(&conn, screen_num).ok()?;
    let target = if prefer_string {
        atoms.string
    } else {
        atoms.utf8_string
    };
    conn.convert_selection(win, atoms.clipboard, target, atoms.property, 0u32)
        .ok()?;
    conn.flush().ok()?;

    let mut notified = None;
    let deadline = Instant::now() + TIMEOUT;
    while Instant::now() < deadline {
        while let Some(event) = conn.poll_for_event().ok().flatten() {
            if let Event::SelectionNotify(notify) = event
                && notify.requestor == win
            {
                notified = Some(notify);
            }
        }
        if notified.is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let notify = notified?;
    if notify.property == x11rb::NONE {
        debug!("GetClipboard: X11 conversion refused (target not offered)");
        return None;
    }

    let mut data = Vec::new();
    let mut offset = 0u32;
    loop {
        let reply = conn
            .get_property(false, win, notify.property, AtomEnum::ANY, offset, u32::MAX)
            .ok()?
            .reply()
            .ok()?;
        if reply.type_ == atoms.incr {
            // INCR streams huge payloads; not worth implementing for a clipboard-sized transfer.
            warn!("GetClipboard: X11 owner streams data via INCR, unsupported");
            return None;
        }
        data.extend_from_slice(&reply.value);
        if reply.bytes_after == 0 {
            break;
        }
        // long_offset is in 32-bit units; format-8 data is padded to 4 bytes.
        offset += (reply.value.len() as u32).div_ceil(4);
        if data.len() > 16 * 1024 * 1024 {
            warn!("GetClipboard: X11 clipboard data suspiciously large, aborting");
            return None;
        }
    }
    if data.is_empty() {
        return None;
    }
    Some(String::from_utf8_lossy(&data).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// end-to-end roundtrip against a real X server, including the wlroots mirror fight.
    #[test]
    #[ignore = "requires a live X11 display"]
    fn x11_set_then_get_roundtrip() {
        let clip = X11Clipboard::new().expect("DISPLAY must be set");
        let expected = "wmacro-x11-roundtrip-42";
        clip.set_text(expected);
        // best-effort; only triggers the mirror fight where Wayland is available.
        let _ = wl_clipboard_rs::copy::Options::new().copy(
            wl_clipboard_rs::copy::Source::Bytes(expected.as_bytes().to_vec().into()),
            wl_clipboard_rs::copy::MimeType::Text,
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if clip.get_text().as_deref() == Some(expected) {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "X11 clipboard roundtrip timed out"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}
