//! Instruction-level tests, with a bias towards the behaviour the JavaScript
//! version got wrong.

use chip9::cpu::{
    Chip9, Fault, Quirks, FONT, FONT_CHAR_SIZE, MAX_PROGRAM_SIZE, PROGRAM_START, SCREEN_HEIGHT,
    SCREEN_WIDTH,
};

/// Assembles a list of opcodes into a program.
fn ops(opcodes: &[u16]) -> Vec<u8> {
    opcodes
        .iter()
        .flat_map(|opcode| opcode.to_be_bytes())
        .collect()
}

/// A machine with `opcodes` loaded and a fixed random seed.
fn machine(opcodes: &[u16]) -> Chip9 {
    let mut chip9 = Chip9::with_seed(0x1234_5678_9ABC_DEF0);
    chip9
        .load(&ops(opcodes))
        .expect("the program fits in memory");
    chip9
}

fn step(chip9: &mut Chip9) {
    chip9.step().expect("no fault");
}

fn run(chip9: &mut Chip9, cycles: u32) {
    chip9.step_many(cycles).expect("no fault");
}

/// Counts the lit pixels on the screen.
fn lit_pixels(chip9: &Chip9) -> usize {
    chip9.framebuffer().iter().filter(|&&lit| lit).count()
}

#[test]
fn starts_at_the_program_entry_point_with_the_font_in_place() {
    let chip9 = machine(&[0x1200]);

    assert_eq!(chip9.pc(), PROGRAM_START);
    assert_eq!(&chip9.memory()[..FONT.len()], &FONT);
    assert_eq!(chip9.memory()[PROGRAM_START as usize], 0x12);
}

#[test]
fn rejects_a_program_that_does_not_fit() {
    let mut chip9 = Chip9::with_seed(1);

    assert!(chip9.load(&vec![0; MAX_PROGRAM_SIZE]).is_ok());
    assert!(chip9.load(&vec![0; MAX_PROGRAM_SIZE + 1]).is_err());
}

#[test]
fn reset_restores_the_loaded_program() {
    // LD V0, 0x42 ; CLS
    let mut chip9 = machine(&[0x6042, 0x00E0]);
    run(&mut chip9, 2);

    assert_eq!(chip9.register(0), 0x42);
    assert_eq!(chip9.pc(), PROGRAM_START + 4);

    chip9.reset();

    assert_eq!(chip9.register(0), 0);
    assert_eq!(chip9.pc(), PROGRAM_START);
    assert_eq!(chip9.memory()[PROGRAM_START as usize], 0x60);
}

// --- the bugs the JavaScript version carried ------------------------------

#[test]
fn drawing_a_blank_sprite_does_not_report_a_collision() {
    // The JavaScript version computed `!(bit ^ pixel) > 0`, which is `true` for
    // every transparent pixel, so drawing anything set VF.
    let mut chip9 = machine(&[0xA300, 0xD001]);
    chip9.write_memory(0x300, 0x00);

    run(&mut chip9, 2);

    assert_eq!(lit_pixels(&chip9), 0);
    assert_eq!(chip9.register(0xF), 0, "a blank sprite must not collide");
}

#[test]
fn collision_is_only_reported_when_a_lit_pixel_is_erased() {
    // LD I, 0x300 ; DRW V0, V0, 1 ; DRW V0, V0, 1
    let mut chip9 = machine(&[0xA300, 0xD001, 0xD001]);
    chip9.write_memory(0x300, 0b1000_0000);

    run(&mut chip9, 2);
    assert!(chip9.pixel(0, 0));
    assert_eq!(chip9.register(0xF), 0, "the screen was empty");

    step(&mut chip9);
    assert!(
        !chip9.pixel(0, 0),
        "the sprite is XORed, so it erases itself"
    );
    assert_eq!(chip9.register(0xF), 1, "erasing a pixel is a collision");
}

