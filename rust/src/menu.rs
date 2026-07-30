//! The intro screen: pick one of the bundled programs, or load one from disk.
//!
//! The JavaScript version had no interface at all — the program was an array
//! literal inside `js/index.html`, and running anything else meant editing the
//! page. Here every program from `js/programs.txt` is on a list, and a file can
//! be typed in, pasted or dropped onto the window.

use std::path::PathBuf;

use sdl2::event::Event;
use sdl2::keyboard::Keycode;

use crate::programs::{BuiltinProgram, BUILTIN_PROGRAMS};
use crate::theme;
use crate::video::{Video, WINDOW_WIDTH};

/// What the menu wants the application to do after an event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuAction {
    /// Nothing to do.
    None,
    /// Start the given program.
    Launch(Launch),
    /// Close the application.
    Quit,
}

/// A program the user chose to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Launch {
    /// One of the programs compiled into the executable.
    Builtin(usize),
    /// A `.ch8` file on disk.
    File(PathBuf),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    List,
    PathEntry,
}

const MARGIN: i32 = 64;
const TITLE_SCALE: usize = 6;
const BODY_SCALE: usize = 2;
const ROW_HEIGHT: i32 = 24;
const VISIBLE_ROWS: usize = 11;

const TITLE_Y: i32 = 26;
const SUBTITLE_Y: i32 = 80;
const LIST_Y: i32 = 120;
const INFO_Y: i32 = LIST_Y + VISIBLE_ROWS as i32 * ROW_HEIGHT + 34;
const FOOTER_Y: i32 = INFO_Y + 96;

/// The last row of the list, which opens the file prompt instead of a program.
const FILE_ENTRY_LABEL: &str = "Load a program from a file...";

/// The intro screen.
pub struct Menu {
    selected: usize,
    scroll: usize,
    mode: Mode,
    path: String,
    status: Option<String>,
    frame: u32,
}

impl Default for Menu {
    fn default() -> Self {
        Self::new()
    }
}

impl Menu {
    #[must_use]
    pub fn new() -> Self {
        Self {
            selected: 0,
            scroll: 0,
            mode: Mode::List,
            path: String::new(),
            status: None,
            frame: 0,
        }
    }

    /// Number of rows, the bundled programs plus the file prompt.
    fn entry_count() -> usize {
        BUILTIN_PROGRAMS.len() + 1
    }

    fn file_entry_index() -> usize {
        BUILTIN_PROGRAMS.len()
    }

    /// Shows a message under the list, in the fault color.
    pub fn set_status(&mut self, message: impl Into<String>) {
        self.status = Some(message.into());
    }

    pub fn clear_status(&mut self) {
        self.status = None;
    }

    /// Drives the caret blink; call once a frame.
    pub fn tick(&mut self) {
        self.frame = self.frame.wrapping_add(1);
    }

    /// Appends text typed or pasted into the file prompt.
    pub fn insert_text(&mut self, text: &str) {
        if self.mode == Mode::PathEntry {
            self.path
                .extend(text.chars().filter(|character| !character.is_control()));
        }
    }

    /// Whether the file prompt is open, so the application knows whether a
    /// paste shortcut should be honoured.
    #[must_use]
    pub fn is_prompting_for_path(&self) -> bool {
        self.mode == Mode::PathEntry
    }

    pub fn handle_event(&mut self, event: &Event) -> MenuAction {
        match *event {
            Event::Quit { .. } => MenuAction::Quit,
            Event::KeyDown {
                keycode: Some(keycode),
                ..
            } => match self.mode {
                Mode::List => self.handle_list_key(keycode),
                Mode::PathEntry => self.handle_path_key(keycode),
            },
            Event::TextInput { ref text, .. } => {
                self.insert_text(text);
                MenuAction::None
            }
            _ => MenuAction::None,
        }
    }

    fn handle_list_key(&mut self, keycode: Keycode) -> MenuAction {
        match keycode {
            Keycode::Escape => return MenuAction::Quit,
            Keycode::Up => self.move_selection(-1),
            Keycode::Down => self.move_selection(1),
            Keycode::PageUp => self.move_selection(-(VISIBLE_ROWS as i32)),
            Keycode::PageDown => self.move_selection(VISIBLE_ROWS as i32),
            Keycode::Home => self.select(0),
            Keycode::End => self.select(Self::entry_count() - 1),
            Keycode::Return | Keycode::Return2 | Keycode::KpEnter | Keycode::Space => {
                return self.activate();
            }
            _ => {}
        }

        MenuAction::None
    }

