use anyhow::{Context, Result};
use std::process::Command;
use std::thread;
use std::time::Duration;

pub fn select_region() -> Result<String> {
    let output = Command::new("slurp").output().context("Failed to execute slurp")?;
    if !output.status.success() {
        anyhow::bail!("slurp cancelled or failed");
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn capture_area(path: &str) -> Result<()> {
    let geom = select_region()?;
    let out = Command::new("grim")
        .args(["-g", &geom, path])
        .output()
        .context("Failed to execute grim")?;
    if !out.status.success() {
        anyhow::bail!("grim failed");
    }
    Ok(())
}

pub fn capture_screen(path: &str) -> Result<()> {
    let out = Command::new("grim")
        .arg(path)
        .output()
        .context("Failed to execute grim")?;
    if !out.status.success() {
        anyhow::bail!("grim failed");
    }
    Ok(())
}

pub fn highlight_region(left: i32, top: i32, width: i32, height: i32, duration_secs: u64) {
    let geom = format!("{},{} {}x{}\n", left, top, width, height);

    thread::spawn(move || {
        let mut child = std::process::Command::new("slurp")
            .args([
                "-r",
                "-b", "00000088",
                "-c", "ffffffff",
            ])
            .stdin(std::process::Stdio::piped())
            .spawn()
            .expect("slurp highlight spawn failed");

        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            let _ = stdin.write_all(geom.as_bytes());
        }

        thread::sleep(Duration::from_secs(duration_secs));
        let _ = child.kill();
        let _ = child.wait();
    });
}
