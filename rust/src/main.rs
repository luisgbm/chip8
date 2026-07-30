//! Entry point: the intro screen, the emulator loop and the glue between the
//! interpreter and SDL2. Ported from the inline script in `js/index.html`.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Error, Result};
use sdl2::event::Event;
use sdl2::keyboard::{Keycode, Mod};
use sdl2::{EventPump, TimerSubsystem};

use chip8::audio::Beeper;
use chip8::cpu::{Chip8, Fault, MAX_PROGRAM_SIZE};
use chip8::keypad::Keypad;
use chip8::menu::{Launch, Menu, MenuAction};
use chip8::programs::{BuiltinProgram, BUILTIN_PROGRAMS};
use chip8::theme;
use chip8::video::{
    Video, PIXEL_SIZE, SCREEN_AREA_HEIGHT, STATUS_BAR_HEIGHT, WINDOW_HEIGHT, WINDOW_WIDTH,
};

const FPS: u32 = 60;

/// How much emulated time one [`App::update`] covers, in seconds.
const TIMESTEP: f64 = 1.0 / FPS as f64;

/// The longest stretch of real time a single frame is allowed to make up for.
/// Without this, dragging the window or hitting a breakpoint would make the
/// interpreter sprint to catch up.
const MAX_CATCH_UP: f64 = 0.25;

const MIN_CYCLES_PER_FRAME: u32 = 1;
const MAX_CYCLES_PER_FRAME: u32 = 100;

const STATUS_SCALE: usize = 2;

fn main() -> Result<()> {
    let sdl_context = sdl2::init().map_err(Error::msg)?;
    let video_subsystem = sdl_context.video().map_err(Error::msg)?;
    let audio_subsystem = sdl_context.audio().map_err(Error::msg)?;
    let mut timer_subsystem = sdl_context.timer().map_err(Error::msg)?;
    let mut event_pump = sdl_context.event_pump().map_err(Error::msg)?;

    let window = video_subsystem
        .window("CHIP-8", WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32)
        .position_centered()
        .build()
        .context("failed to create the SDL window")?;

    let canvas = window
        .into_canvas()
        // No vsync: the loop below paces itself, which keeps the interpreter at
        // 60 Hz whatever the display refresh rate happens to be.
        .build()
        .context("failed to create the SDL renderer")?;

    // The streaming texture borrows from the creator, so the creator has to
    // outlive the `Video` that owns it.
    let texture_creator = canvas.texture_creator();
    let video = Video::new(canvas, &texture_creator, WINDOW_WIDTH, WINDOW_HEIGHT)?;
    let beeper = Beeper::new(&audio_subsystem)?;

    let mut app = App::new(video, beeper, video_subsystem.clipboard());

    // A path on the command line skips the intro screen.
    if let Some(path) = std::env::args_os().nth(1) {
        app.launch_file(&PathBuf::from(path));
    }

    app.run(&mut event_pump, &mut timer_subsystem)
}

/// What the application is currently showing.
enum State {
    Menu(Menu),
    Session(Box<Session>),
}

/// A program that has been loaded and is running.
struct Session {
    chip8: Chip8,
    title: String,
    source: String,
    cycles_per_frame: u32,
    paused: bool,
    /// The fault that stopped the program, if any.
    fault: Option<Fault>,
    /// Set when the whole window has to be repainted, not just the status bar.
    needs_full_redraw: bool,
}

impl Session {
    fn new(title: String, source: String, rom: &[u8], cycles_per_frame: u32) -> Result<Self> {
        let mut chip8 = Chip8::new();
        chip8.load(rom)?;

        Ok(Self {
            chip8,
            title,
            source,
            cycles_per_frame,
            paused: false,
            fault: None,
            needs_full_redraw: true,
        })
    }

    fn from_builtin(program: &BuiltinProgram) -> Result<Self> {
        Self::new(
            program.name.to_owned(),
            format!("built-in, {} bytes", program.rom.len()),
            program.rom,
            program.cycles_per_frame,
        )
    }

    fn from_file(path: &Path) -> Result<Self> {
        let rom = fs::read(path).with_context(|| format!("could not read {}", path.display()))?;

        if rom.len() > MAX_PROGRAM_SIZE {
            anyhow::bail!(
                "{} is {} bytes, but only {MAX_PROGRAM_SIZE} bytes fit in memory",
                path.display(),
                rom.len()
            );
        }

        let title = path.file_stem().map_or_else(
            || "program".to_owned(),
            |stem| stem.to_string_lossy().into_owned(),
        );

        Self::new(
            title,
            format!("{}, {} bytes", path.display(), rom.len()),
            &rom,
            chip8::programs::DEFAULT_CYCLES_PER_FRAME,
        )
    }