    fn handle_path_key(&mut self, keycode: Keycode) -> MenuAction {
        match keycode {
            Keycode::Escape => {
                self.mode = Mode::List;
                self.status = None;
            }
            Keycode::Backspace => {
                self.path.pop();
            }
            Keycode::Return | Keycode::Return2 | Keycode::KpEnter => {
                let path = clean_path(&self.path);

                if path.is_empty() {
                    self.status = Some("Type the path to a .ch8 file".to_owned());
                } else {
                    return MenuAction::Launch(Launch::File(PathBuf::from(path)));
                }
            }
            _ => {}
        }

        MenuAction::None
    }

    fn activate(&mut self) -> MenuAction {
        self.status = None;

        if self.selected == Self::file_entry_index() {
            self.mode = Mode::PathEntry;
            return MenuAction::None;
        }

        MenuAction::Launch(Launch::Builtin(self.selected))
    }

    fn move_selection(&mut self, delta: i32) {
        let last = Self::entry_count() as i32 - 1;
        let selected = (self.selected as i32 + delta).clamp(0, last);
        self.select(selected as usize);
    }

    fn select(&mut self, index: usize) {
        self.selected = index.min(Self::entry_count() - 1);
        self.status = None;

        if self.selected < self.scroll {
            self.scroll = self.selected;
        } else if self.selected >= self.scroll + VISIBLE_ROWS {
            self.scroll = self.selected - VISIBLE_ROWS + 1;
        }
    }

