//! Checks the C9 compiler against the assembler.
//!
//! `programs/leap.c9` and `programs/leap.asm` are the same program written
//! twice, once in each language, and the same goes for `abc123`. Compiling one
//! and assembling the other has to produce the same bytes, which pins the
//! compiler to a reference written by hand and keeps the two copies of each
//! program from drifting apart.

use chip9::asm::assemble;
use chip9::lang::{compile, compile_to_assembly};

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
        rom(include_str!("../programs/leap.c9")),
        include_bytes!("../roms/leap.ch8"),
        "roms/leap.ch8 is out of date, re-run: \
         cargo run --bin c9c -- programs/leap.c9 roms/leap.ch8"
    );
}

#[test]
fn abc123_compiles_to_the_committed_rom() {
    assert_eq!(
        rom(include_str!("../programs/abc123.c9")),
        include_bytes!("../roms/abc123.ch8"),
        "roms/abc123.ch8 is out of date, re-run: \
         cargo run --bin c9c -- programs/abc123.c9 roms/abc123.ch8"
    );
}

#[test]
fn hello_compiles_to_the_committed_rom() {
    assert_eq!(
        rom(include_str!("../programs/hello.c9")),
        include_bytes!("../roms/hello.ch8"),
        "roms/hello.ch8 is out of date, re-run: \
         cargo run --bin c9c -- programs/hello.c9 roms/hello.ch8"
    );
}

#[test]
fn times_compiles_to_the_committed_rom() {
    assert_eq!(
        rom(include_str!("../programs/times.c9")),
        include_bytes!("../roms/times.ch8"),
        "roms/times.ch8 is out of date, re-run: \
         cargo run --bin c9c -- programs/times.c9 roms/times.ch8"
    );
}

#[test]
fn leap_compiles_to_the_same_bytes_the_assembler_makes() {
    let compiled = rom(include_str!("../programs/leap.c9"));
    let assembled = assemble(include_str!("../programs/leap.asm"))
        .expect("the reference assembles")
        .rom;

    assert_eq!(
        compiled, assembled,
        "programs/leap.c9 and programs/leap.asm have drifted apart"
    );
}

#[test]
fn abc123_compiles_to_the_same_bytes_the_assembler_makes() {
    let compiled = rom(include_str!("../programs/abc123.c9"));
    let assembled = assemble(include_str!("../programs/abc123.asm"))
        .expect("the reference assembles")
        .rom;

    assert_eq!(
        compiled, assembled,
        "programs/abc123.c9 and programs/abc123.asm have drifted apart"
    );
}

#[test]
fn the_compiled_game_still_plays() {
    use chip9::cpu::Chip9;

    let mut chip9 = Chip9::new();
    chip9
        .load(&rom(include_str!("../programs/leap.c9")))
        .expect("it fits");

    // Long enough to draw the scene and settle into the tick loop.
    for _ in 0..200 {
        chip9.step_many(20).expect("no fault");
        chip9.tick_timers();
    }

    let floor = (0..64).filter(|&column| chip9.pixel(column, 26)).count();
    assert_eq!(
        floor, 56,
        "the floor should be laid with a pit in the middle"
    );
    assert!(!chip9.pixel(28, 26), "the pit should be open");
}

/// A band of the screen as text, so a test can show what it draws.
fn screen(chip9: &chip9::cpu::Chip9, rows: std::ops::Range<usize>) -> String {
    rows.map(|y| {
        (0..64)
            .map(|x| if chip9.pixel(x, y) { '#' } else { '.' })
            .collect::<String>()
    })
    .collect::<Vec<_>>()
    .join("\n")
}

fn after(source: &str, cycles: u32) -> chip9::cpu::Chip9 {
    let mut chip9 = chip9::cpu::Chip9::new();
    chip9.load(&rom(source)).expect("it fits");
    chip9.step_many(cycles).expect("no fault");
    chip9
}

#[test]
fn hello_greets_the_world_in_the_alphabet_font() {
    let chip9 = after(include_str!("../programs/hello.c9"), 2_000);

    // The O borrows the zero glyph, which is the one letter the four columns
    // of the font cannot tell apart.
    assert_eq!(
        screen(&chip9, 10..23),
        "\
.................#..#.####.#....#....####.......................
.................#..#.#....#....#....#..#.......................
.................####.####.#....#....#..#.......................
.................#..#.#....#....#....#..#.......................
.................#..#.####.####.####.####.......................
................................................................
................................................................
................................................................
.................#..#.####.####.#....###........................
.................#..#.#..#.#..#.#....#..#.......................
.................#..#.#..#.####.#....#..#.......................
.................####.#..#.#.#..#....#..#.......................
.................####.####.#..#.####.###........................"
    );
}

