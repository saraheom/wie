use alloc::{sync::Arc, vec::Vec};
use core::{cell::RefCell, sync::atomic::{AtomicBool, AtomicUsize, Ordering}};

use wasm_bindgen::{Clamped, JsCast};
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, ImageData};

use wie_backend::{Screen, canvas::Image};
use wie_util::Result;

static PHASE8_79_PRESENT_COUNT: AtomicUsize = AtomicUsize::new(0);

pub struct WindowImpl {
    canvas: HtmlCanvasElement,
    context: CanvasRenderingContext2d,
    // Reuse one RGBA staging buffer across frames. WindowImpl is wasm-only and
    // already explicitly marked Send/Sync below because the runtime is single
    // threaded, so RefCell is appropriate here and avoids a per-frame Vec alloc.
    rgba_buffer: RefCell<Vec<u8>>,
    should_redraw: Arc<AtomicBool>,
}

unsafe impl Send for WindowImpl {} // XXX We're on wasm, so it's fine
unsafe impl Sync for WindowImpl {}

impl WindowImpl {
    pub fn new(canvas: HtmlCanvasElement, should_redraw: Arc<AtomicBool>) -> Self {
        // Phase 8.22: obtain the JS 2D context once instead of crossing the
        // wasm-bindgen getContext/dyn_into path for every emulated frame.
        let context = canvas
            .get_context("2d")
            .unwrap()
            .unwrap()
            .dyn_into::<CanvasRenderingContext2d>()
            .unwrap();
        Self { canvas, context, rgba_buffer: RefCell::new(Vec::new()), should_redraw }
    }
}

impl Screen for WindowImpl {
    fn resize(&self, width: u32, height: u32) -> Result<()> {
        self.canvas.set_width(width);
        self.canvas.set_height(height);
        self.request_redraw()
    }

    fn request_redraw(&self) -> Result<()> {
        self.should_redraw.store(true, Ordering::SeqCst);

        Ok(())
    }

    fn paint(&self, image: &dyn Image) {
        // [PHASE8_22_WEB_RGB565_FASTPAINT] presentation hot path. WIPI games normally present a
        // 16-bit RGB565 framebuffer. Avoid allocating Vec<Color> and then a
        // second RGBA Vec. Expand the raw little-endian pixels directly into a
        // reusable RGBA staging buffer in one pass. 32-bit ARGB gets the same
        // treatment. Other image types retain the generic color fallback.
        let mut rgba = self.rgba_buffer.borrow_mut();
        match image.bytes_per_pixel() {
            2 => {
                let raw = image.raw();
                let required = raw.len().saturating_mul(2);
                rgba.resize(required, 0);
                for (pixel, out) in raw.chunks_exact(2).zip(rgba.chunks_exact_mut(4)) {
                    let value = u16::from_le_bytes([pixel[0], pixel[1]]);
                    let r5 = ((value >> 11) & 0x1f) as u32;
                    let g6 = ((value >> 5) & 0x3f) as u32;
                    let b5 = (value & 0x1f) as u32;
                    out[0] = ((r5 * 255 + 15) / 31) as u8;
                    out[1] = ((g6 * 255 + 31) / 63) as u8;
                    out[2] = ((b5 * 255 + 15) / 31) as u8;
                    out[3] = 0xff;
                }
            }
            4 => {
                let raw = image.raw();
                rgba.resize(raw.len(), 0);
                // ArgbPixel stores 0xAARRGGBB in a native u32. wasm32 is
                // little-endian, so raw bytes arrive as BB GG RR AA.
                for (pixel, out) in raw.chunks_exact(4).zip(rgba.chunks_exact_mut(4)) {
                    out[0] = pixel[2];
                    out[1] = pixel[1];
                    out[2] = pixel[0];
                    out[3] = pixel[3];
                }
            }
            _ => {
                rgba.clear();
                rgba.extend(
                    image
                        .colors()
                        .into_iter()
                        .flat_map(|x| [x.r, x.g, x.b, x.a]),
                );
            }
        }
        // Phase 8.79: validate the final presentation buffer before crossing the
        // wasm-bindgen ImageData boundary.  A mismatched buffer or JS exception
        // must never become an opaque WASM trap.
        let expected_rgba = (image.width() as usize)
            .saturating_mul(image.height() as usize)
            .saturating_mul(4);
        let paint_index = PHASE8_79_PRESENT_COUNT.fetch_add(1, Ordering::Relaxed);
        let magenta = rgba
            .chunks_exact(4)
            .filter(|px| px[0] == 0xff && px[1] == 0x00 && px[2] == 0xff && px[3] != 0)
            .count();
        if paint_index < 32 || rgba.len() != expected_rgba || magenta > 0 {
            tracing::info!(
                "[PHASE8_79_PRESENT_PROBE] index={} size={}x{} bpp={} raw_len={} rgba_len={} expected_rgba={} magenta_rgba={}",
                paint_index, image.width(), image.height(), image.bytes_per_pixel(),
                image.raw().len(), rgba.len(), expected_rgba, magenta
            );
        }
        if rgba.len() != expected_rgba {
            tracing::error!(
                "[PHASE8_79_PRESENT_LENGTH_REPAIR] rgba_len={} expected={} size={}x{} bpp={}",
                rgba.len(), expected_rgba, image.width(), image.height(), image.bytes_per_pixel()
            );
            rgba.resize(expected_rgba, 0);
        }

        let data = match ImageData::new_with_u8_clamped_array_and_sh(
            Clamped(rgba.as_slice()),
            image.width(),
            image.height(),
        ) {
            Ok(data) => data,
            Err(error) => {
                tracing::error!("[PHASE8_79_PRESENT_IMAGEDATA_ERROR] error={:?}", error);
                return;
            }
        };

        if let Err(error) = self.context.put_image_data(&data, 0.0, 0.0) {
            tracing::error!("[PHASE8_79_PRESENT_PUT_ERROR] error={:?}", error);
            return;
        }
        if paint_index < 32 {
            tracing::info!("[PHASE8_79_PRESENT_RETURN] index={} ok=true", paint_index);
        }
    }

    fn width(&self) -> u32 {
        self.canvas.width()
    }

    fn height(&self) -> u32 {
        self.canvas.height()
    }
}