#[test]
fn collision_is_latched_for_the_whole_sprite() {
    // Draw two rows where only the first one overlaps: VF must stay 1 even
    // though the last pixel drawn did not collide.
    let mut chip9 = machine(&[0xA300, 0xD001, 0xA301, 0xD002]);
    chip9.write_memory(0x300, 0b1000_0000);
    chip9.write_memory(0x301, 0b1000_0000);
    chip9.write_memory(0x302, 0b0100_0000);

    run(&mut chip9, 4);

    assert!(!chip9.pixel(0, 0), "the overlapping pixel was erased");
    assert!(chip9.pixel(1, 1), "the second row was drawn");
    assert_eq!(chip9.register(0xF), 1);
}

#[test]
fn sprites_are_clipped_at_the_screen_edge_by_default() {
    // LD V0, 63 ; LD V1, 31 ; LD I, 0x300 ; DRW V0, V1, 2
    let mut chip9 = machine(&[0x603F, 0x611F, 0xA300, 0xD012]);
    chip9.write_memory(0x300, 0xFF);
    chip9.write_memory(0x301, 0xFF);

    run(&mut chip9, 4);

    assert!(chip9.pixel(SCREEN_WIDTH - 1, SCREEN_HEIGHT - 1));
    assert_eq!(
        lit_pixels(&chip9),
        1,
        "the rest of the sprite is off screen"
    );
    assert!(
        !chip9.pixel(0, SCREEN_HEIGHT - 1),
        "it must not wrap horizontally"
    );
    assert!(
        !chip9.pixel(SCREEN_WIDTH - 1, 0),
        "it must not wrap vertically"
    );
}

#[test]
fn sprites_wrap_when_the_quirk_is_turned_off() {
    let mut chip9 = machine(&[0x603F, 0x611F, 0xA300, 0xD012]);
    chip9.set_quirks(Quirks {
        clip_sprites: false,
        ..Quirks::default()
    });
    chip9.write_memory(0x300, 0xFF);
    chip9.write_memory(0x301, 0xFF);

    run(&mut chip9, 4);

    assert_eq!(lit_pixels(&chip9), 16);
    assert!(chip9.pixel(SCREEN_WIDTH - 1, SCREEN_HEIGHT - 1));
    assert!(chip9.pixel(0, SCREEN_HEIGHT - 1), "wrapped horizontally");
    assert!(chip9.pixel(SCREEN_WIDTH - 1, 0), "wrapped vertically");
}

#[test]
fn the_starting_position_of_a_sprite_always_wraps() {
    // (65, 33) is the same as (1, 1) on a 64x32 screen.
    let mut chip9 = machine(&[0x6041, 0x6121, 0xA300, 0xD011]);
    chip9.write_memory(0x300, 0b1000_0000);

    run(&mut chip9, 4);

    assert!(chip9.pixel(1, 1));
    assert_eq!(lit_pixels(&chip9), 1);
}

#[test]
fn add_wraps_around_and_reports_the_carry() {
    // LD V0, 0xFF ; LD V1, 0x02 ; ADD V0, V1
    let mut chip9 = machine(&[0x60FF, 0x6102, 0x8014]);
    run(&mut chip9, 3);

    assert_eq!(chip9.register(0), 0x01);
    assert_eq!(chip9.register(0xF), 1);
}

#[test]
fn subtract_wraps_around_instead_of_going_negative() {
    // The JavaScript version stored -1 in the register.
    // LD V0, 0x00 ; LD V1, 0x01 ; SUB V0, V1
    let mut chip9 = machine(&[0x6000, 0x6101, 0x8015]);
    run(&mut chip9, 3);

    assert_eq!(chip9.register(0), 0xFF);
    assert_eq!(chip9.register(0xF), 0, "VF is NOT borrow");
}

#[test]
fn reverse_subtract_wraps_around_too() {
    // LD V0, 0x05 ; LD V1, 0x03 ; SUBN V0, V1  (V0 = V1 - V0)
    let mut chip9 = machine(&[0x6005, 0x6103, 0x8017]);
    run(&mut chip9, 3);

    assert_eq!(chip9.register(0), 0xFE);
    assert_eq!(chip9.register(0xF), 0);
}

#[test]
fn shift_left_keeps_the_result_inside_a_byte() {
    // The JavaScript version stored 0x1FE here.
    // LD V0, 0xFF ; SHL V0
    let mut chip9 = machine(&[0x60FF, 0x800E]);
    run(&mut chip9, 2);

    assert_eq!(chip9.register(0), 0xFE);
    assert_eq!(chip9.register(0xF), 1, "the bit that fell off the top");
}

