//! enumerates Wayland outputs with logical geometry (position + size).
//!
//! prefers xdg_output (stable protocol) for logical positions/sizes, which
//! also transparently covers fractional scale on compositors that support it.
//! falls back to wl_output geometry/scale when xdg_output is not advertised.

use anyhow::{anyhow, bail, Context, Result};
use std::time::{Duration, Instant};

use wayland_client::globals::{registry_queue_init, GlobalListContents};
use wayland_client::protocol::{wl_output, wl_registry};
use wayland_client::{Connection, Dispatch, QueueHandle, WEnum};
use wayland_protocols::xdg::xdg_output::zv1::client::zxdg_output_manager_v1::{
    self, ZxdgOutputManagerV1,
};
use wayland_protocols::xdg::xdg_output::zv1::client::zxdg_output_v1::{self, ZxdgOutputV1};

#[derive(Debug, Clone)]
pub struct OutputInfo {
    pub name: String,

    /// position of the output in the global logical compositor space.
    pub pos: (i32, i32),

    /// logical size of the output.
    pub size: (u32, u32),
}

#[derive(Default)]
struct OutputAccum {
    name: Option<String>,
    geom_pos: Option<(i32, i32)>,
    geom_size: Option<(i32, i32)>,
    scale: Option<i32>,
    log_pos: Option<(i32, i32)>,
    log_size: Option<(i32, i32)>,
}

#[derive(Default)]
struct OutputsState {
    xdg_mgr: Option<ZxdgOutputManagerV1>,
    proxies: Vec<wl_output::WlOutput>,
    outputs: Vec<OutputAccum>,
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for OutputsState {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // global advertisements are handled by the GlobalList internals.
    }
}

impl Dispatch<wl_output::WlOutput, usize> for OutputsState {
    fn event(
        state: &mut Self,
        _: &wl_output::WlOutput,
        event: wl_output::Event,
        data: &usize,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let acc = &mut state.outputs[*data];
        match event {
            wl_output::Event::Name { name } => acc.name = Some(name),
            wl_output::Event::Geometry { x, y, .. } => acc.geom_pos = Some((x, y)),
            wl_output::Event::Mode { flags, width, height, .. } => {
                if matches!(flags, WEnum::Value(m) if m.contains(wl_output::Mode::Current)) {
                    acc.geom_size = Some((width, height));
                }
            }
            wl_output::Event::Scale { factor } => acc.scale = Some(factor),
            _ => {}
        }
    }
}

impl Dispatch<ZxdgOutputManagerV1, ()> for OutputsState {
    fn event(
        _: &mut Self,
        _: &ZxdgOutputManagerV1,
        _: zxdg_output_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZxdgOutputV1, usize> for OutputsState {
    fn event(
        state: &mut Self,
        _: &ZxdgOutputV1,
        event: zxdg_output_v1::Event,
        data: &usize,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let acc = &mut state.outputs[*data];
        match event {
            zxdg_output_v1::Event::LogicalPosition { x, y } => acc.log_pos = Some((x, y)),
            zxdg_output_v1::Event::LogicalSize { width, height } => {
                acc.log_size = Some((width, height));
            }
            _ => {}
        }
    }
}

pub fn query_outputs() -> Result<Vec<OutputInfo>> {
    let conn = Connection::connect_to_env().context("failed to connect to the Wayland display")?;
    let (globals, mut queue) = registry_queue_init::<OutputsState>(&conn)
        .map_err(|e| anyhow!("failed to initialize the global registry: {e}"))?;
    let qh = queue.handle();

    let mut state = OutputsState::default();
    globals.contents().with_list(|list| {
        for global in list {
            if global.interface == "wl_output" {
                let idx = state.outputs.len();
                state.outputs.push(OutputAccum::default());
                let out = globals.registry().bind::<wl_output::WlOutput, usize, OutputsState>(
                    global.name,
                    global.version.min(4),
                    &qh,
                    idx,
                );
                state.proxies.push(out);
            } else if global.interface == "zxdg_output_manager_v1" && state.xdg_mgr.is_none() {
                let mgr = globals.registry().bind::<ZxdgOutputManagerV1, (), OutputsState>(
                    global.name,
                    global.version.min(3),
                    &qh,
                    (),
                );
                state.xdg_mgr = Some(mgr);
            }
        }
    });
    if state.outputs.is_empty() {
        bail!("the compositor advertised no wl_output globals");
    }

    let use_xdg = state.xdg_mgr.is_some();
    if let Some(mgr) = &state.xdg_mgr {
        for idx in 0..state.outputs.len() {
            mgr.get_xdg_output(&state.proxies[idx], &qh, idx);
        }
    }

    // dispatch until every output reports its geometry (or we give up).
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let ready = if use_xdg {
            state
                .outputs
                .iter()
                .all(|a| a.log_pos.is_some() && a.log_size.is_some())
        } else {
            state
                .outputs
                .iter()
                .all(|a| a.geom_size.is_some())
        };
        if ready || Instant::now() >= deadline {
            break;
        }
        if queue.roundtrip(&mut state).is_err() {
            break;
        }
    }

    let mut infos = Vec::new();
    for acc in &state.outputs {
        let info = if use_xdg {
            match (acc.log_pos, acc.log_size) {
                (Some(pos), Some(size)) => Some(OutputInfo {
                    name: acc.name.clone().unwrap_or_default(),
                    pos,
                    size: (size.0.max(0) as u32, size.1.max(0) as u32),
                }),
                _ => None,
            }
        } else {
            match (acc.geom_pos, acc.geom_size) {
                (Some(pos), Some(size)) => {
                    let scale = acc.scale.unwrap_or(1).max(1) as f64;
                    let size = (
                        (size.0.max(0) as f64 / scale).round() as u32,
                        (size.1.max(0) as f64 / scale).round() as u32,
                    );
                    Some(OutputInfo {
                        name: acc.name.clone().unwrap_or_default(),
                        pos,
                        size,
                    })
                }
                _ => None,
            }
        };
        if let Some(info) = info {
            infos.push(info);
        }
    }
    if infos.is_empty() {
        bail!("failed to obtain output geometry from the compositor");
    }
    Ok(infos)
}
