use alloc::{borrow::Cow, boxed::Box, vec, vec::Vec};
use core::ops::{Deref, DerefMut};

use bytemuck::pod_collect_to_vec;

use wipi_types::wipic::{WIPICFramebuffer, WIPICIndirectPtr, WIPICWord};

use wie_backend::canvas::{ArgbPixel, Canvas, Color, Image, ImageBufferCanvas, PixelType, Rgb8Pixel, Rgb565Pixel, VecImageBuffer};
use wie_util::{Result, WieError};

use crate::context::WIPICContext;

// same 256MB as wie_core_arm's HEAP_SIZE; not referenced directly to avoid the dependency
const MAX_FRAMEBUFFER_BYTES: u32 = 0x1000_0000;

fn buffer_size(width: u32, height: u32, bytes_per_pixel: u32) -> Result<(u32, u32)> {
    let bpl = width.checked_mul(bytes_per_pixel).ok_or(WieError::AllocationFailure)?;
    let size = bpl.checked_mul(height).ok_or(WieError::AllocationFailure)?;
    if size > MAX_FRAMEBUFFER_BYTES {
        return Err(WieError::AllocationFailure);
    }

    Ok((size, bpl))
}


// [PHASE8_22_RGB565_RAW_IMAGE] zero-repack RGB565 presentation image.
//
// MC_grpFlushLcd only needs an Image view over the guest framebuffer. The old
// path converted the copied framebuffer bytes into Vec<u16>, then the web
// backend expanded that again into Vec<Color> and finally another RGBA Vec on
// every frame. Keep the 16-bit pixels in their original byte representation so
// the web backend can expand RGB565 -> RGBA in one pass. Drawing canvases keep
// using VecImageBuffer below, so guest graphics semantics are unchanged.
pub struct RawRgb565Image {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

impl RawRgb565Image {
    fn new(width: u32, height: u32, data: Vec<u8>) -> Self {
        Self { width, height, data }
    }
}

impl Image for RawRgb565Image {
    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }

    fn bytes_per_pixel(&self) -> u32 {
        2
    }

    fn get_pixel(&self, x: i32, y: i32) -> Color {
        let index = ((y as u32 * self.width + x as u32) * 2) as usize;
        let raw = u16::from_le_bytes([self.data[index], self.data[index + 1]]);
        Rgb565Pixel::to_color(raw)
    }

    fn raw(&self) -> Cow<'_, [u8]> {
        Cow::Borrowed(&self.data)
    }

    fn colors(&self) -> Vec<Color> {
        self.data
            .chunks_exact(2)
            .map(|pixel| Rgb565Pixel::to_color(u16::from_le_bytes([pixel[0], pixel[1]])))
            .collect()
    }
}

pub struct FrameBuffer(pub WIPICFramebuffer);

impl FrameBuffer {
    pub fn empty() -> Self {
        Self(WIPICFramebuffer {
            width: 0,
            height: 0,
            bpl: 0,
            bpp: 0,
            buf: WIPICIndirectPtr(0),
        })
    }

    pub fn new(context: &mut dyn WIPICContext, width: WIPICWord, height: WIPICWord, bpp: WIPICWord) -> Result<Self> {
        let bytes_per_pixel = bpp / 8;

        let (size, bpl) = buffer_size(width, height, bytes_per_pixel)?;
        let buf = context.alloc(size)?;

        Ok(Self(WIPICFramebuffer {
            width,
            height,
            bpl,
            bpp: bytes_per_pixel * 8,
            buf,
        }))
    }

    pub fn from_image(context: &mut dyn WIPICContext, image: &dyn Image) -> Result<Self> {
        let (size, bpl) = buffer_size(image.width(), image.height(), image.bytes_per_pixel())?;
        let buf = context.alloc(size)?;

        context.write_bytes(context.data_ptr(buf)?, &image.raw())?;

        Ok(Self(WIPICFramebuffer {
            width: image.width(),
            height: image.height(),
            bpl,
            bpp: image.bytes_per_pixel() * 8,
            buf,
        }))
    }

    pub fn data(&self, context: &dyn WIPICContext) -> Result<Vec<u8>> {
        let (size, _) = buffer_size(self.0.width, self.0.height, self.0.bpp / 8)?;
        let mut buf = vec![0; size as _];
        context.read_bytes(context.data_ptr(self.0.buf)?, &mut buf)?;

        Ok(buf)
    }

