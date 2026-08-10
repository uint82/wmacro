pub mod cache;
pub mod capture;
pub mod fft;
pub mod integral;
pub mod math;
pub mod matching;
pub mod outputs;
pub mod wayland;

use anyhow::Result;
use image::GrayImage;

use cache::load_target_cached;
use capture::{capture_region_native, capture_screen_native};
use matching::match_in_memory;

pub use cache::invalidate_target_cache;
pub use wayland::{capture_area, capture_screen, highlight_region, select_region};

pub fn find_image(
    target_path: &str,
    region: Option<(i32, i32, i32, i32)>,
    threshold: f32,
) -> Result<Option<(u32, u32)>> {
    log::debug!("--- [TIMING] Starting find_image for {} ---", target_path);
    let t_start = std::time::Instant::now();
    let target = load_target_cached(target_path)?;
    log::debug!("[TIMING] Load/cache target image took: {:?}", t_start.elapsed());

    let t_capture = std::time::Instant::now();
    let (search, offset_x, offset_y) = if let Some((l, t, w, h)) = region {
        let img = capture_region_native(l, t, w, h)?;
        (img, l.max(0) as u32, t.max(0) as u32)
    } else {
        let img = capture_screen_native()?;
        (img, 0u32, 0u32)
    };
    log::debug!("[TIMING] Total capture pipeline took: {:?}", t_capture.elapsed());

    let res = match_in_memory(&search, &target, offset_x, offset_y, threshold);
    log::debug!("--- [TIMING] Finished find_image in {:?} ---", t_start.elapsed());
    res
}

pub fn find_image_in_frame(
    screen: &GrayImage,
    target_path: &str,
    offset_x: u32,
    offset_y: u32,
    threshold: f32,
) -> Result<Option<(u32, u32)>> {
    let target = load_target_cached(target_path)?;
    match_in_memory(screen, &target, offset_x, offset_y, threshold)
}
