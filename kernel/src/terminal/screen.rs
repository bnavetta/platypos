#![allow(dead_code)] // color changing API won't necessarily get used

use core::{convert::TryFrom, fmt, mem};

use bit_field::BitField;
use core::fmt::Write;
use spin::Mutex;
use ux::u4;
use volatile::Volatile;
use x86_64::{instructions::interrupts, VirtAddr};

// Largely based on https://os.phil-opp.com/vga-text-mode/ and https://en.wikipedia.org/wiki/VGA-compatible_text_mode

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Color {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGray = 7,
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    Pink = 13,
    Yellow = 14,
    White = 15,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct InvalidColorCode;

impl TryFrom<u8> for Color {
    type Error = InvalidColorCode;

    fn try_from(value: u8) -> Result<Color, InvalidColorCode> {
        if value <= Color::White as u8 {
            Ok(unsafe { mem::transmute(value) })
        } else {
            Err(InvalidColorCode)
        }
    }
}

impl From<u4> for Color {
    fn from(value: u4) -> Color {
        unsafe { mem::transmute(u8::from(value)) }
    }
}

impl Into<u4> for Color {
    fn into(self) -> u4 {
        u4::new(self as u8)
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
#[repr(C)]
struct ScreenCharacter {
    character: u8,
    attribute: u8,
}

impl ScreenCharacter {
    const fn new(
        character: u8,
        foreground: Color,
        background: Color,
        blink: bool,
    ) -> ScreenCharacter {
        ScreenCharacter {
            character,
            attribute: (background as u8) << 4 | (foreground as u8) | (blink as u8) << 7,
        }
    }

    #[inline]
    fn is_blink(&self) -> bool {
        self.attribute.get_bit(7)
    }

    #[inline]
    fn set_blink(&mut self, blink: bool) {
        self.attribute.set_bit(7, blink);
    }

    #[inline]
    fn foreground(&self) -> Color {
        Color::try_from(self.attribute.get_bits(0..4)).expect("Invalid color code")
    }

    #[inline]
    fn set_foreground(&mut self, color: Color) {
        self.attribute.set_bits(0..4, color as u8);
    }

    #[inline]
    fn background(&self) -> Color {
        Color::try_from(self.attribute.get_bits(4..7)).expect("Invalid color code")
    }

    #[inline]
    fn set_background(&mut self, color: Color) {
        self.attribute.set_bits(4..7, color as u8);
    }
}

impl fmt::Debug for ScreenCharacter {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("ScreenCharacter")
            .field("character", &char::from(self.character))
            .field("foreground", &self.foreground())
            .field("background", &self.background())
            .field("blink", &self.is_blink())
            .finish()
    }
}

#[repr(transparent)]
struct VgaBuffer {
    chars: [[Volatile<ScreenCharacter>; VgaBuffer::WIDTH]; VgaBuffer::HEIGHT],
}

impl VgaBuffer {
    const HEIGHT: usize = 25;
    const WIDTH: usize = 80;

    unsafe fn new(addr: VirtAddr) -> &'static mut VgaBuffer {
        addr.as_mut_ptr::<VgaBuffer>().as_mut().unwrap()
    }
}

pub struct VgaWriter {
    current_row: usize,
    current_column: usize,
    foreground: Color,
    background: Color,
    buffer: &'static mut VgaBuffer,
}

impl VgaWriter {
    fn new(buffer: &'static mut VgaBuffer, foreground: Color, background: Color) -> VgaWriter {
        VgaWriter {
            current_row: 0,
            current_column: 0,
            foreground,
            background,
            buffer,
        }
    }

    pub fn foreground(&self) -> Color {
        self.foreground
    }

    pub fn set_foreground(&mut self, color: Color) {
        self.foreground = color;
    }

    pub fn background(&self) -> Color {
        self.background
    }

    pub fn set_background(&mut self, color: Color) {
        self.background = color;
    }

    fn current_position(&self) -> (usize, usize) {
        (self.current_row, self.current_column)
    }

    fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.newline(),
            b'\r' => self.current_column = 0,
            byte => {
                if self.current_column >= VgaBuffer::WIDTH {
                    self.newline();
                }

                self.buffer.chars[self.current_row][self.current_column].write(
                    ScreenCharacter::new(byte, self.foreground, self.background, false),
                );
                self.current_column += 1;
            }
        }
    }

    fn newline(&mut self) {
        self.current_row += 1;
        self.current_column = 0;

        // If the screen is full, shift all the text up a row
        if self.current_row >= VgaBuffer::HEIGHT {
            for row in 1..VgaBuffer::HEIGHT {
                for col in 0..VgaBuffer::WIDTH {
                    self.buffer.chars[row - 1][col].write(self.buffer.chars[row][col].read())
                }
            }

            self.clear_row(VgaBuffer::HEIGHT - 1);
            self.current_row -= 1; // Undo moving to the next row
        }
    }

    fn clear_row(&mut self, row: usize) {
        let blank = ScreenCharacter::new(b' ', self.foreground, self.background, false);

        for i in 0..VgaBuffer::WIDTH {
            self.buffer.chars[row][i].write(blank);
        }
    }

    pub fn clear(&mut self) {
        let blank = ScreenCharacter::new(b' ', self.foreground, self.background, false);
        for row in 0..VgaBuffer::HEIGHT {
            for col in 0..VgaBuffer::WIDTH {
                self.buffer.chars[row][col].write(blank)
            }
        }

        self.current_row = 0;
        self.current_column = 0;
    }
}

impl fmt::Write for VgaWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            if byte.is_ascii() {
                self.write_byte(byte);
            } else {
                // Replace non-printable characters with '■'
                self.write_byte(0xfe);
            }
        }

        Ok(())
    }
}

static WRITER: Mutex<Option<VgaWriter>> = Mutex::new(None);

pub fn init(vga_addr: VirtAddr) {
    let buffer = unsafe { VgaBuffer::new(vga_addr) };

    let mut writer = VgaWriter::new(buffer, Color::Green, Color::Black);
    writer.clear();

    let mut global_writer = WRITER.lock();
    global_writer.replace(writer);
}

pub fn with_writer<F: FnOnce(&mut VgaWriter) -> T, T>(func: F) -> T {
    interrupts::without_interrupts(|| {
        let mut writer = WRITER.lock();
        func(writer.as_mut().expect("VGA writer not initialized"))
    })
}

pub fn vga_print(args: fmt::Arguments) {
    with_writer(|w| w.write_fmt(args)).unwrap();
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::terminal::screen::vga_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}