#[test]
fn times_multiplies_and_divides_its_way_across_the_grid() {
    let chip9 = after(include_str!("../programs/times.c9"), 20_000);

    // 1 X 7 = 7 and 5 X 7 = 35 across the top of the grid.
    assert_eq!(
        screen(&chip9, 2..7),
        "\
...#..#..#.####......####........####.#..#.####......####.####..
..##..#..#....#.........#........#....#..#....#.........#.#.....
...#...##....#.........#.........####..##....#.......####.####..
...#..#..#..#.........#.............#.#..#..#...........#....#..
..###.#..#..#.........#..........####.#..#..#........####.####.."
    );

    // 4 X 7 = 28 and 8 X 7 = 56 across the bottom.
    assert_eq!(
        screen(&chip9, 23..28),
        "\
.#..#.#..#.####......####.####...####.#..#.####......####.####..
.#..#.#..#....#.........#.#..#...#..#.#..#....#......#....#.....
.####..##....#.......####.####...####..##....#.......####.####..
....#.#..#..#........#....#..#...#..#.#..#..#...........#.#..#..
....#.#..#..#........####.####...####.#..#..#........####.####.."
    );

    assert_eq!(
        chip9.data_stack_depth(),
        0,
        "every call should have unwound the data stack it borrowed"
    );
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

// -- short circuits ----------------------------------------------------------

#[test]
fn and_gives_up_as_soon_as_one_side_is_false() {
    assert_eq!(
        body("var a @ V2, b @ V3, x @ V4; fn main() { if (a == 1 && b == 2) x = 1; }"),
        ["SE V2, 1", "JP _L0", "SE V3, 2", "JP _L0", "LD V4, 1", "RET"]
    );
}

#[test]
fn or_stops_as_soon_as_one_side_is_true() {
    // Once `a` is 1 there is no point looking at `b`, so the first branch
    // jumps forward to the body instead of round it.
    assert_eq!(
        body("var a @ V2, b @ V3, x @ V4; fn main() { if (a == 1 || b == 2) x = 1; }"),
        [
            "SNE V2, 1",
            "JP _L1",
            "SE V3, 2",
            "JP _L0",
            "LD V4, 1",
            "RET"
        ]
    );
}

#[test]
fn not_around_a_short_circuit_turns_the_whole_thing_round() {
    // `!(a && b)` is `!a || !b`, which is two branches to the body.
    assert_eq!(
        body("var a @ V2, b @ V3, x @ V4; fn main() { if (!(a == 1 && b == 2)) x = 1; }"),
        [
            "SE V2, 1",
            "JP _L1",
            "SNE V3, 2",
            "JP _L0",
            "LD V4, 1",
            "RET"
        ]
    );
}

#[test]
fn a_short_circuit_works_in_a_loop_too() {
    assert_eq!(
        body("var a @ V2, b @ V3; fn main() { while (a != 0 && b != 0) a -= 1; }"),
        [
            "SNE V2, 0",
            "JP _L0",
            "SNE V3, 0",
            "JP _L0",
            "LD V0, 1",
            "SUB V2, V0",
            "JP main",
            "RET",
        ]
    );
}

#[test]
fn three_conditions_chain_without_a_working_register() {
    assert_eq!(
        body(
            "var a @ V2, b @ V3, c @ V4, x @ V5;
             fn main() { if (a == 1 && b == 2 && c == 3) x = 1; }"
        ),
        ["SE V2, 1", "JP _L0", "SE V3, 2", "JP _L0", "SE V4, 3", "JP _L0", "LD V5, 1", "RET"]
    );
}

#[test]
fn a_short_circuit_is_not_a_value() {
    assert_eq!(
        error("var a @ V2, b @ V3, x @ V4; fn main() { x = a == 1 && b == 2; }"),
        "`&&` and `||` only make sense in a condition"
    );
}

// -- multiplying and dividing ------------------------------------------------

#[test]
fn multiplying_keeps_the_low_byte() {
    assert_eq!(
        body("var a @ V2, b @ V3, x @ V4; fn main() { x = a * b; }"),
        ["LD V0, V2", "LD V1, V3", "MUL V0, V1", "LD V4, V0", "RET"]
    );
}

#[test]
fn a_multiply_in_place_needs_no_accumulator() {
    assert_eq!(
        body("var a @ V2, b @ V3; fn main() { a *= b; }"),
        ["MUL V2, V3", "RET"]
    );
}

#[test]
fn dividing_keeps_the_quotient() {
    assert_eq!(
        body("var a @ V2, b @ V3, x @ V4; fn main() { x = a / b; }"),
        ["LD V0, V2", "LD V1, V3", "DIV V0, V1", "LD V4, V0", "RET"]
    );
}

#[test]
fn a_modulo_reads_the_remainder_out_of_the_flag() {
    assert_eq!(
        body("var a @ V2, b @ V3, x @ V4; fn main() { x = a % b; }"),
        [
            "LD V0, V2",
            "LD V1, V3",
            "DIV V0, V1",
            "LD V0, VF",
            "LD V4, V0",
            "RET"
        ]
    );
}

#[test]
fn the_arithmetic_actually_comes_out_right() {
    use chip9::cpu::Chip9;

    let mut chip9 = Chip9::new();
    chip9
        .load(&rom(
            "var a @ V2, b @ V3, c @ V4, d @ V5;
             fn main() { a = 7; b = 3; c = a * b; d = a % b; a = a / b; loop {} }",
        ))
        .expect("it fits");

    chip9.step_many(100).expect("no fault");

    assert_eq!(chip9.register(4), 21, "7 * 3");
    assert_eq!(chip9.register(5), 1, "7 % 3");
    assert_eq!(chip9.register(2), 2, "7 / 3");
}

// -- the data stack ----------------------------------------------------------

#[test]
fn a_value_can_be_pushed_and_taken_back() {
    assert_eq!(
        body("var a @ V2, b @ V3; fn main() { push(a); push(9); b = pop(); a = pop(); }"),
        [
            "PUSH V2",
            "LD V0, 9",
            "PUSH V0",
            "POP V0",
            "LD V3, V0",
            "POP V0",
            "LD V2, V0",
            "RET"
        ]
    );
}

#[test]
fn the_stack_hands_values_back_in_the_other_order() {
    use chip9::cpu::Chip9;

    let mut chip9 = Chip9::new();
    chip9
        .load(&rom(
            "var a @ V2, b @ V3;
             fn main() { push(1); push(2); a = pop(); b = pop(); loop {} }",
        ))
        .expect("it fits");

    chip9.step_many(100).expect("no fault");

    assert_eq!(chip9.register(2), 2, "the last one pushed comes back first");
    assert_eq!(chip9.register(3), 1);
    assert_eq!(chip9.data_stack_depth(), 0);
}

#[test]
fn push_produces_nothing_and_pop_takes_nothing() {
    assert_eq!(
        error("var a; fn main() { a = push(1); }"),
        "`push` does not produce a value"
    );
    assert_eq!(
        error("var a; fn main() { a = pop(1); }"),
        "`pop` takes no arguments"
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

#[test]
fn a_remembered_value_is_dropped_once_its_variable_changes() {
    // The accumulator still holds `x + 1` from the first line, but `x` is not
    // the same `x` any more, so the sum has to be worked out again.
    assert_eq!(
        body("var x @ V2, y @ V3, z @ V4; fn main() { y = x + 1; x = 9; z = x + 1; }"),
        [
            "LD V0, V2",
            "ADD V0, 1",
            "LD V3, V0",
            "LD V2, 9",
            "LD V0, V2",
            "ADD V0, 1",
            "LD V4, V0",
            "RET"
        ]
    );
}

#[test]
fn a_remembered_value_survives_a_write_to_an_unrelated_variable() {
    assert_eq!(
        body("var x @ V2, y @ V3, z @ V4; fn main() { y = x + 1; y = 9; z = x + 1; }"),
        [
            "LD V0, V2",
            "ADD V0, 1",
            "LD V3, V0",
            "LD V3, 9",
            "LD V4, V0",
            "RET"
        ]
    );
}

// -- calls, arguments and results ---------------------------------------------

#[test]
fn a_call_hands_its_argument_straight_over() {
    // The last argument would come off the stack again immediately, so it
    // goes into place directly.
    assert_eq!(
        body("var n @ V2; fn main() { twice(n); } fn twice(k) { k += k; }"),
        ["LD V3, V2", "CALL twice", "RET", "ADD V3, V3", "RET"]
    );
}

#[test]
fn several_arguments_queue_up_on_the_data_stack() {
    assert_eq!(
        body("var a @ V2, b @ V3; fn main() { sum(a, b); } fn sum(x, y) { x += y; }"),
        [
            "LD V0, V2",
            "PUSH V0",
            "LD V5, V3",
            "POP V4",
            "CALL sum",
            "RET",
            "ADD V4, V5",
            "RET"
        ]
    );
}

#[test]
fn the_caller_puts_its_own_registers_somewhere_safe() {
    // `main` has nothing of its own, so it saves nothing. `outer` holds a
    // local across the call, and `inner` was given the same register, so the
    // local has to wait on the stack.
    assert_eq!(
        body(
            "fn main() { outer(); }
             fn outer() { var t; t = 1; inner(t + 1); t += 1; }
             fn inner(k) { k += 1; }"
        ),
        [
            "CALL outer",
            "RET", // main
            "LD V2, 1",
            "PUSH V2",
            "LD V0, V2",
            "ADD V0, 1",
            "LD V2, V0",
            "CALL inner",
            "POP V2",
            "ADD V2, 1",
            "RET", // outer
            "ADD V2, 1",
            "RET", // inner
        ]
    );
}

#[test]
fn a_returned_value_comes_back_in_the_accumulator() {
    assert_eq!(
        body("var x @ V2; fn main() { x = one(); } fn one() { return 1; }"),
        ["CALL one", "LD V2, V0", "RET", "LD V0, 1", "RET"]
    );
}

#[test]
fn a_function_that_already_returns_does_not_get_a_second_one() {
    assert_eq!(
        body("fn main() { other(); } fn other() { return; }"),
        ["CALL other", "RET", "RET"]
    );
}

#[test]
fn recursion_works_all_the_way_down() {
    use chip9::cpu::Chip9;

    let mut chip9 = Chip9::new();
    chip9
        .load(&rom("var answer @ V2;
             fn main() { answer = fact(5); loop {} }
             fn fact(k) {
                 var rest;
                 if (k == 0) { return 1; }
                 rest = fact(k - 1);
                 return k * rest;
             }"))
        .expect("it fits");

    chip9.step_many(400).expect("no fault");

    assert_eq!(chip9.register(2), 120, "5! is 120");
    assert_eq!(
        chip9.data_stack_depth(),
        0,
        "every push should have been matched by a pop"
    );
}

#[test]
fn a_call_may_stand_in_for_an_argument() {
    use chip9::cpu::Chip9;

    let mut chip9 = Chip9::new();
    chip9
        .load(&rom("var answer @ V2;
             fn main() { answer = twice(twice(3)); loop {} }
             fn twice(k) { return k + k; }"))
        .expect("it fits");

    chip9.step_many(200).expect("no fault");
    assert_eq!(chip9.register(2), 12);
    assert_eq!(chip9.data_stack_depth(), 0);
}

#[test]
fn a_function_has_to_be_called_with_the_right_number_of_arguments() {
    assert_eq!(
        error("fn main() { add(1); } fn add(a, b) { a += b; }"),
        "`add` takes 2 arguments, but 1 were given"
    );
}

#[test]
fn the_first_function_cannot_take_arguments() {
    assert_eq!(
        error("fn main(k) { k += 1; }"),
        "`main` is where the program starts, so there is nobody to pass it arguments"
    );
}

// -- scopes ------------------------------------------------------------------

#[test]
fn a_local_lasts_as_long_as_its_block_and_no_longer() {
    // `a` and `b` never exist at the same time, so one register does for both.
    assert_eq!(
        body("fn main() { { var a; a = 1; } { var b; b = 2; } }"),
        ["LD V2, 1", "LD V2, 2", "RET"]
    );
}

#[test]
fn two_locals_side_by_side_get_registers_of_their_own() {
    assert_eq!(
        body("fn main() { var a, b; a = 1; b = 2; }"),
        ["LD V2, 1", "LD V3, 2", "RET"]
    );
}

#[test]
fn a_local_hides_a_global_of_the_same_name() {
    assert_eq!(
        body("var x @ V5; fn main() { x = 1; { var x; x = 2; } x = 3; }"),
        ["LD V5, 1", "LD V2, 2", "LD V5, 3", "RET"]
    );
}

#[test]
fn a_pinned_register_is_left_alone_by_the_allocator() {
    assert_eq!(
        body("var a @ V2; fn main() { var b; a = 1; b = 2; }"),
        ["LD V2, 1", "LD V3, 2", "RET"]
    );
}

#[test]
fn a_name_out_of_scope_is_not_a_name_at_all() {
    assert_eq!(
        error("fn main() { { var a; a = 1; } a = 2; }"),
        "`a` has not been declared"
    );
}

#[test]
fn a_local_cannot_be_declared_twice_in_one_block() {
    assert_eq!(
        error("fn main() { var a; var a; }"),
        "`a` is declared twice"
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
