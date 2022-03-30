use core::convert::Infallible;

use bootloader::boot_info::{FrameBuffer, PixelFormat};
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::pixelcolor::Bgr888;
use embedded_graphics::prelude::*;
use embedded_graphics::Pixel;

pub type Display = FrameBufferTarget<'static>;
pub type Color = Bgr888;
pub type Error = Infallible;

pub struct FrameBufferTarget<'a> {
    inner: &'a mut FrameBuffer,
}

impl<'a> FrameBufferTarget<'a> {
    pub fn new(buffer: &'a mut FrameBuffer) -> FrameBufferTarget<'a> {
        FrameBufferTarget { inner: buffer }
    }
}

impl<'a> DrawTarget for FrameBufferTarget<'a> {
    // TODO: this assumes RGB byte support as the default and converts on the fly if
    // not
    type Color = Color;

    type Error = Error;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        let info = self.inner.info();
        let stride: i32 = info.stride.try_into().expect("stride larger than i32::MAX");
        let buffer = self.inner.buffer_mut();

        for Pixel(coord, color) in pixels.into_iter() {
            let pixel_offset = coord.y * stride + coord.x;
            let byte_offset = pixel_offset as usize * info.bytes_per_pixel;
            match info.pixel_format {
                PixelFormat::RGB => {
                    // Could avoid some work by casting the frame buffer to a u32 array, but that
                    // seems... sketchy
                    buffer[byte_offset] = color.r();
                    buffer[byte_offset + 1] = color.g();
                    buffer[byte_offset + 2] = color.b();
                }
                PixelFormat::BGR => {
                    buffer[byte_offset] = color.b();
                    buffer[byte_offset + 1] = color.g();
                    buffer[byte_offset + 2] = color.r();
                }
                other => panic!("Pixel format {other:?} not supported"),
            }

            // TODO: volatile read/write necessary?
        }

        Ok(())
    }
}

impl<'a> OriginDimensions for FrameBufferTarget<'a> {
    fn size(&self) -> Size {
        let info = self.inner.info();
        Size::new(
            info.horizontal_resolution as u32,
            info.vertical_resolution as u32,
        )
    }
}