    fn reset(&mut self) {
        self.chip8.reset();
        self.fault = None;
        self.paused = false;
        self.needs_full_redraw = true;
    }

    fn set_speed(&mut self, cycles_per_frame: u32) {
        self.cycles_per_frame = cycles_per_frame.clamp(MIN_CYCLES_PER_FRAME, MAX_CYCLES_PER_FRAME);
    }

    fn is_stopped(&self) -> bool {
        self.paused || self.fault.is_some()
    }

    /// Runs one frame's worth of instructions and one timer tick.
    fn update(&mut self) {
        if self.is_stopped() {
            return;
        }

        if let Err(fault) = self.chip8.step_many(self.cycles_per_frame) {
            self.fault = Some(fault);
            return;
        }

        self.chip8.tick_timers();
    }
}

struct App<'a> {
    video: Video<'a>,
    beeper: Beeper,
    clipboard: sdl2::clipboard::ClipboardUtil,
    state: State,
    running: bool,
}

impl<'a> App<'a> {
    fn new(video: Video<'a>, beeper: Beeper, clipboard: sdl2::clipboard::ClipboardUtil) -> Self {
        Self {
            video,
            beeper,
            clipboard,
            state: State::Menu(Menu::new()),
            running: true,
        }
    }

    /// Runs a fixed-timestep loop: the interpreter always advances at
    /// [`FPS`] steps per second, however often the display happens to refresh.
    fn run(&mut self, event_pump: &mut EventPump, timer: &mut TimerSubsystem) -> Result<()> {
        let frequency = timer.performance_frequency() as f64;
        let mut previous = timer.performance_counter();
        let mut accumulator = 0.0;

        while self.running {
            let frame_start = timer.performance_counter();
            let elapsed = (frame_start - previous) as f64 / frequency;
            previous = frame_start;
            accumulator += elapsed.min(MAX_CATCH_UP);

            for event in event_pump.poll_iter() {
                self.handle_event(&event);
            }

            while accumulator >= TIMESTEP {
                accumulator -= TIMESTEP;
                self.update();
            }

            self.render()?;

            // Sleep out the rest of the frame instead of spinning. Rounding up
            // keeps the next pass over the loop past the timestep, so it runs
            // exactly one update rather than alternating between none and two.
            let work = (timer.performance_counter() - frame_start) as f64 / frequency;
            let remaining = (TIMESTEP - accumulator - work) * 1000.0;

            if remaining >= 1.0 {
                timer.delay(remaining.ceil() as u32);
            }
        }

        self.beeper.set_playing(false);

        Ok(())
    }

    fn handle_event(&mut self, event: &Event) {
        // Dropping a file works from anywhere, menu or emulator.
        if let Event::DropFile { filename, .. } = event {
            self.launch_file(&PathBuf::from(filename));
            return;
        }

        match &mut self.state {
            State::Menu(menu) => {
                if is_paste_shortcut(event) && menu.is_prompting_for_path() {
                    if let Ok(text) = self.clipboard.clipboard_text() {
                        menu.insert_text(&text);
                    }

                    return;
                }

                match menu.handle_event(event) {
                    MenuAction::None => {}
                    MenuAction::Quit => self.running = false,
                    MenuAction::Launch(Launch::Builtin(index)) => {
                        if let Some(program) = BUILTIN_PROGRAMS.get(index) {
                            self.start(Session::from_builtin(program));
                        }
                    }
                    MenuAction::Launch(Launch::File(path)) => self.launch_file(&path),
                }
            }
            State::Session(session) => match *event {
                Event::Quit { .. } => self.running = false,
                Event::KeyDown {
                    keycode: Some(keycode),
                    repeat: false,
                    ..
                } => match keycode {
                    Keycode::Escape => self.show_menu(),
                    Keycode::F5 => session.reset(),
                    Keycode::Space => session.paused = !session.paused,
                    Keycode::Minus | Keycode::KpMinus => {
                        session.set_speed(session.cycles_per_frame.saturating_sub(1));
                    }
                    Keycode::Equals | Keycode::KpPlus => {
                        session.set_speed(session.cycles_per_frame + 1);
                    }
                    _ => {
                        if let Some(key) = Keypad::key_for(keycode) {
                            session.chip8.keypad_mut().set_pressed(key, true);
                        }
                    }
                },
                Event::KeyUp {
                    keycode: Some(keycode),
                    ..
                } => {
                    if let Some(key) = Keypad::key_for(keycode) {
                        session.chip8.keypad_mut().set_pressed(key, false);
                    }
                }
                // A program should not keep hearing a key that was held while
                // the window lost focus.
                Event::Window {
                    win_event: sdl2::event::WindowEvent::FocusLost,
                    ..
                } => {
                    session.chip8.keypad_mut().reset();
                }
                _ => {}
            },
        }
    }

