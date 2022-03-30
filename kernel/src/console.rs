//! Text console

use core::fmt;

use az::{SaturatingAs, SaturatingCast};
use embedded_graphics::mono_font::{ascii, MonoTextStyle};
use embedded_graphics::prelude::*;
use embedded_graphics::text::renderer::TextRenderer;
use embedded_graphics::text::{Alignment, Text, TextStyle};
use platypos_platform::Platform;

pub struct Console<P: Platform> {
    text_style: TextStyle,
    character_style: MonoTextStyle<'static, P::DisplayColor>,
    cursor: Point,
    origin: Point,
    display: P::Display,
}

/// Console margin, in pixels
const MARGIN: i32 = 5;

// TODO: consider the embedded-text crate, although it doesn't support appending
// + reflowing text

impl<P: Platform> Console<P> {
    pub fn new(display: P::Display) -> Self {
        let text_style = TextStyle::with_alignment(Alignment::Left);
        let character_style = MonoTextStyle::new(&ascii::FONT_10X20, P::DisplayColor::GREEN);

        let origin = Point::new(MARGIN, MARGIN + line_height(&text_style, &character_style));

        Self {
            display,
            cursor: origin,
            origin,
            text_style,
            character_style,
        }
    }

    pub fn write(&mut self, s: &str) -> Result<(), P::DisplayError> {
        // The overall algorithm is to loop through characters until we find a newline
        // or exceed the screen width, then go to the next line and keep going

        let size = self.display.size();
        let mut line_start = 0;
        let mut line_width: u32 = self.cursor.x.saturating_as();

        for (idx, ch) in s.char_indices() {
            let needs_line_break = ch == '\n' || {
                let char_width = self
                    .character_style
                    .measure_string(
                        &s[idx..idx + ch.len_utf8()],
                        self.cursor,
                        self.text_style.baseline,
                    )
                    .bounding_box
                    .size
                    .width;
                // Either this character doesn't force a line break, and we need to add it to
                // the line width, or it does and we're resetting the line width to 0 anyways.
                line_width += char_width;

                line_width >= size.width
            };

            if needs_line_break {
                // Write out the current line and a newline
                self.cursor = Text::with_text_style(
                    &s[line_start..idx],
                    self.cursor,
                    self.character_style,
                    self.text_style,
                )
                .draw(&mut self.display)?;
                self.newline()?;

                line_start = if ch == '\n' { idx + 1 } else { idx };
                line_width = 0;
            }
        }

        // Finally, write out any remaining text
        if line_start < s.len() {
            self.cursor = Text::with_text_style(
                &s[line_start..],
                self.cursor,
                self.character_style,
                self.text_style,
            )
            .draw(&mut self.display)?;
        }
        Ok(())
    }

    pub fn clear(&mut self) -> Result<(), P::DisplayError> {
        self.display.clear(P::DisplayColor::BLACK)?;
        self.cursor = self.origin;
        Ok(())
    }

    pub fn newline(&mut self) -> Result<(), P::DisplayError> {
        let new_y = self.cursor.y + line_height(&self.text_style, &self.character_style);
        if new_y > self.display.size().height.saturating_cast() {
            self.clear()
        } else {
            self.cursor = Point::new(MARGIN, new_y);
            Ok(())
        }
    }

    /// Gets the underlying display
    #[inline(always)]
    #[allow(dead_code)]
    pub fn into_display(self) -> P::Display {
        self.display
    }
}

impl<P: Platform> fmt::Write for Console<P> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write(s).map_err(|_| fmt::Error)
    }
}

fn line_height<S: TextRenderer>(text_style: &TextStyle, character_style: &S) -> i32 {
    text_style
        .line_height
        .to_absolute(character_style.line_height())
        .saturating_as::<i32>()
}
