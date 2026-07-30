//! The instructions CHIP-9 adds on top of CHIP-8.
//!
//! Everything the original had is covered by `cpu.rs` next door; this file is
//! only about multiply, divide, the data stack and the alphabet.

use chip9::cpu::{
    Chip9, Fault, DATA_STACK_SIZE, FONT, FONT_CHAR_COUNT, FONT_CHAR_SIZE, PROGRAM_START,
};

fn ops(opcodes: &[u16]) -> Vec<u8> {
    opcodes
        .iter()
        .flat_map(|opcode| opcode.to_be_bytes())
        .collect()
}

fn machine(opcodes: &[u16]) -> Chip9 {
    let mut chip9 = Chip9::with_seed(0x1234_5678_9ABC_DEF0);
    chip9
        .load(&ops(opcodes))
        .expect("the program fits in memory");
    chip9
}

fn run(chip9: &mut Chip9, cycles: u32) {
    chip9.step_many(cycles).expect("no fault");
}

// -- multiply ----------------------------------------------------------------

#[test]
fn mul_puts_the_low_byte_in_vx_and_the_high_byte_in_vf() {
    // LD V0, 12 ; LD V1, 12 ; MUL V0, V1
    let mut chip9 = machine(&[0x600C, 0x610C, 0x8018]);
    run(&mut chip9, 3);

    assert_eq!(chip9.register(0), 144);
    assert_eq!(chip9.register(0xF), 0, "144 fits in a byte");
}

#[test]
fn mul_keeps_the_whole_product() {
    // LD V0, 200 ; LD V1, 3 ; MUL V0, V1  -> 600 = 0x0258
    let mut chip9 = machine(&[0x60C8, 0x6103, 0x8018]);
    run(&mut chip9, 3);

    assert_eq!(chip9.register(0), 0x58);
    assert_eq!(chip9.register(0xF), 0x02);
    assert_eq!(
        u16::from(chip9.register(0xF)) << 8 | u16::from(chip9.register(0)),
        600
    );
}

#[test]
fn mul_reaches_the_largest_product_there_is() {
    // LD V0, 255 ; LD V1, 255 ; MUL V0, V1 -> 65025 = 0xFE01
    let mut chip9 = machine(&[0x60FF, 0x61FF, 0x8018]);
    run(&mut chip9, 3);

    assert_eq!(chip9.register(0), 0x01);
    assert_eq!(chip9.register(0xF), 0xFE);
}

#[test]
fn mul_by_zero_clears_both_halves() {
    // LD V0, 99 ; LD V1, 0 ; MUL V0, V1
    let mut chip9 = machine(&[0x6063, 0x6100, 0x8018]);
    run(&mut chip9, 3);

    assert_eq!(chip9.register(0), 0);
    assert_eq!(chip9.register(0xF), 0);
}

#[test]
fn mul_can_square_a_register_against_itself() {
    // LD V3, 16 ; MUL V3, V3 -> 256 = 0x0100
    let mut chip9 = machine(&[0x6310, 0x8338]);
    run(&mut chip9, 2);

    assert_eq!(chip9.register(3), 0x00);
    assert_eq!(chip9.register(0xF), 0x01);
}

#[test]
fn a_product_written_into_vf_loses_to_the_flag() {
    // The flag is written last, exactly as it is for ADD and SUB.
    // LD VF, 3 ; LD V1, 2 ; MUL VF, V1
    let mut chip9 = machine(&[0x6F03, 0x6102, 0x8F18]);
    run(&mut chip9, 3);

    assert_eq!(chip9.register(0xF), 0, "the high byte of 6");
}

// -- divide ------------------------------------------------------------------

#[test]
fn div_puts_the_quotient_in_vx_and_the_remainder_in_vf() {
    // LD V0, 17 ; LD V1, 5 ; DIV V0, V1
    let mut chip9 = machine(&[0x6011, 0x6105, 0x8019]);
    run(&mut chip9, 3);

    assert_eq!(chip9.register(0), 3);
    assert_eq!(chip9.register(0xF), 2);
}

#[test]
fn an_exact_division_leaves_no_remainder() {
    // LD V0, 100 ; LD V1, 4 ; DIV V0, V1
    let mut chip9 = machine(&[0x6064, 0x6104, 0x8019]);
    run(&mut chip9, 3);

    assert_eq!(chip9.register(0), 25);
    assert_eq!(chip9.register(0xF), 0);
}

