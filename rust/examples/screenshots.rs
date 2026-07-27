//! Renders the intro screen and a running program, and writes PNGs.
//!
//! Handy when there is no display to run on: it draws through the same
//! [`Video`] buffer the application does, so what lands in `screenshots/` is
//! exactly what you would see on screen.
//!
//! ```text
//! cargo run --release --example screenshots
//! ```
//!
//! SDL still has to initialise, but a hidden window and the software renderer
//! are enough — set `SDL_VIDEODRIVER=dummy` if there is no display server.

use std::path::Path;

use anyhow::{Context, Error, Result};
use sdl2::event::Event;
use sdl2::keyboard::{Keycode, Mod};

use chip8::cpu::Chip8;
use chip8::menu::Menu;
use chip8::programs::BUILTIN_PROGRAMS;
use chip8::theme;
use chip8::video::{Video, PIXEL_SIZE, WINDOW_HEIGHT, WINDOW_WIDTH};

/// How long each program is run before its picture is taken, in frames.
const SETTLE_FRAMES: u32 = 90;

/// The programs worth a picture, and the key held down while they run.
const SHOTS: &[(&str, Option<u8>)] = &[
    ("IBM Logo", None),
    ("Space Invaders", Some(0x5)),
    ("Pong", None),
    ("Computer", None),
];

fn main() -> Result<()> {
    let output = Path::new(env!("CARGO_MANIFEST_DIR")).join("screenshots");
    std::fs::create_dir_all(&output)
        .with_context(|| format!("failed to create `{}`", output.display()))?;

    let sdl_context = sdl2::init().map_err(Error::msg)?;
    let video_subsystem = sdl_context.video().map_err(Error::msg)?;

    let window = video_subsystem
        .window("screenshots", WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32)
        .hidden()
        .build()?;

    let canvas = window.into_canvas().software().build()?;
    let texture_creator = canvas.texture_creator();
    let mut video = Video::new(canvas, &texture_creator, WINDOW_WIDTH, WINDOW_HEIGHT)?;

    let mut menu = Menu::new();
    menu.render(&mut video);
    save(&video, &output.join("menu.png"))?;

    // The last row of the list is the file prompt.
    press(&mut menu, Keycode::End);
    press(&mut menu, Keycode::Return);
    menu.insert_text("roms/pong.ch8");
    menu.render(&mut video);
    save(&video, &output.join("menu-file.png"))?;

    for &(name, key) in SHOTS {
        let program = BUILTIN_PROGRAMS
            .iter()
            .find(|program| program.name == name)
            .with_context(|| format!("no program called `{name}`"))?;

        let mut chip8 = Chip8::with_seed(0xC81C_8C81_C8C8_1C81);
        chip8.load(program.rom)?;

        if let Some(key) = key {
            chip8.keypad_mut().set_pressed(key, true);
        }

        for _ in 0..SETTLE_FRAMES {
            // Programs that run off the end of their code are still worth a
            // picture of what they drew.
            if chip8.step_many(program.cycles_per_frame).is_err() {
                break;
            }

            chip8.tick_timers();
        }

        video.clear(theme::BACKGROUND);
        video.draw_chip8_screen(
            chip8.framebuffer(),
            0,
            0,
            PIXEL_SIZE,
            theme::SCREEN_ON,
            theme::SCREEN_OFF,
        );

        save(&video, &output.join(format!("{}.png", slug(name))))?;
    }

    Ok(())
}

/// Feeds a key press to the menu, the way the event loop would.
fn press(menu: &mut Menu, keycode: Keycode) {
    menu.handle_event(&Event::KeyDown {
        timestamp: 0,
        window_id: 0,
        keycode: Some(keycode),
        scancode: None,
        keymod: Mod::NOMOD,
        repeat: false,
    });
}

fn slug(name: &str) -> String {
    name.to_lowercase().replace(' ', "-")
}

fn save(video: &Video, path: &Path) -> Result<()> {
    let mut rgb = Vec::with_capacity(video.pixels().len() * 3);

    for &pixel in video.pixels() {
        rgb.push(((pixel >> 16) & 0xFF) as u8);
        rgb.push(((pixel >> 8) & 0xFF) as u8);
        rgb.push((pixel & 0xFF) as u8);
    }

    image::save_buffer(
        path,
        &rgb,
        video.width() as u32,
        video.height() as u32,
        image::ExtendedColorType::Rgb8,
    )
    .with_context(|| format!("failed to write `{}`", path.display()))?;

    println!("wrote {}", path.display());

    Ok(())
}
