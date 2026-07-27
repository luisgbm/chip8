//! Window and software framebuffer. Ported from `video.js`, with the drawing
//! helpers the menu needs on top.
//!
//! Everything — the emulator's 64x32 screen, the menu, the status bar — is
//! rendered into one `0x00RRGGBB` buffer at window resolution, which is then
//! uploaded to a single streaming texture once a frame.

use anyhow::{Context, Error, Result};
use sdl2::pixels::PixelFormatEnum;
use sdl2::render::{Texture, TextureCreator, WindowCanvas};
use sdl2::video::WindowContext;

use crate::cpu::{SCREEN_HEIGHT, SCREEN_WIDTH};
use crate::font::{self, ADVANCE, GLYPH_HEIGHT, GLYPH_WIDTH};

/// How many window pixels a CHIP-8 pixel takes up.
pub const PIXEL_SIZE: usize = 16;

/// Width of the window, which is exactly the emulator screen scaled up.
pub const WINDOW_WIDTH: usize = SCREEN_WIDTH * PIXEL_SIZE;

/// Height of the emulator screen inside the window.
pub const SCREEN_AREA_HEIGHT: usize = SCREEN_HEIGHT * PIXEL_SIZE;

/// Height of the status bar drawn under the emulator screen.
pub const STATUS_BAR_HEIGHT: usize = 64;

/// Height of the window.
pub const WINDOW_HEIGHT: usize = SCREEN_AREA_HEIGHT + STATUS_BAR_HEIGHT;

/// Owns the window, the streaming texture and the pixel buffer behind them.
///
/// The texture borrows from the [`TextureCreator`], so the creator has to
/// outlive the `Video`; `main` keeps it on the stack for that reason.
pub struct Video<'a> {
    canvas: WindowCanvas,
    texture: Texture<'a>,
    pixels: Vec<u32>,
    width: usize,
    height: usize,
}

impl<'a> Video<'a> {
    /// Wraps a canvas in a `width` by `height` software framebuffer.
    ///
    /// # Errors
    ///
    /// Returns an error when SDL cannot allocate the streaming texture the
    /// framebuffer is uploaded through.
    pub fn new(
        canvas: WindowCanvas,
        texture_creator: &'a TextureCreator<WindowContext>,
        width: usize,
        height: usize,
    ) -> Result<Self> {
        let texture = texture_creator
            .create_texture_streaming(PixelFormatEnum::RGB888, width as u32, height as u32)
            .context("failed to create the streaming texture")?;

        Ok(Self {
            canvas,
            texture,
            pixels: vec![0; width * height],
            width,
            height,
        })
    }

    #[must_use]
    pub fn width(&self) -> usize {
        self.width
    }

    #[must_use]
    pub fn height(&self) -> usize {
        self.height
    }

    /// The pixel buffer, row-major, one `0x00RRGGBB` word per pixel.
    #[must_use]
    pub fn pixels(&self) -> &[u32] {
        &self.pixels
    }

    pub fn clear(&mut self, color: u32) {
        self.pixels.fill(color);
    }

    /// Fills a rectangle, clipped to the buffer.
    pub fn fill_rect(&mut self, x: i32, y: i32, width: usize, height: usize, color: u32) {
        let left = x.max(0) as usize;
        let top = y.max(0) as usize;
        let right = (x + width as i32).clamp(0, self.width as i32) as usize;
        let bottom = (y + height as i32).clamp(0, self.height as i32) as usize;

        for row in top..bottom {
            self.pixels[row * self.width + left..row * self.width + right].fill(color);
        }
    }

    /// Draws `text` with its top left corner at `(x, y)`.
    ///
    /// Returns the x coordinate just past the last glyph, which makes it easy
    /// to draw a line in more than one color.
    pub fn draw_text(&mut self, x: i32, y: i32, text: &str, scale: usize, color: u32) -> i32 {
        let mut cursor = x;

        for character in text.chars() {
            let glyph = font::glyph(character);

            for (row, bits) in glyph.iter().enumerate() {
                for column in 0..GLYPH_WIDTH {
                    if bits & (1 << (GLYPH_WIDTH - 1 - column)) == 0 {
                        continue;
                    }

                    self.fill_rect(
                        cursor + (column * scale) as i32,
                        y + (row * scale) as i32,
                        scale,
                        scale,
                        color,
                    );
                }
            }

            cursor += (ADVANCE * scale) as i32;
        }

        cursor - scale as i32
    }

    /// Draws `text` centered horizontally in the window.
    pub fn draw_text_centered(&mut self, y: i32, text: &str, scale: usize, color: u32) {
        let x = (self.width as i32 - font::text_width(text, scale) as i32) / 2;
        self.draw_text(x, y, text, scale, color);
    }

    /// Draws `text` with its right edge at `right`.
    pub fn draw_text_right(&mut self, right: i32, y: i32, text: &str, scale: usize, color: u32) {
        let x = right - font::text_width(text, scale) as i32;
        self.draw_text(x, y, text, scale, color);
    }

    /// Height of a line of text at `scale`, in pixels.
    #[must_use]
    pub fn line_height(scale: usize) -> usize {
        GLYPH_HEIGHT * scale
    }

    /// Blits a CHIP-8 framebuffer, scaling every pixel to a `scale` square.
    pub fn draw_chip8_screen(
        &mut self,
        framebuffer: &[bool],
        origin_x: i32,
        origin_y: i32,
        scale: usize,
        on: u32,
        off: u32,
    ) {
        for (index, &lit) in framebuffer.iter().enumerate() {
            let x = index % SCREEN_WIDTH;
            let y = index / SCREEN_WIDTH;

            self.fill_rect(
                origin_x + (x * scale) as i32,
                origin_y + (y * scale) as i32,
                scale,
                scale,
                if lit { on } else { off },
            );
        }
    }

    /// Uploads the pixel buffer and flips the window.
    ///
    /// # Errors
    ///
    /// Returns an error when SDL cannot lock or copy the streaming texture.
    pub fn present(&mut self) -> Result<()> {
        // Destructured so the closure borrows the buffer and the texture
        // separately.
        let Self {
            texture,
            pixels,
            width,
            ..
        } = self;

        texture
            .with_lock(None, |bytes: &mut [u8], pitch: usize| {
                // Row by row, because the texture is allowed to be padded.
                for (row, target) in pixels.chunks_exact(*width).zip(bytes.chunks_mut(pitch)) {
                    for (pixel, color) in target.chunks_exact_mut(4).zip(row) {
                        pixel.copy_from_slice(&color.to_le_bytes());
                    }
                }
            })
            .map_err(Error::msg)
            .context("failed to lock the streaming texture")?;

        self.canvas.clear();
        self.canvas
            .copy(&self.texture, None, None)
            .map_err(Error::msg)?;
        self.canvas.present();

        Ok(())
    }
}
