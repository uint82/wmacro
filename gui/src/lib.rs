use std::sync::{Mutex, OnceLock, mpsc};

mod app;
mod backend;
mod cursor;
mod hotkey;
pub mod image_utils;
mod ipc_bridge;
pub mod macro_engine;
mod settings;
mod state;
mod ui;

pub static GLOBAL_BACKEND: OnceLock<Mutex<Box<dyn backend::ClickBackend + Send>>> = OnceLock::new();
pub static IPC_EVENT_RX: OnceLock<Mutex<Option<mpsc::Receiver<wmacro_core_types::DaemonEvent>>>> =
    OnceLock::new();

pub fn run() -> eframe::Result<()> {
    // egui-winit logs an ERROR on empty-clipboard paste (a normal no-op here); the editor handles copy/cut/paste itself, so that module's logs are noise.
    env_logger::builder()
        .filter_module("egui_winit::clipboard", log::LevelFilter::Off)
        .init();

    let (event_tx, event_rx) = mpsc::channel::<wmacro_core_types::DaemonEvent>();

    if IPC_EVENT_RX.set(Mutex::new(Some(event_rx))).is_err() {
        log::warn!("IPC_EVENT_RX already set.");
    }

    // the backend is process-global because the capture pipeline (portal, DMABuf pools) is heavyweight to construct; one instance is shared everywhere.
    let backend_status = match backend::build_backend(event_tx) {
        Ok(backend) => {
            if GLOBAL_BACKEND.set(Mutex::new(backend)).is_err() {
                log::warn!("Failed to initialize global backend.");
            }
            Ok(())
        }
        Err(e) => {
            if GLOBAL_BACKEND
                .set(Mutex::new(Box::new(backend::DummyBackend)))
                .is_err()
            {
                log::warn!("Failed to set dummy backend.");
            }
            Err(e)
        }
    };

    let options = eframe::NativeOptions {
        // the app is a utility window, not a document: modest default size, but never smaller than what the editor comfortably fits.
        viewport: egui::ViewportBuilder::default()
            .with_title("wmacro")
            .with_app_id("wmacro")
            .with_inner_size([1000.0, 700.0])
            .with_min_inner_size([800.0, 600.0])
            .with_resizable(true),
        ..Default::default()
    };

    eframe::run_native(
        "wmacro",
        options,
        Box::new(move |cc| Ok(Box::new(app::WmacroApp::new(cc, backend_status)))),
    )
    .ok();

    std::process::exit(0);
}
