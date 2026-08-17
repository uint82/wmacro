//! screen detection conditions: `IfPixelColor`, `IfImageFound` and `IfColorFound`.

use log::error;
use wmacro_core_types::Value;

use crate::macro_engine::player::frame::ExecFrame;
use crate::macro_engine::player::models::{FlowControl, PlaybackContext};
use crate::macro_engine::player::utils::resolve_coord;

#[allow(clippy::too_many_arguments)] // one argument per IfPixelColor field
pub(super) fn execute_if_pixel_color(
    x: &wmacro_core_types::Coord,
    y: &wmacro_core_types::Coord,
    r: u8,
    g: u8,
    b: u8,
    tolerance: u8,
    ctx: &mut PlaybackContext,
    frame: &mut ExecFrame,
) -> FlowControl {
    let px = resolve_coord(x, &ctx.variables);
    let py = resolve_coord(y, &ctx.variables);
    let (cr, cg, cb) = crate::cursor::get_pixel_color(px, py);
    if !is_color_match(cr, cg, cb, r, g, b, tolerance) {
        frame.skip_to_else_or_endif();
    }
    FlowControl::Continue
}

fn is_color_match(cr: u8, cg: u8, cb: u8, r: u8, g: u8, b: u8, tolerance: u8) -> bool {
    if tolerance == 0 {
        return cr == r && cg == g && cb == b;
    }
    // squared euclidean distance: avoids the sqrt; the threshold is the max
    // distance a tolerance percentage allows (441.673 = sqrt(3 * 255^2)).
    let dr = cr as i64 - r as i64;
    let dg = cg as i64 - g as i64;
    let db = cb as i64 - b as i64;
    let dist_sq = f64::from((dr * dr + dg * dg + db * db) as i32);
    let max = 441.673_f64 * (f64::from(tolerance) / 100.0);
    dist_sq <= max * max
}

#[allow(clippy::too_many_arguments)] // one argument per IfImageFound field
pub(super) fn execute_if_image_found(
    path: &str,
    threshold: f32,
    move_cursor: bool,
    trigger_not_found: bool,
    region: &Option<(i32, i32, i32, i32)>,
    store_x: Option<&str>,
    store_y: Option<&str>,
    ctx: &mut PlaybackContext,
    frame: &mut ExecFrame,
) -> FlowControl {
    let result = crate::image_utils::find_image(path, *region, threshold);

    match result {
        Ok(Some((x, y))) => {
            ctx.last_image_pos = Some((x as i32, y as i32));
            if let Some(name) = store_x {
                ctx.variables
                    .insert(name.to_string(), Value::Number(x as i64));
            }
            if let Some(name) = store_y {
                ctx.variables
                    .insert(name.to_string(), Value::Number(y as i64));
            }
            handle_found_image(x, y, path, move_cursor, trigger_not_found, frame);
        }
        Ok(None) => handle_missing_image(trigger_not_found, frame),
        Err(e) => handle_image_error(e, trigger_not_found, frame),
    }
    FlowControl::Continue
}

#[allow(clippy::too_many_arguments)]
pub(super) fn execute_if_color_found(
    region: &Option<(i32, i32, i32, i32)>,
    r: u8,
    g: u8,
    b: u8,
    tolerance: u8,
    min_width: u32,
    min_height: u32,
    move_cursor: bool,
    store_x: Option<&str>,
    store_y: Option<&str>,
    store_w: Option<&str>,
    store_h: Option<&str>,
    ctx: &mut PlaybackContext,
    frame: &mut ExecFrame,
) -> FlowControl {
    let result = crate::image_utils::capture::capture_color_region(
        *region, r, g, b, tolerance, min_width, min_height,
    );

    match result {
        Ok(Some((cx, cy, cw, ch))) => {
            if move_cursor
                && let Ok(mut backend_guard) = crate::GLOBAL_BACKEND.get().unwrap().lock()
            {
                let _ = backend_guard.move_to(cx, cy);
            }
            if let Some(name) = store_x {
                ctx.variables
                    .insert(name.to_string(), Value::Number(i64::from(cx)));
            }
            if let Some(name) = store_y {
                ctx.variables
                    .insert(name.to_string(), Value::Number(i64::from(cy)));
            }
            if let Some(name) = store_w {
                ctx.variables
                    .insert(name.to_string(), Value::Number(i64::from(cw)));
            }
            if let Some(name) = store_h {
                ctx.variables
                    .insert(name.to_string(), Value::Number(i64::from(ch)));
            }
        }
        Ok(None) => frame.skip_to_else_or_endif(),
        Err(e) => {
            error!("IfColorFound error: {}", e);
            frame.skip_to_else_or_endif();
        }
    }
    FlowControl::Continue
}

fn handle_found_image(
    x: u32,
    y: u32,
    path: &str,
    move_cursor: bool,
    trigger_not_found: bool,
    frame: &mut ExecFrame,
) {
    if trigger_not_found {
        frame.skip_to_else_or_endif();
    } else if move_cursor {
        move_cursor_to_image(path, x, y);
    }
}

fn move_cursor_to_image(path: &str, x: u32, y: u32) {
    if let Ok(img) = image::open(path) {
        let center_x = x as i32 + (img.width() / 2) as i32;
        let center_y = y as i32 + (img.height() / 2) as i32;
        if let Ok(mut backend_guard) = crate::GLOBAL_BACKEND.get().unwrap().lock() {
            let _ = backend_guard.move_to(center_x, center_y);
        }
    }
}

fn handle_missing_image(trigger_not_found: bool, frame: &mut ExecFrame) {
    if !trigger_not_found {
        frame.skip_to_else_or_endif();
    }
}

fn handle_image_error(e: impl std::fmt::Display, trigger_not_found: bool, frame: &mut ExecFrame) {
    error!("IfImageFound error: {}", e);
    if !trigger_not_found {
        frame.skip_to_else_or_endif();
    }
}