    /// Loads a file, falling back to the menu with a message if it cannot be
    /// read.
    fn launch_file(&mut self, path: &Path) {
        match Session::from_file(path) {
            Ok(session) => self.start(Ok(session)),
            Err(error) => {
                let message = format!("{error:#}");
                eprintln!("chip8: {message}");
                self.show_menu();

                if let State::Menu(menu) = &mut self.state {
                    menu.set_status(message);
                }
            }
        }
    }

    fn start(&mut self, session: Result<Session>) {
        match session {
            Ok(session) => self.state = State::Session(Box::new(session)),
            Err(error) => {
                let message = format!("{error:#}");
                eprintln!("chip8: {message}");

                if let State::Menu(menu) = &mut self.state {
                    menu.set_status(message);
                }
            }
        }
    }

    fn show_menu(&mut self) {
        self.beeper.set_playing(false);
        self.state = State::Menu(Menu::new());
    }

    fn update(&mut self) {
        match &mut self.state {
            State::Menu(menu) => {
                menu.tick();
                self.beeper.set_playing(false);
            }
            State::Session(session) => {
                session.update();
                self.beeper
                    .set_playing(!session.is_stopped() && session.chip8.is_beeping());
            }
        }
    }

    fn render(&mut self) -> Result<()> {
        match &mut self.state {
            State::Menu(menu) => menu.render(&mut self.video),
            State::Session(session) => render_session(&mut self.video, session),
        }

        self.video.present()
    }
}

/// Draws the emulator screen and the status bar under it.
fn render_session(video: &mut Video, session: &mut Session) {
    if session.chip8.take_redraw() || session.needs_full_redraw {
        video.draw_chip8_screen(
            session.chip8.framebuffer(),
            0,
            0,
            PIXEL_SIZE,
            theme::SCREEN_ON,
            theme::SCREEN_OFF,
        );

        session.needs_full_redraw = false;
    }

    let bar_y = SCREEN_AREA_HEIGHT as i32;
    video.fill_rect(0, bar_y, WINDOW_WIDTH, STATUS_BAR_HEIGHT, theme::BACKGROUND);
    video.fill_rect(0, bar_y, WINDOW_WIDTH, 2, theme::SEPARATOR);

    let margin = 16;
    let right = WINDOW_WIDTH as i32 - margin;
    let first_line = bar_y + 14;
    let second_line = bar_y + 38;

    let name_end = video.draw_text(
        margin,
        first_line,
        &session.title,
        STATUS_SCALE,
        theme::ACCENT,
    );
    video.draw_text(
        name_end + 24,
        first_line,
        &session.source,
        STATUS_SCALE,
        theme::DIM,
    );

    let speed = format!(
        "{} cy/frame ~ {} Hz",
        session.cycles_per_frame,
        session.cycles_per_frame * FPS
    );
    video.draw_text_right(right, first_line, &speed, STATUS_SCALE, theme::DIM);

    video.draw_text(
        margin,
        second_line,
        "ESC menu    F5 reset    SPACE pause    -/= speed",
        STATUS_SCALE,
        theme::DIM,
    );

    if session.fault.is_some() {
        video.draw_text_right(right, second_line, "STOPPED", STATUS_SCALE, theme::ERROR);
    } else if session.paused {
        video.draw_text_right(right, second_line, "PAUSED", STATUS_SCALE, theme::HIGHLIGHT);
    } else if session.chip8.is_awaiting_key() {
        video.draw_text_right(
            right,
            second_line,
            "WAITING FOR A KEY",
            STATUS_SCALE,
            theme::DIM,
        );
    }

    if let Some(fault) = session.fault {
        render_fault(video, fault);
        session.needs_full_redraw = true;
    }
}

