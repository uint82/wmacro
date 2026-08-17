use log::{error, info};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::process::Command;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use wmacro_core_types::{DaemonEvent, DaemonRequest, HardwareEvent, HotkeyEvent};

pub mod evdev_listener;
pub mod uinput;

fn get_socket_path() -> String {
    "/run/wmacro/wmacro.sock".to_string()
}

fn main() {
    env_logger::init();
    info!("[daemon] Starting wmacro daemon...");

    let mut backend = match init_backend() {
        Some(b) => b,
        None => std::process::exit(1),
    };

    let socket_path = get_socket_path();
    let listener = bind_socket(&socket_path);

    info!("[daemon] Listening for GUI commands on {}...", socket_path);

    for stream in listener.incoming() {
        accept_stream(stream, &mut backend);
    }
}

fn init_backend() -> Option<uinput::UinputBackend> {
    uinput::UinputBackend::new()
        .map_err(|e| {
            error!("[daemon] FATAL: Failed to initialize uinput: {}", e);
            error!(
                "[daemon] Ensure you are in the 'wmacro' group and the uinput module is loaded."
            );
        })
        .ok()
}

fn bind_socket(socket_path: &str) -> UnixListener {
    if let Some(parent) = std::path::Path::new(socket_path).parent()
        && !parent.exists()
    {
        // TODO: propagate startup errors as Results instead of panicking here.
        std::fs::create_dir_all(parent).expect("Failed to create socket directory");

        if let (Ok(uid), Ok(gid)) = (std::env::var("SUDO_UID"), std::env::var("SUDO_GID")) {
            let mut dir_perms = fs::metadata(parent).unwrap().permissions();
            dir_perms.set_mode(0o700);
            fs::set_permissions(parent, dir_perms).unwrap();

            let _ = Command::new("chown")
                .arg(format!("{}:{}", uid, gid))
                .arg(parent)
                .status();
        } else {
            let mut dir_perms = fs::metadata(parent).unwrap().permissions();
            dir_perms.set_mode(0o755);
            fs::set_permissions(parent, dir_perms).unwrap();
        }
    }

    let _ = fs::remove_file(socket_path);

    let listener = UnixListener::bind(socket_path).expect("Failed to bind socket");

    let mut perms = fs::metadata(socket_path).unwrap().permissions();
    perms.set_mode(0o666);
    fs::set_permissions(socket_path, perms).unwrap();

    listener
}

fn accept_stream(stream: std::io::Result<UnixStream>, backend: &mut uinput::UinputBackend) {
    let stream = match stream {
        Ok(stream) => stream,
        Err(e) => {
            error!("[daemon] Failed to accept connection: {}", e);
            return;
        }
    };

    info!("[daemon] GUI client connected.");
    handle_client(stream, backend);
}

fn handle_client(stream: UnixStream, backend: &mut uinput::UinputBackend) {
    let writer_stream = stream.try_clone().expect("Failed to clone socket stream");
    let mut reader = BufReader::new(stream);

    let (tx_hw, rx_hw): (Sender<HardwareEvent>, Receiver<HardwareEvent>) = mpsc::channel();
    let (tx_hotkey, rx_hotkey): (Sender<HotkeyEvent>, Receiver<HotkeyEvent>) = mpsc::channel();

    let stop_flag = Arc::new(AtomicBool::new(false));
    let is_recording = Arc::new(AtomicBool::new(false));

    info!("[daemon] Spawning permanent evdev_listener for this client...");
    evdev_listener::spawn_evdev_listener(tx_hw, tx_hotkey, stop_flag.clone());

    let writer = writer_stream;
    let is_recording_clone = is_recording.clone();

    std::thread::spawn(move || {
        forward_events(writer, rx_hw, rx_hotkey, is_recording_clone);
    });

    read_requests(&mut reader, backend, &is_recording);

    info!("[daemon] Stopping evdev_listener...");
    stop_flag.store(true, Ordering::SeqCst);
}

struct DrainResult {
    got_any: bool,
    disconnected: bool,
    write_failed: bool,
}