#[test]
fn shift_right_reports_the_bit_that_fell_off() {
    // LD V0, 0x03 ; SHR V0
    let mut chip9 = machine(&[0x6003, 0x8006]);
    run(&mut chip9, 2);

    assert_eq!(chip9.register(0), 0x01);
    assert_eq!(chip9.register(0xF), 1);
}

#[test]
fn add_byte_wraps_without_touching_the_flag() {
    // LD VF, 0x07 ; LD V0, 0xFF ; ADD V0, 0x02
    let mut chip9 = machine(&[0x6F07, 0x60FF, 0x7002]);
    run(&mut chip9, 3);

    assert_eq!(chip9.register(0), 0x01);
    assert_eq!(chip9.register(0xF), 0x07, "7xkk never sets the carry flag");
}

#[test]
fn arithmetic_into_vf_keeps_the_flag_not_the_result() {
    // LD VF, 0x01 ; LD V1, 0x01 ; ADD VF, V1 -> VF holds the carry, which is 0.
    let mut chip9 = machine(&[0x6F01, 0x6101, 0x8F14]);
    run(&mut chip9, 3);

    assert_eq!(chip9.register(0xF), 0);
}

#[test]
fn random_can_produce_every_byte_including_255() {
    // `Math.floor(Math.random() * 0xFF)` in the JavaScript version topped out
    // at 254.
    let mut chip9 = machine(&[0xC0FF, 0x1200]);
    let mut seen = [false; 256];

    for _ in 0..20_000 {
        run(&mut chip9, 2);
        seen[usize::from(chip9.register(0))] = true;
    }

    assert!(seen[255], "0xFF must be reachable");
    assert!(seen.iter().all(|&hit| hit), "every byte must be reachable");
}

#[test]
fn random_is_masked_by_the_operand() {
    let mut chip9 = machine(&[0xC00F, 0x1200]);

    for _ in 0..1_000 {
        run(&mut chip9, 2);
        assert_eq!(chip9.register(0) & 0xF0, 0);
    }
}

#[test]
fn an_opcode_of_zero_is_a_fault_rather_than_a_silent_stop() {
    // Memory past the program is zeroed, which is exactly what a runaway
    // program hits. The JavaScript version ignored it and spun forever.
    let mut chip9 = machine(&[0x1202]);
    step(&mut chip9);

    let fault = chip9.step().expect_err("0x0000 is not an instruction");

    assert_eq!(
        fault,
        Fault::UnknownOpcode {
            pc: 0x202,
            opcode: 0x0000
        }
    );
    assert!(!fault.hint().is_empty());
}

#[test]
fn timers_stop_at_zero() {
    // LD V0, 2 ; LD DT, V0 ; LD ST, V0
    let mut chip9 = machine(&[0x6002, 0xF015, 0xF018]);
    run(&mut chip9, 3);

    assert_eq!(chip9.delay_timer(), 2);
    assert!(chip9.is_beeping());

    for _ in 0..5 {
        chip9.tick_timers();
    }

    assert_eq!(chip9.delay_timer(), 0);
    assert_eq!(chip9.sound_timer(), 0);
    assert!(!chip9.is_beeping());
}

// --- control flow ---------------------------------------------------------

#[test]
fn call_and_return_walk_the_stack() {
    // CALL 0x204 ; JP 0x200 ; RET
    let mut chip9 = machine(&[0x2204, 0x1200, 0x00EE]);

    step(&mut chip9);
    assert_eq!(chip9.pc(), 0x204);

    step(&mut chip9);
    assert_eq!(
        chip9.pc(),
        0x202,
        "RET goes back to the instruction after CALL"
    );
}

#[test]
fn too_many_calls_are_a_fault() {
    // A subroutine that calls itself.
    let mut chip9 = machine(&[0x2200]);

    let fault = chip9.step_many(20).expect_err("the stack is 16 deep");

    assert!(matches!(fault, Fault::StackOverflow { .. }));
}

#[test]
fn returning_without_calling_is_a_fault() {
    let mut chip9 = machine(&[0x00EE]);

    assert_eq!(chip9.step(), Err(Fault::StackUnderflow { pc: 0x200 }));
}