/// Draws the panel that explains why a program stopped.
fn render_fault(video: &mut Video, fault: Fault) {
    let width = WINDOW_WIDTH - 128;
    let height = 132;
    let x = 64;
    let y = (SCREEN_AREA_HEIGHT as i32 - height) / 2;

    video.fill_rect(x - 4, y - 4, width + 8, height as usize + 8, theme::ERROR);
    video.fill_rect(x, y, width, height as usize, theme::BACKGROUND);

    let line_height = Video::line_height(STATUS_SCALE) as i32 + 10;

    video.draw_text(
        x + 24,
        y + 24,
        "The program stopped",
        STATUS_SCALE,
        theme::ERROR,
    );
    video.draw_text(
        x + 24,
        y + 24 + line_height,
        &fault.to_string(),
        STATUS_SCALE,
        theme::TEXT,
    );
    video.draw_text(
        x + 24,
        y + 24 + 2 * line_height,
        fault.hint(),
        STATUS_SCALE,
        theme::DIM,
    );
    video.draw_text(
        x + 24,
        y + 24 + 4 * line_height,
        "F5 restarts it, ESC goes back to the menu",
        STATUS_SCALE,
        theme::DIM,
    );
}

fn is_paste_shortcut(event: &Event) -> bool {
    matches!(
        *event,
        Event::KeyDown { keycode: Some(Keycode::V), keymod, .. }
            if keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pong() -> &'static BuiltinProgram {
        BUILTIN_PROGRAMS
            .iter()
            .find(|program| program.name == "Pong")
            .expect("pong is bundled")
    }

    #[test]
    fn a_builtin_session_starts_ready_to_run() {
        let session = Session::from_builtin(pong()).expect("pong loads");

        assert_eq!(session.title, "Pong");
        assert_eq!(
            session.source,
            format!("built-in, {} bytes", pong().rom.len())
        );
        assert_eq!(session.cycles_per_frame, pong().cycles_per_frame);
        assert_eq!(session.chip8.program(), pong().rom);
        assert!(!session.paused);
        assert!(session.fault.is_none());
        assert!(!session.is_stopped());
    }

    #[test]
    fn a_session_from_a_file_is_named_after_it() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("roms/space_invaders.ch8");
        let session = Session::from_file(&path).expect("the file is there");

        assert_eq!(session.title, "space_invaders");
        assert!(session.source.contains("space_invaders.ch8"));
        assert_eq!(session.chip8.program().len(), 1296);
    }

    #[test]
    fn a_missing_file_is_reported_rather_than_crashing() {
        let Err(error) = Session::from_file(Path::new("no/such/program.ch8")) else {
            panic!("the file does not exist");
        };

        assert!(format!("{error:#}").contains("could not read"));
    }

    #[test]
    fn a_file_that_does_not_fit_in_memory_is_refused() {
        let path = std::env::temp_dir().join("chip8-too-large.ch8");
        fs::write(&path, vec![0; MAX_PROGRAM_SIZE + 1]).expect("the temporary file is writable");

        let Err(error) = Session::from_file(&path) else {
            panic!("it does not fit");
        };

        assert!(format!("{error:#}").contains("fit in memory"));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn a_stopped_session_does_not_run_and_can_be_restarted() {
        let mut session = Session::from_builtin(pong()).expect("pong loads");
        session.fault = Some(Fault::StackUnderflow { pc: 0x200 });

        assert!(session.is_stopped());

        let pc = session.chip8.pc();
        session.update();
        assert_eq!(session.chip8.pc(), pc, "a stopped program must not advance");

        session.reset();
        assert!(session.fault.is_none());
        assert!(!session.is_stopped());

        session.update();
        assert_ne!(session.chip8.pc(), pc);
    }

    #[test]
    fn pausing_freezes_the_program() {
        let mut session = Session::from_builtin(pong()).expect("pong loads");
        session.paused = true;

        let pc = session.chip8.pc();
        session.update();

        assert!(session.is_stopped());
        assert_eq!(session.chip8.pc(), pc);
    }

    #[test]
    fn the_speed_stays_inside_its_limits() {
        let mut session = Session::from_builtin(pong()).expect("pong loads");

        session.set_speed(0);
        assert_eq!(session.cycles_per_frame, MIN_CYCLES_PER_FRAME);

        session.set_speed(10_000);
        assert_eq!(session.cycles_per_frame, MAX_CYCLES_PER_FRAME);
    }

    #[test]
    fn only_ctrl_v_pastes() {
        let paste = Event::KeyDown {
            timestamp: 0,
            window_id: 0,
            keycode: Some(Keycode::V),
            scancode: None,
            keymod: Mod::LCTRLMOD,
            repeat: false,
        };
        let typing = Event::KeyDown {
            timestamp: 0,
            window_id: 0,
            keycode: Some(Keycode::V),
            scancode: None,
            keymod: Mod::NOMOD,
            repeat: false,
        };

        assert!(is_paste_shortcut(&paste));
        assert!(!is_paste_shortcut(&typing), "V is also a keypad key");
    }
}