fn forward_events(
    mut writer: UnixStream,
    rx_hw: Receiver<HardwareEvent>,
    rx_hotkey: Receiver<HotkeyEvent>,
    is_recording: Arc<AtomicBool>,
) {
    loop {
        let hw = drain(&mut writer, &rx_hw, |hw_event| {
            if !is_recording.load(Ordering::SeqCst) {
                return None;
            }
            Some(DaemonEvent::Recorded(hw_event))
        });
        if hw.write_failed {
            return;
        }

        let hotkey = drain(&mut writer, &rx_hotkey, |hotkey_event| {
            Some(DaemonEvent::Hotkey(hotkey_event))
        });
        if hotkey.write_failed {
            return;
        }

        if hw.disconnected && hotkey.disconnected {
            break;
        }

        if !hw.got_any && !hotkey.got_any {
            // TODO: block on the channels with a timeout instead of this 1ms sleep-poll.
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
}

fn drain<T>(
    writer: &mut UnixStream,
    rx: &Receiver<T>,
    to_event: impl Fn(T) -> Option<DaemonEvent>,
) -> DrainResult {
    let mut got_any = false;

    loop {
        match rx.try_recv() {
            Ok(item) => {
                got_any = true;
                let Some(event) = to_event(item) else {
                    continue;
                };
                if send_event(writer, event).is_err() {
                    return DrainResult {
                        got_any,
                        disconnected: false,
                        write_failed: true,
                    };
                }
            }
            Err(TryRecvError::Empty) => {
                return DrainResult {
                    got_any,
                    disconnected: false,
                    write_failed: false,
                };
            }
            Err(TryRecvError::Disconnected) => {
                return DrainResult {
                    got_any,
                    disconnected: true,
                    write_failed: false,
                };
            }
        }
    }
}

fn send_event(writer: &mut UnixStream, event: DaemonEvent) -> std::io::Result<()> {
    let mut bytes = match serde_json::to_vec(&event) {
        Ok(b) => b,
        Err(_) => return Ok(()),
    };
    bytes.push(b'\n');
    writer.write_all(&bytes)
}

fn read_requests(
    reader: &mut BufReader<UnixStream>,
    backend: &mut uinput::UinputBackend,
    is_recording: &Arc<AtomicBool>,
) {
    let mut line = String::new();

    while let Ok(bytes_read) = reader.read_line(&mut line) {
        if bytes_read == 0 {
            info!("[daemon] GUI client disconnected.");
            break;
        }

        dispatch_line(&line, backend, is_recording);
        line.clear();
    }
}

fn dispatch_line(line: &str, backend: &mut uinput::UinputBackend, is_recording: &Arc<AtomicBool>) {
    let req = match serde_json::from_str::<DaemonRequest>(line) {
        Ok(req) => req,
        Err(_) => {
            error!("[daemon] Failed to parse request: {}", line);
            return;
        }
    };

    handle_request(backend, req, is_recording);
}

fn handle_request(
    backend: &mut uinput::UinputBackend,
    req: DaemonRequest,
    is_recording: &Arc<AtomicBool>,
) {
    let result = match req {
        DaemonRequest::MoveTo { x, y } => backend.move_to(x, y),
        DaemonRequest::Press { button } => backend.press(&button),
        DaemonRequest::Release { button } => backend.release(&button),
        DaemonRequest::Scroll { dx, dy } => backend.scroll(dx, dy),
        DaemonRequest::KeyDown { key, code } => backend.key_down(&key, code),
        DaemonRequest::KeyUp { key, code } => backend.key_up(&key, code),
        DaemonRequest::Click {
            target_x,
            target_y,
            button,
            click_type,
            hold_duration_ms,
            move_cursor,
            ..
        } => backend.click(
            target_x,
            target_y,
            &button,
            &click_type,
            hold_duration_ms,
            move_cursor,
        ),
        DaemonRequest::StartRecording => {
            handle_start_recording(is_recording);
            Ok(())
        }
        DaemonRequest::StopRecording => {
            handle_stop_recording(is_recording);
            Ok(())
        }
    };

    if let Err(e) = result {
        error!("[daemon] Backend interaction failed: {:?}", e);
    }
}

fn handle_start_recording(is_recording: &Arc<AtomicBool>) {
    if is_recording.swap(true, Ordering::SeqCst) {
        info!("[daemon] Recording is already running.");
    } else {
        info!("[daemon] Started recording.");
    }
}

fn handle_stop_recording(is_recording: &Arc<AtomicBool>) {
    if is_recording.swap(false, Ordering::SeqCst) {
        info!("[daemon] Stopped recording.");
    } else {
        info!("[daemon] No recording was running.");
    }
}