#[test]
fn dividing_by_something_larger_gives_zero_and_keeps_the_value() {
    // LD V0, 7 ; LD V1, 10 ; DIV V0, V1
    let mut chip9 = machine(&[0x6007, 0x610A, 0x8019]);
    run(&mut chip9, 3);

    assert_eq!(chip9.register(0), 0);
    assert_eq!(chip9.register(0xF), 7);
}

#[test]
fn dividing_by_zero_is_a_fault_rather_than_a_wrong_answer() {
    // LD V0, 10 ; LD V1, 0 ; DIV V0, V1
    let mut chip9 = machine(&[0x600A, 0x6100, 0x8019]);
    run(&mut chip9, 2);

    let fault = chip9
        .step()
        .expect_err("dividing by zero stops the program");

    assert_eq!(
        fault,
        Fault::DivideByZero {
            pc: PROGRAM_START + 4
        }
    );
    assert_eq!(chip9.register(0), 10, "the dividend is left alone");
}

#[test]
fn mul_and_div_undo_each_other() {
    // LD V0, 7 ; LD V1, 9 ; MUL V0, V1 ; DIV V0, V1
    let mut chip9 = machine(&[0x6007, 0x6109, 0x8018, 0x8019]);
    run(&mut chip9, 4);

    assert_eq!(chip9.register(0), 7);
    assert_eq!(chip9.register(0xF), 0);
}

// -- the data stack ----------------------------------------------------------

#[test]
fn push_and_pop_move_a_byte_through_the_stack() {
    // LD V2, 42 ; PUSH V2 ; LD V2, 0 ; POP V3
    let mut chip9 = machine(&[0x622A, 0xF201, 0x6200, 0xF302]);

    run(&mut chip9, 2);
    assert_eq!(chip9.data_stack_depth(), 1);
    assert_eq!(chip9.data_stack(), &[42]);

    run(&mut chip9, 2);
    assert_eq!(chip9.register(3), 42);
    assert_eq!(chip9.data_stack_depth(), 0);
}

#[test]
fn the_stack_gives_values_back_in_reverse() {
    // LD V0, 1 ; PUSH V0 ; LD V0, 2 ; PUSH V0 ; LD V0, 3 ; PUSH V0
    // POP V1 ; POP V2 ; POP V3
    let mut chip9 = machine(&[
        0x6001, 0xF001, 0x6002, 0xF001, 0x6003, 0xF001, 0xF102, 0xF202, 0xF302,
    ]);

    run(&mut chip9, 6);
    assert_eq!(chip9.data_stack(), &[1, 2, 3]);

    run(&mut chip9, 3);
    assert_eq!(chip9.register(1), 3);
    assert_eq!(chip9.register(2), 2);
    assert_eq!(chip9.register(3), 1);
}

#[test]
fn the_depth_can_be_read_back() {
    // LD V0, 9 ; PUSH V0 ; PUSH V0 ; LD V1, SP ; POP V0 ; LD V2, SP
    let mut chip9 = machine(&[0x6009, 0xF001, 0xF001, 0xF103, 0xF002, 0xF203]);
    run(&mut chip9, 6);

    assert_eq!(chip9.register(1), 2);
    assert_eq!(chip9.register(2), 1);
}

#[test]
fn popping_an_empty_stack_is_a_fault() {
    let mut chip9 = machine(&[0xF002]);

    let fault = chip9.step().expect_err("there is nothing to pop");

    assert_eq!(fault, Fault::DataStackUnderflow { pc: PROGRAM_START });
}

#[test]
fn filling_the_stack_and_pushing_once_more_is_a_fault() {
    // LD V0, 1 ; loop: PUSH V0 ; JP loop
    let mut chip9 = machine(&[0x6001, 0xF001, 0x1202]);

    // One load, then a push and a jump for each byte the stack holds.
    run(&mut chip9, 1 + 2 * DATA_STACK_SIZE as u32);
    assert_eq!(chip9.data_stack_depth(), DATA_STACK_SIZE);

    let fault = chip9.step().expect_err("the stack is full");

    assert!(matches!(fault, Fault::DataStackOverflow { .. }));
}

#[test]
fn the_data_stack_is_not_the_call_stack() {
    // A pushed byte survives a call and a return, and RET does not pop it.
    // PUSH V0 ; CALL sub ; POP V1 ; JP end ; sub: RET ; end: JP end
    let mut chip9 = machine(&[0x6007, 0xF001, 0x220A, 0xF102, 0x120C, 0x00EE, 0x120C]);
    run(&mut chip9, 6);

    assert_eq!(chip9.register(1), 7);
    assert_eq!(chip9.data_stack_depth(), 0);
}

