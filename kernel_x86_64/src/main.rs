#![no_std]
#![no_main]

use bootloader::{entry_point, BootInfo};
use platypos_kernel::kmain;

mod framebuffer;
mod platform;

use self::platform::PlatformX86_64;

static HELLO: &[u8] = b"Hello World!";

fn start(info: &'static mut BootInfo) -> ! {
    {
        use embedded_graphics::mono_font::{ascii::FONT_10X20, MonoTextStyle};
        use embedded_graphics::pixelcolor::Rgb888;
        use embedded_graphics::prelude::*;
        use embedded_graphics::text::Text;
        use framebuffer::FrameBufferTarget;
        let mut display = FrameBufferTarget::new(info.framebuffer.as_mut().unwrap());
        display.clear(Rgb888::WHITE).unwrap();

        let style = MonoTextStyle::new(&FONT_10X20, Rgb888::BLUE);
        Text::new("Hello, PlatypOS!", Point::new(20, 30), style)
            .draw(&mut display)
            .unwrap();
    }

    kmain::<PlatformX86_64>();
}

entry_point!(start);
