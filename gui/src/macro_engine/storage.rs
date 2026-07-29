use wmacro_core_types::Macro;
use std::fs;
use std::path::{Path, PathBuf};

pub fn default_macro_dir() -> PathBuf {
    let base_dir = directories::ProjectDirs::from("", "", "wmacro")
        .map(|d| d.config_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    let dir = base_dir.join("macros");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn ensure_dir(dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("create dir: {e}"))
}

pub fn save_wmr(m: &Macro, path: &Path) -> Result<(), String> {
    ensure_dir(path.parent().unwrap_or(Path::new(".")))?;
    let script = crate::macro_engine::script::serialize(m);
    fs::write(path, script).map_err(|e| format!("write wmr: {e}"))
}

pub fn load_wmr(path: &Path) -> Result<Macro, String> {
    let script = fs::read_to_string(path).map_err(|e| format!("read wmr: {e}"))?;
    crate::macro_engine::script::deserialize(&script)
}

pub fn macro_wmr_path(name: &str) -> PathBuf {
    default_macro_dir().join(format!("{}.wmr", name))
}
