//! macro playback: command dispatch, flow control, screen detection, clipboard access and the playback thread.

mod commands;
mod control;
mod detection;
mod dispatch;
mod effects;
mod engine;
pub(crate) mod expr;
mod frame;
mod models;
mod utils;
mod variables;
pub(crate) mod x11_clipboard;

pub use engine::spawn_player;