    pub fn image(&self, context: &mut dyn WIPICContext) -> Result<Box<dyn Image>> {
        let data = self.data(context)?;

        Ok(match self.0.bpp {
            16 => Box::new(RawRgb565Image::new(
                self.0.width as _,
                self.0.height as _,
                data,
            )),
            32 => Box::new(VecImageBuffer::<ArgbPixel>::from_raw(
                self.0.width as _,
                self.0.height as _,
                pod_collect_to_vec(&data),
            )),
            _ => unimplemented!("Unsupported pixel format: {}", self.0.bpp),
        })
    }

    pub fn canvas<'a>(&'a self, context: &'a mut dyn WIPICContext) -> Result<FramebufferCanvas<'a>> {
        let data = self.data(context)?;

        let canvas: Box<dyn Canvas> = match self.0.bpp {
            16 => Box::new(ImageBufferCanvas::new(VecImageBuffer::<Rgb565Pixel>::from_raw(
                self.0.width as _,
                self.0.height as _,
                pod_collect_to_vec(&data),
            ))),
            32 => Box::new(ImageBufferCanvas::new(VecImageBuffer::<ArgbPixel>::from_raw(
                self.0.width as _,
                self.0.height as _,
                pod_collect_to_vec(&data),
            ))),
            _ => unimplemented!("Unsupported pixel format: {}", self.0.bpp),
        };

        Ok(FramebufferCanvas {
            framebuffer: self,
            context,
            canvas,
            flushed: false,
        })
    }

    pub fn write(&self, context: &mut dyn WIPICContext, data: &[u8]) -> Result<()> {
        context.write_bytes(context.data_ptr(self.0.buf)?, data)
    }

    pub fn pixel_to_color(&self, pixel: WIPICWord) -> Color {
        match self.0.bpp {
            16 => Rgb565Pixel::to_color(pixel as u16),
            _ => Rgb8Pixel::to_color(pixel),
        }
    }
}

pub struct FramebufferCanvas<'a> {
    framebuffer: &'a FrameBuffer,
    context: &'a mut dyn WIPICContext,
    canvas: Box<dyn Canvas>,
    flushed: bool,
}

impl FramebufferCanvas<'_> {
    pub fn flush(mut self) -> Result<()> {
        self.flushed = true;

        self.framebuffer.write(self.context, &self.canvas.image().raw())
    }
}

// best-effort fallback for canvases dropped without an explicit flush
impl Drop for FramebufferCanvas<'_> {
    fn drop(&mut self) {
        if self.flushed {
            return;
        }

        tracing::warn!("framebuffer canvas dropped without explicit flush; write-back errors will be lost");

        if let Err(err) = self.framebuffer.write(self.context, &self.canvas.image().raw()) {
            tracing::error!("Failed to flush framebuffer canvas: {err}");
        }
    }
}

impl Deref for FramebufferCanvas<'_> {
    type Target = Box<dyn Canvas>;

    fn deref(&self) -> &Self::Target {
        &self.canvas
    }
}

impl DerefMut for FramebufferCanvas<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.canvas
    }
}

#[cfg(test)]
mod test {
    use wie_util::WieError;

    use crate::context::test::TestContext;

    use super::FrameBuffer;

    #[test]
    fn test_new_overflow_returns_error() {
        let mut context = TestContext::new();

        assert!(matches!(
            FrameBuffer::new(&mut context, 0x10000, 0x10000, 32),
            Err(WieError::AllocationFailure)
        ));
    }

    #[test]
    fn test_new_over_heap_limit_returns_error() {
        let mut context = TestContext::new();

        assert!(matches!(
            FrameBuffer::new(&mut context, 0x4000, 0x4000, 32),
            Err(WieError::AllocationFailure)
        ));
    }

    #[test]
    fn test_new_zero_height_bpl_overflow_returns_error() {
        let mut context = TestContext::new();

        assert!(matches!(
            FrameBuffer::new(&mut context, 0xffff_ffff, 0, 32),
            Err(WieError::AllocationFailure)
        ));
    }

    #[test]
    fn test_new_normal_size_ok() {
        let mut context = TestContext::new();

        let framebuffer = FrameBuffer::new(&mut context, 100, 100, 16).unwrap();
        assert_eq!(framebuffer.0.width, 100);
        assert_eq!(framebuffer.0.height, 100);
        assert_eq!(framebuffer.0.bpl, 200);
        assert_eq!(framebuffer.0.bpp, 16);
        assert_eq!(framebuffer.data(&context).unwrap().len(), 20000);
    }
}