    /// The program the cursor is on, if it is not on the file prompt.
    #[must_use]
    pub fn selected_program(&self) -> Option<&'static BuiltinProgram> {
        BUILTIN_PROGRAMS.get(self.selected)
    }

    pub fn render(&self, video: &mut Video) {
        video.clear(theme::BACKGROUND);

        video.draw_text_centered(TITLE_Y, "CHIP-8", TITLE_SCALE, theme::ACCENT);
        video.draw_text_centered(
            SUBTITLE_Y,
            "a chip-8 interpreter in rust",
            BODY_SCALE,
            theme::DIM,
        );

        self.render_list(video);
        self.render_info(video);
        Self::render_footer(video);

        if self.mode == Mode::PathEntry {
            self.render_path_prompt(video);
        }
    }

    fn render_list(&self, video: &mut Video) {
        let right = WINDOW_WIDTH as i32 - MARGIN;

        for row in 0..VISIBLE_ROWS {
            let index = self.scroll + row;

            if index >= Self::entry_count() {
                break;
            }

            let y = LIST_Y + row as i32 * ROW_HEIGHT;
            let selected = index == self.selected;

            if selected {
                video.fill_rect(
                    MARGIN - 16,
                    y - 5,
                    (WINDOW_WIDTH as i32 - 2 * MARGIN + 32) as usize,
                    ROW_HEIGHT as usize,
                    theme::PANEL,
                );
            }

            let color = if selected {
                theme::HIGHLIGHT
            } else {
                theme::TEXT
            };

            if selected {
                video.draw_text(MARGIN - 8, y, ">", BODY_SCALE, theme::HIGHLIGHT);
            }

            match BUILTIN_PROGRAMS.get(index) {
                Some(program) => {
                    video.draw_text(MARGIN + 16, y, program.name, BODY_SCALE, color);
                    video.draw_text_right(
                        right - 80,
                        y,
                        &format!("{} B", program.rom.len()),
                        BODY_SCALE,
                        theme::DIM,
                    );
                    video.draw_text_right(
                        right,
                        y,
                        program.category.label(),
                        BODY_SCALE,
                        theme::DIM,
                    );
                }
                None => {
                    video.draw_text(MARGIN + 16, y, FILE_ENTRY_LABEL, BODY_SCALE, color);
                    video.draw_text_right(right, y, "FILE", BODY_SCALE, theme::DIM);
                }
            }
        }

        let hidden = Self::entry_count().saturating_sub(self.scroll + VISIBLE_ROWS);

        if hidden > 0 {
            video.draw_text_right(
                right,
                LIST_Y + VISIBLE_ROWS as i32 * ROW_HEIGHT,
                &format!("{hidden} more below"),
                BODY_SCALE,
                theme::DIM,
            );
        }
    }

    fn render_info(&self, video: &mut Video) {
        video.fill_rect(
            MARGIN - 16,
            INFO_Y - 16,
            WINDOW_WIDTH - 2 * MARGIN as usize + 32,
            1,
            theme::SEPARATOR,
        );

        let line_height = Video::line_height(BODY_SCALE) as i32 + 8;

        match self.selected_program() {
            Some(program) => {
                video.draw_text(MARGIN, INFO_Y, program.description, BODY_SCALE, theme::TEXT);

                if let Some(controls) = program.controls {
                    video.draw_text(
                        MARGIN,
                        INFO_Y + line_height,
                        controls,
                        BODY_SCALE,
                        theme::DIM,
                    );
                }
            }
            None => {
                video.draw_text(
                    MARGIN,
                    INFO_Y,
                    "Run a program stored on disk, of any size up to 3584 bytes.",
                    BODY_SCALE,
                    theme::TEXT,
                );
                video.draw_text(
                    MARGIN,
                    INFO_Y + line_height,
                    "You can also drop a file onto this window at any time.",
                    BODY_SCALE,
                    theme::DIM,
                );
            }
        }

        if let Some(status) = &self.status {
            video.draw_text(
                MARGIN,
                INFO_Y + 2 * line_height,
                status,
                BODY_SCALE,
                theme::ERROR,
            );
        }
    }

    fn render_footer(video: &mut Video) {
        let line_height = Video::line_height(BODY_SCALE) as i32 + 8;

        video.draw_text(
            MARGIN,
            FOOTER_Y,
            "UP/DOWN select    ENTER run    ESC quit",
            BODY_SCALE,
            theme::DIM,
        );
        video.draw_text(
            MARGIN,
            FOOTER_Y + line_height,
            "Keypad: 1 2 3 4 / Q W E R / A S D F / Z X C V",
            BODY_SCALE,
            theme::DIM,
        );
    }

    fn render_path_prompt(&self, video: &mut Video) {
        let width = WINDOW_WIDTH - 2 * MARGIN as usize;
        let height = 168;
        let x = MARGIN;
        let y = (video.height() as i32 - height) / 2;

        video.fill_rect(
            x - 4,
            y - 4,
            width + 8,
            height as usize + 8,
            theme::SEPARATOR,
        );
        video.fill_rect(x, y, width, height as usize, theme::PANEL);

        let line_height = Video::line_height(BODY_SCALE) as i32 + 8;
        let inner = x + 24;

        video.draw_text(
            inner,
            y + 20,
            "Load a program from a file",
            BODY_SCALE,
            theme::ACCENT,
        );
        video.draw_text(
            inner,
            y + 20 + line_height,
            "Type or paste a path, then press ENTER",
            BODY_SCALE,
            theme::DIM,
        );

        let field_y = y + 24 + 2 * line_height;
        video.fill_rect(inner, field_y - 6, width - 48, 28, theme::BACKGROUND);

        // Keep the tail of the path visible once it outgrows the field.
        let visible = ((width - 64) / (crate::font::ADVANCE * BODY_SCALE)).max(1);
        let text: String = if self.path.chars().count() > visible {
            self.path
                .chars()
                .skip(self.path.chars().count() - visible)
                .collect()
        } else {
            self.path.clone()
        };

        let caret_x = video.draw_text(inner + 8, field_y, &text, BODY_SCALE, theme::TEXT);

        if self.frame % 60 < 30 {
            video.fill_rect(
                caret_x + 4,
                field_y,
                BODY_SCALE * 5,
                Video::line_height(BODY_SCALE),
                theme::HIGHLIGHT,
            );
        }

        let hint_y = field_y + 2 * line_height;
        video.draw_text(
            inner,
            hint_y,
            "ENTER load    ESC cancel    CTRL+V paste    BACKSPACE erase",
            BODY_SCALE,
            theme::DIM,
        );

        if let Some(status) = &self.status {
            video.draw_text(
                inner,
                hint_y + line_height,
                status,
                BODY_SCALE,
                theme::ERROR,
            );
        }
    }
}

