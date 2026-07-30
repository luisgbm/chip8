//! Checks that the ROMs built by the assembler match the source they came
//! from, so neither can be changed without the other.

use chip9::asm::assemble;

fn check(name: &str, source: &str, rom: &[u8]) {
    let assembly = assemble(source).unwrap_or_else(|error| panic!("{name}.asm: {error}"));

    assert_eq!(
        assembly.rom, rom,
        "roms/{name}.ch8 is out of date, re-run: \
         cargo run --bin asm -- programs/{name}.asm roms/{name}.ch8"
    );
}

#[test]
fn leap_matches_its_source() {
    check(
        "leap",
        include_str!("../programs/leap.asm"),
        include_bytes!("../roms/leap.ch8"),
    );
}

#[test]
fn abc123_matches_its_source() {
    check(
        "abc123",
        include_str!("../programs/abc123.asm"),
        include_bytes!("../roms/abc123.ch8"),
    );
}

#[test]
fn the_tutorial_program_draws_six_glyphs_and_stops() {
    use chip9::cpu::Chip9;

    let mut chip9 = Chip9::new();
    chip9
        .load(include_bytes!("../roms/abc123.ch8"))
        .expect("it fits");

    // Far longer than it needs, to prove it settles rather than runs away.
    chip9.step_many(500).expect("no fault");

    // Every glyph is five rows tall, on rows 13 to 17 and nowhere else.
    for row in 0..32 {
        let lit = (0..64).filter(|&column| chip9.pixel(column, row)).count();
        if (13..18).contains(&row) {
            assert!(lit > 0, "row {row} of the message is blank");
        } else {
            assert_eq!(lit, 0, "row {row} should be empty");
        }
    }

    // Six glyphs, four columns wide, one column apart, starting at 17.
    assert!(chip9.pixel(17, 13), "the A should start at column 17");
    assert!(!chip9.pixel(46, 13), "nothing should be drawn past the 3");
}
