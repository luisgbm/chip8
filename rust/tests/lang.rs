//! Checks the C8 compiler against the assembler.
//!
//! `programs/leap.c8` and `programs/leap.asm` are the same program written
//! twice, once in each language, and the same goes for `abc123`. Compiling one
//! and assembling the other has to produce the same bytes, which pins the
//! compiler to a reference written by hand and keeps the two copies of each
//! program from drifting apart.

use chip8::asm::assemble;
use chip8::lang::{compile, compile_to_assembly};

fn rom(source: &str) -> Vec<u8> {
    compile(source)
        .unwrap_or_else(|error| panic!("{error}\n"))
        .rom
}

/// The instructions a snippet compiles to, with the wrapping stripped off, so
/// a test can say what it means.
fn body(source: &str) -> Vec<String> {
    let assembly = compile_to_assembly(source).unwrap_or_else(|error| panic!("{error}\n"));

    assembly
        .lines()
        .map(str::trim)
        .filter(|line| {
            !line.is_empty()
                && !line.starts_with(';')
                && !line.starts_with("DB ")
                && !line.ends_with(':')
        })
        .map(str::to_owned)
        .collect()
}

fn error(source: &str) -> String {
    compile(source)
        .expect_err("this should not compile")
        .message
}

// -- the two programs, both ways ---------------------------------------------

#[test]
fn leap_compiles_to_the_committed_rom() {
    assert_eq!(
        rom(include_str!("../programs/leap.c8")),
        include_bytes!("../roms/leap.ch8"),
        "roms/leap.ch8 is out of date, re-run: \
         cargo run --bin c8c -- programs/leap.c8 roms/leap.ch8"
    );
}

#[test]
fn abc123_compiles_to_the_committed_rom() {
    assert_eq!(
        rom(include_str!("../programs/abc123.c8")),
        include_bytes!("../roms/abc123.ch8"),
        "roms/abc123.ch8 is out of date, re-run: \
         cargo run --bin c8c -- programs/abc123.c8 roms/abc123.ch8"
    );
}

#[test]
fn leap_compiles_to_the_same_bytes_the_assembler_makes() {
    let compiled = rom(include_str!("../programs/leap.c8"));
    let assembled = assemble(include_str!("../programs/leap.asm"))
        .expect("the reference assembles")
        .rom;

    assert_eq!(
        compiled, assembled,
        "programs/leap.c8 and programs/leap.asm have drifted apart"
    );
}

#[test]
fn abc123_compiles_to_the_same_bytes_the_assembler_makes() {
    let compiled = rom(include_str!("../programs/abc123.c8"));
    let assembled = assemble(include_str!("../programs/abc123.asm"))
        .expect("the reference assembles")
        .rom;

    assert_eq!(
        compiled, assembled,
        "programs/abc123.c8 and programs/abc123.asm have drifted apart"
    );
}

#[test]
fn the_compiled_game_still_plays() {
    use chip8::cpu::Chip8;

    let mut chip8 = Chip8::new();
    chip8
        .load(&rom(include_str!("../programs/leap.c8")))
        .expect("it fits");

    // Long enough to draw the scene and settle into the tick loop.
    for _ in 0..200 {
        chip8.step_many(20).expect("no fault");
        chip8.tick_timers();
    }

    let floor = (0..64).filter(|&column| chip8.pixel(column, 26)).count();
    assert_eq!(
        floor, 56,
        "the floor should be laid with a pit in the middle"
    );
    assert!(!chip8.pixel(28, 26), "the pit should be open");
}

// -- what the machine can do -------------------------------------------------

#[test]
fn assignment_uses_the_shortest_form() {
    assert_eq!(
        body("var x @ V2, y @ V3; fn main() { x = 7; y = x; x += 1; }"),
        ["LD V2, 7", "LD V3, V2", "ADD V2, 1", "RET"]
    );
}

#[test]
fn subtracting_a_constant_goes_through_the_accumulator() {
    // There is no subtract immediate, so the value has to be loaded first.
    assert_eq!(
        body("var x @ V2; fn main() { x -= 1; }"),
        ["LD V0, 1", "SUB V2, V0", "RET"]
    );
}

#[test]
fn a_constant_on_the_left_of_a_subtraction_becomes_subn() {
    assert_eq!(
        body("var x @ V2, y @ V3; fn main() { x = 8 - y; }"),
        ["LD V0, V3", "LD V1, 8", "SUBN V0, V1", "LD V2, V0", "RET"]
    );
}

#[test]
fn shifting_happens_in_place() {
    assert_eq!(
        body("var x @ V2, y @ V3; fn main() { x = y >> 1; }"),
        ["LD V2, V3", "SHR V2", "RET"]
    );
}

#[test]
fn only_shifts_by_one_are_possible() {
    assert_eq!(
        error("var x @ V2; fn main() { x >>= 2; }"),
        "the machine can only shift by one"
    );
}

// -- conditions --------------------------------------------------------------