#[test]
fn skips_jump_over_one_instruction() {
    // LD V0, 5 ; SE V0, 5 ; LD V1, 1 ; LD V1, 2
    let mut chip9 = machine(&[0x6005, 0x3005, 0x6101, 0x6102]);
    run(&mut chip9, 3);

    assert_eq!(chip9.register(1), 2, "the LD V1, 1 was skipped");
}

#[test]
fn jump_with_offset_uses_v0() {
    // LD V0, 4 ; JP V0, 0x300
    let mut chip9 = machine(&[0x6004, 0xB300]);
    run(&mut chip9, 2);

    assert_eq!(chip9.pc(), 0x304);
}

// --- memory ---------------------------------------------------------------

#[test]
fn bcd_splits_a_byte_into_three_digits() {
    // LD V0, 254 ; LD I, 0x300 ; LD B, V0
    let mut chip9 = machine(&[0x60FE, 0xA300, 0xF033]);
    run(&mut chip9, 3);

    assert_eq!(&chip9.memory()[0x300..0x303], &[2, 5, 4]);
}

#[test]
fn registers_round_trip_through_memory() {
    // LD V0, 0xAA ; LD V1, 0xBB ; LD V2, 0xCC ; LD I, 0x300 ; LD [I], V2
    // LD V0, 0 ; LD V1, 0 ; LD V2, 0 ; LD I, 0x300 ; LD V2, [I]
    let mut chip9 = machine(&[
        0x60AA, 0x61BB, 0x62CC, 0xA300, 0xF255, 0x6000, 0x6100, 0x6200, 0xA300, 0xF265,
    ]);
    run(&mut chip9, 10);

    assert_eq!(&chip9.memory()[0x300..0x303], &[0xAA, 0xBB, 0xCC]);
    assert_eq!(chip9.register(0), 0xAA);
    assert_eq!(chip9.register(1), 0xBB);
    assert_eq!(chip9.register(2), 0xCC);
    assert_eq!(chip9.index(), 0x300, "I is left alone by default");
}

#[test]
fn load_and_store_can_move_the_index_register() {
    let mut chip9 = machine(&[0xA300, 0xF255]);
    chip9.set_quirks(Quirks {
        increment_i_on_load_store: true,
        ..Quirks::default()
    });
    run(&mut chip9, 2);

    assert_eq!(chip9.index(), 0x303);
}

#[test]
fn writing_past_the_end_of_memory_is_a_fault() {
    // LD I, 0xFFF ; LD [I], VF touches 0xFFF..0x100E.
    let mut chip9 = machine(&[0xAFFF, 0xFF55]);
    step(&mut chip9);

    let fault = chip9.step().expect_err("the write runs past 4 KiB");

    assert!(matches!(fault, Fault::AddressOutOfRange { .. }));
}

#[test]
fn font_sprites_are_addressable() {
    // LD V0, 0xA ; LD F, V0 ; LD I, ...
    let mut chip9 = machine(&[0x600A, 0xF029]);
    run(&mut chip9, 2);

    let address = chip9.index();
    assert_eq!(address, 0xA * FONT_CHAR_SIZE);

    let expected = &FONT[usize::from(address)..usize::from(address + FONT_CHAR_SIZE)];
    assert_eq!(
        &chip9.memory()[address as usize..(address + FONT_CHAR_SIZE) as usize],
        expected
    );
}

#[test]
fn the_font_character_wraps_round_the_alphabet() {
    // CHIP-8 masked this to a nibble because it only had sixteen characters.
    // CHIP-9 has thirty-six, so 0xFA lands on 250 % 36 = 34, which is 'Y'.
    let mut chip9 = machine(&[0x60FA, 0xF029]);
    run(&mut chip9, 2);

    assert_eq!(chip9.index(), 34 * FONT_CHAR_SIZE);
}

#[test]
fn clear_screen_empties_the_framebuffer() {
    let mut chip9 = machine(&[0xA300, 0xD005, 0x00E0]);
    chip9.write_memory(0x300, 0xFF);

    run(&mut chip9, 2);
    assert!(lit_pixels(&chip9) > 0);

    step(&mut chip9);
    assert_eq!(lit_pixels(&chip9), 0);
}

