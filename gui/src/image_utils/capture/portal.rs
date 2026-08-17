//! XDG Desktop Portal screencast session setup and restore-token persistence.

use std::path::PathBuf;

use anyhow::{Result, bail};

use crate::image_utils::outputs::OutputInfo;

fn token_path() -> PathBuf {
    if let Some(dir) = std::env::var_os("XDG_CACHE_HOME") {
        PathBuf::from(dir).join("wmacro-restore-token")
    } else if let Some(home) = std::env::var_os("HOME") {
        PathBuf::from(home).join(".cache/wmacro-restore-token")
    } else {
        PathBuf::from("/tmp/wmacro-restore-token")
    }
}

pub(super) fn load_restore_token() -> Option<String> {
    std::fs::read_to_string(token_path())
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub(super) fn save_restore_token(token: Option<&str>) {
    let path = token_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match token {
        Some(t) => {
            let _ = std::fs::write(&path, t);
        }
        None => {
            let _ = std::fs::remove_file(&path);
        }
    }
}

pub(super) fn match_output(
    outputs: &[OutputInfo],
    stream_size: Option<(u32, u32)>,
    stream_pos: Option<(i32, i32)>,
    mapping_id: Option<&str>,
) -> Result<OutputInfo> {
    if let Some(o) = mapping_id.and_then(|name| {
        outputs
            .iter()
            .find(|o| !o.name.is_empty() && o.name == name)
    }) {
        return Ok(o.clone());
    }
    if let (Some(size), Some(pos)) = (stream_size, stream_pos) {
        if let Some(o) = outputs.iter().find(|o| o.pos == pos && o.size == size) {
            return Ok(o.clone());
        }
        if let Some(o) = outputs.iter().find(|o| {
            pos.0 >= o.pos.0
                && pos.1 >= o.pos.1
                && pos.0 < o.pos.0 + o.size.0 as i32
                && pos.1 < o.pos.1 + o.size.1 as i32
        }) {
            return Ok(o.clone());
        }
        if let Some(o) = outputs.iter().find(|o| o.size == size) {
            return Ok(o.clone());
        }
    }
    if outputs.len() == 1 {
        return Ok(outputs[0].clone());
    }
    bail!(
        "could not match the captured stream to a known output ({} outputs available)",
        outputs.len()
    )
}