#[test]
fn a_reset_empties_the_data_stack() {
    // LD V0, 5 ; PUSH V0
    let mut chip9 = machine(&[0x6005, 0xF001]);
    run(&mut chip9, 2);
    assert_eq!(chip9.data_stack_depth(), 1);

    chip9.reset();

    assert_eq!(chip9.data_stack_depth(), 0);
    assert_eq!(chip9.data_stack(), &[] as &[u8]);
}

// -- the alphabet ------------------------------------------------------------

#[test]
fn the_font_holds_ten_digits_and_twenty_six_letters() {
    assert_eq!(FONT_CHAR_COUNT, 36);
    assert_eq!(FONT.len(), 36 * FONT_CHAR_SIZE as usize);
}

#[test]
fn the_first_sixteen_characters_are_still_the_chip8_font() {
    #[rustfmt::skip]
    let original: [u8; 80] = [
        0xF0, 0x90, 0x90, 0x90, 0xF0, 0x20, 0x60, 0x20, 0x20, 0x70,
        0xF0, 0x10, 0xF0, 0x80, 0xF0, 0xF0, 0x10, 0xF0, 0x10, 0xF0,
        0x90, 0x90, 0xF0, 0x10, 0x10, 0xF0, 0x80, 0xF0, 0x10, 0xF0,
        0xF0, 0x80, 0xF0, 0x90, 0xF0, 0xF0, 0x10, 0x20, 0x40, 0x40,
        0xF0, 0x90, 0xF0, 0x90, 0xF0, 0xF0, 0x90, 0xF0, 0x10, 0xF0,
        0xF0, 0x90, 0xF0, 0x90, 0x90, 0xE0, 0x90, 0xE0, 0x90, 0xE0,
        0xF0, 0x80, 0x80, 0x80, 0xF0, 0xE0, 0x90, 0x90, 0x90, 0xE0,
        0xF0, 0x80, 0xF0, 0x80, 0xF0, 0xF0, 0x80, 0xF0, 0x80, 0x80,
    ];

    assert_eq!(&FONT[..80], &original);
}

#[test]
fn ld_f_finds_every_character() {
    for character in 0..FONT_CHAR_COUNT {
        // LD V0, character ; LD F, V0
        let mut chip9 = machine(&[0x6000 | u16::from(character), 0xF029]);
        run(&mut chip9, 2);

        assert_eq!(
            chip9.index(),
            u16::from(character) * FONT_CHAR_SIZE,
            "character {character}"
        );
    }
}

#[test]
fn ld_f_still_agrees_with_chip8_for_the_hex_digits() {
    for digit in 0..16u16 {
        let mut chip9 = machine(&[0x6000 | digit, 0xF029]);
        run(&mut chip9, 2);

        assert_eq!(chip9.index(), digit * FONT_CHAR_SIZE);
    }
}

#[test]
fn ld_f_wraps_round_the_alphabet_rather_than_pointing_at_nothing() {
    // 36 is one past 'Z', so it comes back round to '0'.
    let mut chip9 = machine(&[0x6024, 0xF029]);
    run(&mut chip9, 2);

    assert_eq!(chip9.index(), 0);
}

#[test]
fn every_glyph_fits_in_the_left_four_columns() {
    for (index, glyph) in FONT.chunks(FONT_CHAR_SIZE as usize).enumerate() {
        for (row, byte) in glyph.iter().enumerate() {
            assert_eq!(
                byte & 0x0F,
                0,
                "character {index} row {row} spills past four pixels"
            );
        }
    }
}

#[test]
fn no_letter_is_blank() {
    // 'A' is character 10; anything after it is one of the new ones.
    for (index, glyph) in FONT.chunks(FONT_CHAR_SIZE as usize).enumerate().skip(10) {
        assert!(
            glyph.iter().any(|&row| row != 0),
            "character {index} has nothing in it"
        );
    }
}

#[test]
fn drawing_a_letter_lights_the_pixels_the_font_asks_for() {
    // 'H' is character 17: LD V0, 17 ; LD F, V0 ; LD V1, 0 ; DRW V1, V1, 5
    let mut chip9 = machine(&[0x6011, 0xF029, 0x6100, 0xD115]);
    run(&mut chip9, 4);

    let expected = &FONT[17 * FONT_CHAR_SIZE as usize..][..FONT_CHAR_SIZE as usize];

    for (row, byte) in expected.iter().enumerate() {
        for column in 0..4 {
            assert_eq!(
                chip9.pixel(column, row),
                byte & (0x80 >> column) != 0,
                "row {row} column {column} of H"
            );
        }
    }
}
