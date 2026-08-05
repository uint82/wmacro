use anyhow::{Context, Result};
use image::GrayImage;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

static IMAGE_CACHE: OnceLock<Mutex<HashMap<String, Arc<GrayImage>>>> = OnceLock::new();

pub(crate) fn load_target_cached(path: &str) -> Result<Arc<GrayImage>> {
    let cache = IMAGE_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    {
        let map = cache.lock().unwrap();
        if let Some(img) = map.get(path) {
            return Ok(img.clone());
        }
    }
    let img_rgba = image::open(path).context("Failed to open target image")?.into_rgba8();
    let w = img_rgba.width();
    let h = img_rgba.height();
    let mut img = GrayImage::new(w, h);
    
    img_rgba.into_raw()
        .chunks_exact(4)
        .zip(img.iter_mut())
        .for_each(|(rgba, g)| {
            *g = ((rgba[0] as u16 * 77 + rgba[1] as u16 * 150 + rgba[2] as u16 * 29) >> 8) as u8;
        });

    let arc = Arc::new(img);
    cache.lock().unwrap().insert(path.to_string(), arc.clone());
    Ok(arc)
}

pub fn invalidate_target_cache(path: &str) {
    if let Some(cache) = IMAGE_CACHE.get() {
        cache.lock().unwrap().remove(path);
    }
}