#[test]
fn a_one_instruction_body_hangs_off_the_skip() {
    assert_eq!(
        body("var x @ V2; fn main() { if (x != 3) x += 1; }"),
        ["SE V2, 3", "ADD V2, 1", "RET"]
    );
}

#[test]
fn an_equality_test_skips_on_the_opposite_one() {
    assert_eq!(
        body("var x @ V2; fn main() { if (x == 3) x += 1; }"),
        ["SNE V2, 3", "ADD V2, 1", "RET"]
    );
}

#[test]
fn a_longer_body_jumps_around_itself() {
    // Too big to hang off the skip, so the skip guards a jump over it instead
    // and the sense of the test flips back.
    assert_eq!(
        body("var x @ V2; fn main() { if (x == 3) { x += 1; x += 2; } }"),
        ["SE V2, 3", "JP _L0", "ADD V2, 1", "ADD V2, 2", "RET"]
    );
}

#[test]
fn a_relational_test_reads_the_flag_a_subtraction_left() {
    assert_eq!(
        body("var x @ V2; fn main() { if (x >= 4) x += 1; }"),
        [
            "LD V0, V2",
            "LD V1, 4",
            "SUB V0, V1",
            "SE VF, 0",
            "ADD V2, 1",
            "RET"
        ]
    );
}

#[test]
fn greater_than_is_less_than_the_other_way_round() {
    assert_eq!(
        body("var x @ V2; fn main() { if (x > 4) x += 1; }"),
        [
            "LD V0, 4",
            "LD V1, V2",
            "SUB V0, V1",
            "SE VF, 1",
            "ADD V2, 1",
            "RET"
        ]
    );
}

#[test]
fn a_second_comparison_reuses_the_first_ones_answer() {
    // `x - 4` is already in the accumulator, so only the right hand side and
    // the subtraction itself are emitted the second time.
    assert_eq!(
        body("var x @ V2, r @ V3; fn main() { if (x < 4) return; if (x - 4 >= 2) return; r = 1; }"),
        [
            "LD V0, V2",
            "LD V1, 4",
            "SUB V0, V1",
            "SE VF, 1",
            "RET",
            "LD V1, 2",
            "SUB V0, V1",
            "SE VF, 0",
            "RET",
            "LD V3, 1",
            "RET",
        ]
    );
}

#[test]
fn a_stale_flag_is_never_reused() {
    // The same comparison twice in a row still subtracts twice: the first
    // answer is still in the accumulator, but its flag has already been read
    // and the second `if` needs a fresh one.
    assert_eq!(
        body("var x @ V2, r @ V3; fn main() { if (x < 4) r = 1; if (x < 4) r = 2; }"),
        [
            "LD V0, V2",
            "LD V1, 4",
            "SUB V0, V1",
            "SE VF, 1",
            "LD V3, 1",
            "LD V0, V2",
            "LD V1, 4",
            "SUB V0, V1",
            "SE VF, 1",
            "LD V3, 2",
            "RET",
        ]
    );
}

#[test]
fn a_key_test_becomes_a_skip() {
    assert_eq!(
        body("var k @ VC; fn main() { k = 5; if (pressed(k)) return; }"),
        ["LD VC, 5", "SKNP VC", "RET", "RET"]
    );
}

#[test]
fn a_condition_has_to_be_a_comparison() {
    assert_eq!(
        error("var x @ V2; fn main() { if (x) return; }"),
        "a condition has to be a comparison, `pressed(key)`, or one of those with `!` in front"
    );
}

// -- loops -------------------------------------------------------------------

#[test]
fn a_do_while_tests_at_the_bottom() {
    assert_eq!(
        body("var x @ V2; fn main() { x = 0; do { x += 1; } while (x != 8); }"),
        ["LD V2, 0", "ADD V2, 1", "SE V2, 8", "JP _L0", "RET"]
    );
}

#[test]
fn a_while_tests_at_the_top() {
    assert_eq!(
        body("var x @ V2; fn main() { x = 0; while (x != 8) { x += 1; } }"),
        [
            "LD V2, 0",
            "SNE V2, 8",
            "JP _L1",
            "ADD V2, 1",
            "JP _L0",
            "RET"
        ]
    );
}

#[test]
fn a_bare_loop_never_ends() {
    // The loop starts where the function does, so it borrows that label
    // rather than making one of its own.
    assert_eq!(body("fn main() { loop {} }"), ["JP main"]);
}

#[test]
fn break_and_continue_find_the_nearest_loop() {
    assert_eq!(
        body(
            "var x @ V2;
             fn main() { x = 0; loop { if (x == 1) break; if (x == 2) continue; x += 1; } }"
        ),
        [
            "LD V2, 0",
            "SNE V2, 1",
            "JP _L1",
            "SNE V2, 2",
            "JP _L0",
            "ADD V2, 1",
            "JP _L0",
            "RET",
        ]
    );
}

#[test]
fn break_needs_a_loop_to_break_out_of() {
    assert_eq!(
        error("fn main() { break; }"),
        "`break` is only allowed inside a loop"
    );
}

// -- memory, sprites and timers ----------------------------------------------

