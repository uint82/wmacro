// TODO(egl-readback): EGL/GL readback would be faster (as OBS, WebRTC,
// wl-screenrec do), but NVIDIA's broken EGL_EXT_image_dma_buf_import keeps
// this CPU path as the fallback.

use std::os::fd::RawFd;
use std::time::Instant;

use anyhow::{Context, Result, bail};
use gbm::{AsRaw, Device as GbmDevice, Format as GbmFormat};

/// GBM_BO_USE_SW_READ_OFTEN, as grim uses for imported scanout buffers; not exposed by gbm-rs.
const GBM_BO_USE_SW_READ_OFTEN: u32 = 1 << 6;

struct RawBo(*mut gbm_sys::gbm_bo);

impl Drop for RawBo {
    fn drop(&mut self) {
        unsafe { gbm_sys::gbm_bo_destroy(self.0) };
    }
}

pub(super) struct GbmSession {
    _devfile: std::fs::File,
    device: GbmDevice<std::fs::File>,
}

impl GbmSession {
    pub(super) fn open() -> Result<Self> {
        for idx in 128..=150 {
            let path = format!("/dev/dri/renderD{idx}");
            match std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&path)
            {
                Ok(file) => {
                    let devfile = file.try_clone().context("failed to clone render node fd")?;
                    match GbmDevice::new(file) {
                        Ok(device) => {
                            return Ok(GbmSession {
                                _devfile: devfile,
                                device,
                            });
                        }
                        Err(e) => log::debug!("gbm init failed on {path}: {e}"),
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => break,
                Err(e) => log::debug!("cannot open {path}: {e}"),
            }
        }
        bail!("no usable DRM render node found (/dev/dri/renderD*)")
    }

    /// copies the frame (or a `region` of it, in frame pixels; `None` = whole
    /// frame) into `out`, compacted row-by-row. the import lifetime is tied to
    /// `out`: callers must not hold the data past the PipeWire buffer's lifetime.
    /// `modifier` (0 = linear) must be the exact negotiated value or the driver
    /// refuses CPU access; region reads skip the uncached dma-buf readback of
    /// the rest of the screen.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn read_frame(
        &self,
        fd: RawFd,
        fourcc: GbmFormat,
        width: u32,
        height: u32,
        stride: u32,
        offset: u32,
        modifier: u64,
        region: Option<(u32, u32, u32, u32)>,
        out: &mut Vec<u8>,
    ) -> Result<()> {
        let read_start_of = Instant::now();
        let dev = self.device.as_raw() as *mut gbm_sys::gbm_device;
        let mut bo = if modifier != 0 {
            let mut import = gbm_sys::gbm_import_fd_modifier_data {
                width,
                height,
                format: fourcc as u32,
                num_fds: 1,
                fds: [fd, -1, -1, -1],
                strides: [stride as i32, 0, 0, 0],
                offsets: [offset as i32, 0, 0, 0],
                modifier,
            };
            unsafe {
                gbm_sys::gbm_bo_import(
                    dev,
                    gbm_sys::GBM_BO_IMPORT_FD_MODIFIER,
                    &mut import as *mut _ as *mut libc::c_void,
                    GBM_BO_USE_SW_READ_OFTEN,
                )
            }
        } else {
            std::ptr::null_mut()
        };
        if bo.is_null() {
            let mut import = gbm_sys::gbm_import_fd_data {
                fd,
                width,
                height,
                stride,
                format: fourcc as u32,
            };
            bo = unsafe {
                gbm_sys::gbm_bo_import(
                    dev,
                    gbm_sys::GBM_BO_IMPORT_FD,
                    &mut import as *mut _ as *mut libc::c_void,
                    GBM_BO_USE_SW_READ_OFTEN,
                )
            };
        }
        let import_took = read_start_of.elapsed();
        if bo.is_null() {
            bail!("gbm_bo_import failed: {}", std::io::Error::last_os_error());
        }
        let _guard = RawBo(bo);

        let map_start = Instant::now();
        let mut map_stride = 0u32;
        let mut map_data: *mut libc::c_void = std::ptr::null_mut();
        let ptr = unsafe {
            gbm_sys::gbm_bo_map(
                bo,
                0,
                0,
                width,
                height,
                gbm_sys::gbm_bo_transfer_flags::GBM_BO_TRANSFER_READ,
                &mut map_stride,
                &mut map_data,
            )
        };
        if ptr.is_null() {
            bail!("gbm_bo_map failed: {}", std::io::Error::last_os_error());
        }
        let (rx0, ry0, rw, rh) = region.unwrap_or((0, 0, width, height));
        let rw = rw.min(width - rx0.min(width));
        let rh = rh.min(height - ry0.min(height));
        out.clear();
        out.reserve(rw as usize * rh as usize * 4);
        let copy_start = Instant::now();
        unsafe {
            let base = ptr as *const u8;
            for row in 0..rh {
                let src_row =
                    base.add((ry0 + row) as usize * map_stride as usize + rx0 as usize * 4);
                out.extend_from_slice(std::slice::from_raw_parts(src_row, rw as usize * 4));
            }
            gbm_sys::gbm_bo_unmap(bo, map_data);
        }
        log::trace!(
            "read_frame: region {}x{} import took {:.3}ms, map took {:.3}ms, copy took {:.3}ms",
            rw,
            rh,
            import_took.as_secs_f64() * 1000.0,
            map_start.elapsed().as_secs_f64() * 1000.0,
            copy_start.elapsed().as_secs_f64() * 1000.0
        );
        Ok(())
    }
}
