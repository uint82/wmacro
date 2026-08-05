use anyhow::{Context, Result};
use image::GrayImage;
use std::sync::Mutex;
use once_cell::sync::Lazy;

static REUSABLE_BUFFER: Lazy<Mutex<Vec<u8>>> = Lazy::new(|| Mutex::new(Vec::new()));

pub(crate) fn capture_screen_native() -> Result<GrayImage> {
    let t0 = std::time::Instant::now();

    let img_dynamic = {
        let wayshot = libwayshot::WayshotConnection::new().context("Failed to connect to Wayland")?;
        wayshot.screenshot_all(false).context("Failed to capture screen natively")?
    };

    let img_rgba = img_dynamic.into_rgba8();
    let w = img_rgba.width();
    let h = img_rgba.height();
    let pixel_count = (w * h) as usize;

    let mut buffer_guard = REUSABLE_BUFFER.lock().unwrap();
    if buffer_guard.len() != pixel_count {
        buffer_guard.resize(pixel_count, 0u8);
    }

    img_rgba.into_raw()
        .chunks_exact(4)
        .zip(buffer_guard.iter_mut())
        .for_each(|(rgba, g)| {
            *g = ((rgba[0] as u16 * 77 + rgba[1] as u16 * 150 + rgba[2] as u16 * 29) >> 8) as u8;
        });

    let img_gray = GrayImage::from_raw(w, h, buffer_guard.clone()).unwrap();
    log::debug!("[TIMING] On-Demand GUI Capture took: {:?}", t0.elapsed());

    Ok(img_gray)
}

pub(crate) fn capture_region_native(left: i32, top: i32, width: i32, height: i32) -> Result<GrayImage> {
    let t0 = std::time::Instant::now();
    let full_screen = capture_screen_native()?;

    let cropped = image::imageops::crop_imm(
        &full_screen,
        left.max(0) as u32,
        top.max(0) as u32,
        width.max(0) as u32,
        height.max(0) as u32,
    ).to_image();

    log::debug!("[TIMING] Capture & crop took: {:?}", t0.elapsed());
    Ok(cropped)
}
