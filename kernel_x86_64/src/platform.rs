//! x86-64 implementation of the [`Platform`] trait.

use core::convert::Infallible;

use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::pixelcolor::Bgr888;
use platypos_platform::Platform;

use crate::framebuffer::FrameBufferTarget;

#[derive(PartialEq, Eq, Clone, Copy, Hash, Debug)]
pub struct PlatformX86_64 {}

impl Platform for PlatformX86_64 {
    type DisplayColor = Bgr888;
    type DisplayError = Infallible;
    type Display = FrameBufferTarget<'static>;
}