#[test]
fn the_redraw_flag_follows_the_framebuffer() {
    // LD V0, 1 ; DRW V0, V0, 0 (no rows, so nothing is drawn but D is still a
    // draw) ; LD V1, 1
    let mut chip9 = machine(&[0xD000, 0x6101]);
    assert!(
        chip9.take_redraw(),
        "the cleared screen has to be painted once"
    );
    assert!(!chip9.take_redraw());

    step(&mut chip9);
    assert!(chip9.take_redraw());

    step(&mut chip9);
    assert!(!chip9.take_redraw(), "LD does not touch the screen");
}

// --- the keypad -----------------------------------------------------------

#[test]
fn skip_instructions_read_the_keypad() {
    // LD V0, 7 ; SKP V0 ; JP 0x208 ; JP 0x20A
    let mut chip9 = machine(&[0x6007, 0xE09E, 0x1208, 0x120A]);
    chip9.keypad_mut().set_pressed(0x7, true);
    run(&mut chip9, 3);

    assert_eq!(
        chip9.pc(),
        0x20A,
        "the key was down, so the jump was skipped"
    );

    chip9.reset();
    chip9.keypad_mut().set_pressed(0x7, false);
    run(&mut chip9, 3);

    assert_eq!(chip9.pc(), 0x208);
}

#[test]
fn waiting_for_a_key_blocks_until_it_is_released() {
    // LD V0, K ; CLS
    let mut chip9 = machine(&[0xF00A, 0x00E0]);

    step(&mut chip9);
    assert_eq!(chip9.pc(), 0x200, "still on the same instruction");
    assert!(chip9.is_awaiting_key());

    chip9.keypad_mut().set_pressed(0xB, true);
    step(&mut chip9);
    assert_eq!(chip9.pc(), 0x200, "a key that is still held does not count");

    chip9.keypad_mut().set_pressed(0xB, false);
    step(&mut chip9);

    assert_eq!(chip9.register(0), 0xB);
    assert_eq!(chip9.pc(), 0x202);
    assert!(!chip9.is_awaiting_key());
}

#[test]
fn keys_released_before_the_wait_started_are_ignored() {
    let mut chip9 = machine(&[0xF00A, 0x00E0]);

    chip9.keypad_mut().set_pressed(0x3, true);
    chip9.keypad_mut().set_pressed(0x3, false);

    step(&mut chip9);
    assert!(
        chip9.is_awaiting_key(),
        "the stale release must not satisfy Fx0A"
    );
    assert_eq!(chip9.pc(), 0x200);
}

#[test]
fn the_keypad_covers_all_sixteen_keys_exactly_once() {
    use chip9::keypad::{Keypad, KEY_BINDINGS, KEY_COUNT};

    let mut seen = [false; KEY_COUNT];

    for &(keycode, key) in &KEY_BINDINGS {
        assert!(usize::from(key) < KEY_COUNT, "{key:#X} is not a keypad key");
        assert!(!seen[usize::from(key)], "{key:#X} is bound twice");
        seen[usize::from(key)] = true;

        assert_eq!(Keypad::key_for(keycode), Some(key));
        assert_eq!(Keypad::keycode_for(key), Some(keycode));
    }

    assert!(
        seen.iter().all(|&bound| bound),
        "every key must be reachable"
    );
}

#[test]
fn releasing_a_key_that_was_never_held_is_not_a_release() {
    use chip9::keypad::Keypad;

    let mut keypad = Keypad::new();
    keypad.set_pressed(0x4, false);
    assert_eq!(keypad.take_released(), None);

    keypad.set_pressed(0x4, true);
    assert!(keypad.is_down(0x4));
    keypad.set_pressed(0x4, false);

    assert_eq!(keypad.take_released(), Some(0x4));
    assert_eq!(
        keypad.take_released(),
        None,
        "a release is only reported once"
    );
}

#[test]
fn out_of_range_keys_are_ignored() {
    use chip9::keypad::Keypad;

    let mut keypad = Keypad::new();
    keypad.set_pressed(0x10, true);

    assert!(!keypad.is_down(0x10));
    assert_eq!(keypad.take_released(), None);
}
