//! Checks that every bundled program is loadable and actually runs.

use chip9::cpu::{Chip9, Fault, MAX_PROGRAM_SIZE};
use chip9::programs::{BuiltinProgram, BUILTIN_PROGRAMS};

/// Programs that end by running past their own last instruction. Memory is
/// zeroed, so they stop on `Fault::UnknownOpcode { opcode: 0x0000 }` instead of
/// spinning forever like the JavaScript version did.
const RUNS_OFF_THE_END: [&str; 2] = ["Stack 1", "Stack 2"];

fn is_expected_fault(program: &BuiltinProgram, fault: Fault) -> bool {
    RUNS_OFF_THE_END.contains(&program.name)
        && matches!(fault, Fault::UnknownOpcode { opcode: 0x0000, .. })
}

#[test]
fn there_is_at_least_one_program_for_every_category() {
    use chip9::programs::Category;

    for category in [Category::Game, Category::Demo, Category::Test] {
        assert!(
            BUILTIN_PROGRAMS
                .iter()
                .any(|program| program.category == category),
            "no program in {}",
            category.label()
        );
    }
}

#[test]
fn every_program_is_described_and_fits_in_memory() {
    for program in BUILTIN_PROGRAMS {
        assert!(!program.name.is_empty());
        assert!(
            !program.description.is_empty(),
            "{} has no description",
            program.name
        );
        assert!(
            program.cycles_per_frame > 0,
            "{} would never run",
            program.name
        );

        assert!(!program.rom.is_empty(), "{} is empty", program.name);
        assert!(
            program.rom.len() <= MAX_PROGRAM_SIZE,
            "{} is {} bytes, which does not fit",
            program.name,
            program.rom.len()
        );
    }
}

#[test]
fn program_names_are_unique() {
    let mut names: Vec<&str> = BUILTIN_PROGRAMS
        .iter()
        .map(|program| program.name)
        .collect();
    let total = names.len();
    names.sort_unstable();
    names.dedup();

    assert_eq!(names.len(), total, "two programs share a name");
}

#[test]
fn every_program_loads_and_runs_without_an_unexpected_fault() {
    for program in BUILTIN_PROGRAMS {
        let mut chip9 = Chip9::with_seed(0xC0FF_EE00_1234_5678);
        chip9
            .load(program.rom)
            .unwrap_or_else(|error| panic!("{}: {error}", program.name));

        assert_eq!(chip9.program(), program.rom);

        // A couple of seconds of emulated time.
        for _ in 0..120 {
            if let Err(fault) = chip9.step_many(program.cycles_per_frame) {
                assert!(
                    is_expected_fault(program, fault),
                    "{} stopped: {fault} ({})",
                    program.name,
                    fault.hint()
                );
                break;
            }

            chip9.tick_timers();
        }
    }
}

#[test]
fn the_ibm_logo_shows_up_on_screen() {
    let program = BUILTIN_PROGRAMS
        .iter()
        .find(|program| program.name == "IBM Logo")
        .expect("the IBM logo is bundled");

    let mut chip9 = Chip9::with_seed(1);
    chip9.load(program.rom).expect("it fits");
    chip9.step_many(40).expect("no fault");

    let lit = chip9.framebuffer().iter().filter(|&&pixel| pixel).count();

    assert!(
        lit > 100,
        "expected the logo to be drawn, only {lit} pixels are lit"
    );
    assert!(chip9.take_redraw());
}

#[test]
fn pong_draws_its_court_and_waits_for_a_player() {
    let program = BUILTIN_PROGRAMS
        .iter()
        .find(|program| program.name == "Pong")
        .expect("pong is bundled");

    let mut chip9 = Chip9::with_seed(0xDEAD_BEEF_CAFE_F00D);
    chip9.load(program.rom).expect("it fits");

    for _ in 0..60 {
        chip9.step_many(program.cycles_per_frame).expect("no fault");
        chip9.tick_timers();
    }

    let lit = chip9.framebuffer().iter().filter(|&&pixel| pixel).count();
    assert!(
        lit > 20,
        "expected paddles and a net, only {lit} pixels are lit"
    );
}
