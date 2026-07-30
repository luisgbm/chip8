//! Plays `roms/leap.ch8` to check the game in `programs/leap.asm` behaves.
//!
//! The register numbers and the geometry below are the ones the program
//! documents at the top of its source.

use chip9::cpu::{Chip9, SCREEN_WIDTH};
use chip9::programs::BUILTIN_PROGRAMS;

const KEY_LEFT: u8 = 4;
const KEY_RIGHT: u8 = 6;
const KEY_JUMP: u8 = 5;

const X: u8 = 2;
const STATE: u8 = 6;

const IN_AIR: u8 = 0;
const ON_FLOOR: u8 = 1;
const IN_PIT: u8 = 2;

const START_X: u8 = 4;
const FLOOR_Y: usize = 26;
const HOLE_X: usize = 28;
const HOLE_W: usize = 8;

struct Game {
    chip9: Chip9,
}

impl Game {
    fn new() -> Self {
        let program = BUILTIN_PROGRAMS
            .iter()
            .find(|program| program.name == "Leap")
            .expect("leap is bundled");

        let mut chip9 = Chip9::with_seed(7);
        chip9.load(program.rom).expect("it fits");

        let mut game = Self { chip9 };
        game.frames(1);
        game
    }

    fn hold(&mut self, key: Option<u8>) {
        for candidate in [KEY_LEFT, KEY_RIGHT, KEY_JUMP] {
            self.chip9
                .keypad_mut()
                .set_pressed(candidate, Some(candidate) == key);
        }
    }

    fn frames(&mut self, frames: u32) {
        for _ in 0..frames {
            self.chip9.step_many(60).expect("no fault");
            self.chip9.tick_timers();
        }
    }

    /// The game gives itself two frames per tick.
    fn ticks(&mut self, ticks: u32) {
        self.frames(2 * ticks);
    }

    fn x(&self) -> u8 {
        self.chip9.register(X)
    }

    fn state(&self) -> u8 {
        self.chip9.register(STATE)
    }

    fn row(&self, y: usize) -> Vec<bool> {
        (0..SCREEN_WIDTH).map(|x| self.chip9.pixel(x, y)).collect()
    }

    fn lit(&self) -> usize {
        self.chip9.framebuffer().iter().filter(|&&on| on).count()
    }

    /// The ledges either side of the pit, which nothing should ever rub out.
    fn ledges_are_solid(&self) -> bool {
        let hole = HOLE_X..HOLE_X + HOLE_W;

        (0..2).all(|offset| {
            let row = self.row(FLOOR_Y + offset);
            (0..SCREEN_WIDTH)
                .filter(|column| !hole.contains(column))
                .all(|column| row[column])
        })
    }

    /// Nothing at all in the pit, which only holds while the player is out of
    /// it.
    fn pit_is_open(&self) -> bool {
        (0..2).all(|offset| {
            let row = self.row(FLOOR_Y + offset);
            (HOLE_X..HOLE_X + HOLE_W).all(|column| !row[column])
        })
    }

    /// Walk right until the player is somewhere the pit can be jumped from.
    fn walk_to(&mut self, target: u8) {
        self.hold(Some(KEY_RIGHT));
        for _ in 0..80 {
            if self.x() >= target {
                return;
            }
            self.ticks(1);
        }
        panic!("never reached x = {target}, stuck at {}", self.x());
    }
}

#[test]
fn the_floor_has_a_pit_in_the_middle_of_it() {
    let game = Game::new();

    assert!(game.ledges_are_solid(), "the floor should be solid");
    assert!(
        game.pit_is_open(),
        "columns {HOLE_X}..{} should be missing",
        HOLE_X + HOLE_W
    );
}

#[test]
fn the_player_starts_standing_on_the_left() {
    let game = Game::new();

    assert_eq!(game.x(), START_X);
    assert_eq!(game.state(), ON_FLOOR);
}

#[test]
fn walking_moves_the_player_and_stops_at_the_wall() {
    let mut game = Game::new();

    game.hold(Some(KEY_RIGHT));
    game.ticks(5);
    assert_eq!(game.x(), START_X + 5);

    game.hold(Some(KEY_LEFT));
    game.ticks(20);
    assert_eq!(game.x(), 0, "the player should stop at the left wall");
}

#[test]
fn walking_into_the_pit_ends_the_game() {
    let mut game = Game::new();

    game.hold(Some(KEY_RIGHT));
    game.ticks(24);

    assert_eq!(game.state(), IN_PIT, "the player should have dropped in");
    assert!(
        (HOLE_X..HOLE_X + HOLE_W).contains(&usize::from(game.x())),
        "the player should be inside the pit, not at {}",
        game.x()
    );
    assert!(
        game.ledges_are_solid(),
        "falling past the lip should not rub any of the floor out"
    );

    // The pit cannot be walked out of, so the fall is fatal.
    game.ticks(8);
    assert_eq!(game.row(FLOOR_Y).iter().filter(|&&on| on).count(), 0);
    assert!(game.lit() > 60, "GAME OVER should be on screen");
    for row in 13..18 {
        assert!(game.row(row).iter().any(|&on| on), "row {row} is blank");
    }
}

#[test]
fn the_game_starts_again_by_itself() {
    let mut game = Game::new();

    game.hold(Some(KEY_RIGHT));
    game.ticks(32);
    assert!(game.lit() > 60, "expected the game over screen");

    game.hold(None);
    game.frames(185);

    assert_eq!(game.x(), START_X, "the player should be back at the start");
    assert_eq!(game.state(), ON_FLOOR);
    assert!(
        game.ledges_are_solid(),
        "the floor should have been redrawn"
    );
    assert!(game.pit_is_open(), "the pit should have been redrawn");
}

#[test]
fn a_well_timed_jump_clears_the_pit() {
    let mut game = Game::new();

    game.walk_to(22);

    game.hold(Some(KEY_JUMP));
    game.ticks(1);
    assert_eq!(game.state(), IN_AIR, "the jump should have started");

    game.hold(Some(KEY_RIGHT));
    for _ in 0..20 {
        game.ticks(1);
        if game.state() == ON_FLOOR {
            break;
        }
    }

    assert_eq!(game.state(), ON_FLOOR, "the player never landed");
    assert!(
        usize::from(game.x()) >= HOLE_X + HOLE_W,
        "the player landed at {} instead of past the pit",
        game.x()
    );
    assert!(game.ledges_are_solid());
    assert!(game.pit_is_open(), "the player should have cleared the pit");
}

#[test]
fn a_jump_that_is_left_too_late_falls_in() {
    let mut game = Game::new();

    game.walk_to(26);

    game.hold(Some(KEY_JUMP));
    game.ticks(1);
    assert_ne!(game.state(), IN_AIR, "there is nothing left to jump from");

    game.hold(Some(KEY_RIGHT));
    game.ticks(10);

    assert_eq!(game.state(), IN_PIT);
}

#[test]
fn the_player_cannot_jump_while_falling() {
    let mut game = Game::new();

    game.hold(Some(KEY_RIGHT));
    game.ticks(24);
    assert_eq!(game.state(), IN_PIT);

    let depth = game.chip9.register(4);
    game.hold(Some(KEY_JUMP));
    game.ticks(1);

    assert_eq!(game.state(), IN_PIT, "a second jump should not be allowed");
    assert!(
        game.chip9.register(4) > depth,
        "the player should still be on the way down"
    );
}