#[test]
fn reading_an_array_sets_the_index_register_every_time() {
    assert_eq!(
        body("var i @ V2; byte t[] = { 1, 2 }; fn main() { i = t[i]; }"),
        ["LD I, t", "ADD I, V2", "LD V0, [I]", "LD V2, V0", "RET"]
    );
}

#[test]
fn a_constant_index_is_folded_into_the_address() {
    assert_eq!(
        body("var x @ V2; byte t[] = { 1, 2 }; fn main() { x = t[1]; }"),
        ["LD I, t + 1", "LD V0, [I]", "LD V2, V0", "RET"]
    );
}

#[test]
fn drawing_the_same_sprite_twice_only_loads_the_address_once() {
    assert_eq!(
        body(
            "var x @ V2, y @ V3; sprite s = { $80, $80 };
             fn main() { draw(x, y, s); draw(x, y, s); }"
        ),
        ["LD I, s", "DRW V2, V3, 2", "DRW V2, V3, 2", "RET"]
    );
}

#[test]
fn a_font_glyph_is_five_rows_tall() {
    assert_eq!(
        body("var x @ V2, y @ V3, d @ V4; fn main() { draw(x, y, font(d)); }"),
        ["LD F, V4", "DRW V2, V3, 5", "RET"]
    );
}

#[test]
fn the_timers_go_through_the_accumulator() {
    assert_eq!(
        body("fn main() { delay = 60; sound = 2; }"),
        ["LD V0, 60", "LD DT, V0", "LD V0, 2", "LD ST, V0", "RET"]
    );
}

#[test]
fn waiting_on_the_delay_timer_reads_it_afresh_each_time() {
    assert_eq!(
        body("fn main() { wait: if (delay != 0) goto wait; }"),
        ["LD V0, DT", "SE V0, 0", "JP wait", "RET"]
    );
}

#[test]
fn bcd_and_the_block_moves_use_the_index_register() {
    assert_eq!(
        body(
            "var n @ V2; byte t[] = { 0, 0, 0 };
             fn main() { I = t; bcd(n); restore(V2); }"
        ),
        ["LD I, t", "LD B, V2", "LD V2, [I]", "RET"]
    );
}

// -- functions ---------------------------------------------------------------

#[test]
fn every_function_returns_unless_it_has_jumped_away() {
    // `main` gets a `RET`; `other` never reaches the end of itself, so it
    // does not.
    assert_eq!(
        body("fn main() { other(); } fn other() { loop {} }"),
        ["CALL other", "RET", "JP other"]
    );
}

#[test]
fn a_jump_under_a_skip_is_not_the_end_of_a_function() {
    // The skip can step straight past the jump, so there has to be something
    // to land on.
    assert_eq!(
        body("var x @ V2; fn main() { top: x += 1; if (x != 8) goto top; }"),
        ["ADD V2, 1", "SE V2, 8", "JP top", "RET"]
    );
}

#[test]
fn a_call_is_assumed_to_disturb_everything() {
    // The accumulator is reloaded after the call even though 1 was just put
    // there, because the subroutine may well have used it.
    assert_eq!(
        body("var x @ V2; fn main() { x -= 1; other(); x -= 1; } fn other() { }"),
        [
            "LD V0, 1",
            "SUB V2, V0",
            "CALL other",
            "LD V0, 1",
            "SUB V2, V0",
            "RET",
            "RET",
        ]
    );
}

#[test]
fn the_first_function_is_where_the_program_starts() {
    let assembly = compile_to_assembly("fn first() { } fn second() { }").expect("it compiles");
    let first = assembly.find("first:").expect("first is there");
    let second = assembly.find("second:").expect("second is there");

    assert!(first < second, "the entry point has to come first");
}

// -- names -------------------------------------------------------------------

#[test]
fn constants_are_worked_out_at_compile_time() {
    assert_eq!(
        body("const W = 8; const X = 28; const R = X + W - 5; var x @ V2; fn main() { x = R; }"),
        ["LD V2, 31", "RET"]
    );
}

#[test]
fn a_constant_cannot_be_assigned_to() {
    assert_eq!(
        error("const N = 1; fn main() { N = 2; }"),
        "`N` is a constant, so it cannot be assigned to"
    );
}

#[test]
fn an_undeclared_name_is_caught() {
    assert_eq!(error("fn main() { x = 1; }"), "`x` has not been declared");
}

#[test]
fn a_value_that_does_not_fit_in_a_byte_is_caught() {
    assert_eq!(
        error("var x @ V2; fn main() { x = 300; }"),
        "300 does not fit in a byte"
    );
}

#[test]
fn a_program_needs_a_function() {
    assert_eq!(
        error("const N = 1;"),
        "a program needs at least one function, and the first one is where it starts"
    );
}

#[test]
fn errors_carry_the_line_they_happened_on() {
    let error = compile("var x @ V2;\n\nfn main() {\n    y = 1;\n}\n").expect_err("it fails");

    assert_eq!(error.line, 4);
    assert_eq!(error.to_string(), "line 4: `y` has not been declared");
}