/// Strips the whitespace and quotes that come with a pasted Windows path.
fn clean_path(path: &str) -> &str {
    path.trim().trim_matches('"').trim()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(keycode: Keycode) -> Event {
        Event::KeyDown {
            timestamp: 0,
            window_id: 0,
            keycode: Some(keycode),
            scancode: None,
            keymod: sdl2::keyboard::Mod::NOMOD,
            repeat: false,
        }
    }

    #[test]
    fn a_pasted_path_loses_its_quotes_and_whitespace() {
        assert_eq!(
            clean_path("  \"C:\\roms\\pong.ch8\" \n"),
            "C:\\roms\\pong.ch8"
        );
        assert_eq!(clean_path("pong.ch8"), "pong.ch8");
        assert_eq!(clean_path("   "), "");
    }

    #[test]
    fn the_selection_stops_at_both_ends_of_the_list() {
        let mut menu = Menu::new();
        assert_eq!(menu.selected, 0);

        menu.handle_event(&key(Keycode::Up));
        assert_eq!(menu.selected, 0, "the first row cannot be passed");

        menu.handle_event(&key(Keycode::End));
        assert_eq!(menu.selected, Menu::entry_count() - 1);

        menu.handle_event(&key(Keycode::Down));
        assert_eq!(
            menu.selected,
            Menu::entry_count() - 1,
            "the last row cannot be passed"
        );

        menu.handle_event(&key(Keycode::Home));
        assert_eq!(menu.selected, 0);
    }

    #[test]
    fn the_list_scrolls_to_keep_the_selection_visible() {
        let mut menu = Menu::new();
        menu.handle_event(&key(Keycode::End));

        assert!(menu.selected >= menu.scroll);
        assert!(menu.selected < menu.scroll + VISIBLE_ROWS);
    }

    #[test]
    fn enter_runs_the_selected_program() {
        let mut menu = Menu::new();

        assert_eq!(
            menu.handle_event(&key(Keycode::Return)),
            MenuAction::Launch(Launch::Builtin(0))
        );
    }

    #[test]
    fn the_last_row_opens_the_file_prompt() {
        let mut menu = Menu::new();
        menu.handle_event(&key(Keycode::End));

        assert_eq!(menu.handle_event(&key(Keycode::Return)), MenuAction::None);
        assert!(menu.is_prompting_for_path());

        menu.insert_text("roms/pong.ch8");
        let action = menu.handle_event(&key(Keycode::Return));

        assert_eq!(
            action,
            MenuAction::Launch(Launch::File(PathBuf::from("roms/pong.ch8")))
        );
    }

    #[test]
    fn an_empty_path_is_refused_instead_of_launching_nothing() {
        let mut menu = Menu::new();
        menu.handle_event(&key(Keycode::End));
        menu.handle_event(&key(Keycode::Return));

        assert_eq!(menu.handle_event(&key(Keycode::Return)), MenuAction::None);
        assert!(menu.status.is_some());
        assert!(menu.is_prompting_for_path(), "the prompt stays open");
    }

    #[test]
    fn escape_backs_out_of_the_prompt_and_then_quits() {
        let mut menu = Menu::new();
        menu.handle_event(&key(Keycode::End));
        menu.handle_event(&key(Keycode::Return));

        assert_eq!(menu.handle_event(&key(Keycode::Escape)), MenuAction::None);
        assert!(!menu.is_prompting_for_path());

        assert_eq!(menu.handle_event(&key(Keycode::Escape)), MenuAction::Quit);
    }

    #[test]
    fn typing_only_reaches_the_path_prompt() {
        let mut menu = Menu::new();
        menu.insert_text("ignored");
        assert!(menu.path.is_empty());

        menu.handle_event(&key(Keycode::End));
        menu.handle_event(&key(Keycode::Return));
        menu.insert_text("pong.ch8\r\n");

        assert_eq!(menu.path, "pong.ch8", "control characters are dropped");

        menu.handle_event(&key(Keycode::Backspace));
        assert_eq!(menu.path, "pong.ch");
    }
}
