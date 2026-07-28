use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::sync::mpsc::Sender;
use std::time::Duration;

use core_types::{ClickButton, ClickType, DaemonEvent, DaemonRequest};

use crate::backend::{keymap, ClickBackend};

const DAEMON_SOCKET_PATH: &str = "/run/wmacro/wmacro.sock";
const KEY_CODE_SHIFT: u16 = 42;
const SHIFT_KEY: &str = "Shift";

pub struct IpcBackend {
    socket: UnixStream,
}

impl IpcBackend {
    pub fn new(event_tx: Sender<DaemonEvent>) -> Result<Self, String> {
        let socket = UnixStream::connect(DAEMON_SOCKET_PATH)
            .map_err(|e| format!("Failed to connect to Daemon at {DAEMON_SOCKET_PATH}: {e}"))?;

        let reader_socket = socket
            .try_clone()
            .map_err(|e| format!("Failed to clone socket for reader: {e}"))?;

        spawn_daemon_event_listener(reader_socket, event_tx);

        Ok(Self { socket })
    }

    fn send_req(&mut self, req: DaemonRequest) -> Result<(), String> {
        let mut bytes = serde_json::to_vec(&req)
            .map_err(|e| format!("Failed to serialize daemon request: {e}"))?;
        bytes.push(b'\n');
        self.socket.write_all(&bytes).map_err(|e| format!("Failed to write daemon request: {e}"))
    }

    fn type_character(&mut self, c: char, code: u16, shift: bool) -> Result<(), String> {
        let char_key = c.to_string();

        if shift {
            self.send_req(DaemonRequest::KeyDown {
                key: SHIFT_KEY.to_owned(),
                code: KEY_CODE_SHIFT,
            })?;
        }

        self.send_req(DaemonRequest::KeyDown {
            key: char_key.clone(),
            code,
        })?;

        // TODO: use robust synchronization method instead of sleep if possible.
        std::thread::sleep(Duration::from_millis(2));

        self.send_req(DaemonRequest::KeyUp {
            key: char_key,
            code,
        })?;

        if shift {
            self.send_req(DaemonRequest::KeyUp {
                key: SHIFT_KEY.to_owned(),
                code: KEY_CODE_SHIFT,
            })?;
        }

        std::thread::sleep(Duration::from_millis(5));

        Ok(())
    }
}

impl ClickBackend for IpcBackend {
    fn move_to(&mut self, x: i32, y: i32) -> Result<(), String> {
        self.send_req(DaemonRequest::MoveTo { x, y })?;

        #[cfg(target_os = "linux")]
        {
            if crate::cursor::hyprland_movecursor_socket(x, y).is_none() {
                log::warn!("Failed to synchronize Hyprland cursor");
            }
        }

        Ok(())
    }

    fn press(&mut self, button: &ClickButton) -> Result<(), String> {
        self.send_req(DaemonRequest::Press {
            button: button.clone(),
        })
    }

    fn release(&mut self, button: &ClickButton) -> Result<(), String> {
        self.send_req(DaemonRequest::Release {
            button: button.clone(),
        })
    }

    fn scroll(&mut self, dx: i32, dy: i32) -> Result<(), String> {
        self.send_req(DaemonRequest::Scroll { dx, dy })
    }

    fn key_down(&mut self, key: &str, code: u16) -> Result<(), String> {
        self.send_req(DaemonRequest::KeyDown {
            key: key.to_string(),
            code,
        })
    }

    fn key_up(&mut self, key: &str, code: u16) -> Result<(), String> {
        self.send_req(DaemonRequest::KeyUp {
            key: key.to_string(),
            code,
        })
    }

    fn type_text(&mut self, text: &str) -> Result<(), String> {
        for c in text.chars() {
            let (code, shift) = keymap::char_to_evdev(c)
                .ok_or_else(|| format!("Unsupported character: {c:?}"))?;
            self.type_character(c, code, shift)?;
        }
        Ok(())
    }

    fn click(
        &mut self,
        target_x: i32,
        target_y: i32,
        current_x: i32,
        current_y: i32,
        button: &ClickButton,
        click_type: &ClickType,
        hold_duration_ms: u64,
        move_cursor: bool,
    ) -> Result<(), String> {
        self.send_req(DaemonRequest::Click {
            target_x,
            target_y,
            current_x,
            current_y,
            button: button.clone(),
            click_type: click_type.clone(),
            hold_duration_ms,
            move_cursor,
        })
    }

    fn start_recording(&mut self) -> Result<(), String> {
        self.send_req(DaemonRequest::StartRecording)
    }

    fn stop_recording(&mut self) -> Result<(), String> {
        self.send_req(DaemonRequest::StopRecording)
    }
}

fn spawn_daemon_event_listener(socket: UnixStream, event_tx: Sender<DaemonEvent>) {
    std::thread::spawn(move || {
        let mut reader = BufReader::new(socket);
        let mut line = String::new();

        loop {
            line.clear();

            match reader.read_line(&mut line) {
                Ok(0) => {
                    log::warn!("Daemon closed connection.");
                    break;
                }
                Ok(_) => {
                    let Ok(event) = serde_json::from_str::<DaemonEvent>(&line) else {
                        log::error!("Failed to parse DaemonEvent (raw: {})", line.trim());
                        continue;
                    };

                    if event_tx.send(event).is_err() {
                        break;
                    }
                }
                Err(e) => {
                    log::error!("Error reading from socket: {e}");
                    break;
                }
            }
        }
    });
}
