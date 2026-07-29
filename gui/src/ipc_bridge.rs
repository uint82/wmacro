use wmacro_core_types::{DaemonEvent, HardwareEvent, HotkeyEvent};
use std::sync::mpsc::{Receiver, Sender};

pub fn spawn_ipc_bridge(
    event_rx: Receiver<DaemonEvent>,
    recorder_tx: Sender<HardwareEvent>,
    hotkey_tx: Sender<HotkeyEvent>,
) {
    std::thread::spawn(move || {
        for event in event_rx {
            match event {
                DaemonEvent::Recorded(hw) => {
                    if recorder_tx.send(hw).is_err() {
                        log::warn!("Recorder receiver dropped, stopping bridge.");
                        break;
                    }
                }
                DaemonEvent::Hotkey(hk) => {
                    if hotkey_tx.send(hk).is_err() {
                        log::warn!("Hotkey receiver dropped, stopping bridge.");
                        break;
                    }
                }
            }
        }
        log::info!("event_rx closed, bridge thread exiting.");
    });
}
