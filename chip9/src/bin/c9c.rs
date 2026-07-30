//! The command line front end for [`chip9::lang`].
//!
//! ```text
//! cargo run --bin c9c -- programs/leap.c9 roms/leap.ch8 [--asm programs/leap.asm]
//! ```

use std::path::PathBuf;
use std::process::ExitCode;

use chip9::lang::compile;

const USAGE: &str = "usage: c9c <source.c9> <output.ch8> [--asm <output.asm>]";

fn main() -> ExitCode {
    let mut paths: Vec<PathBuf> = Vec::new();
    let mut assembly_path: Option<PathBuf> = None;
    let mut arguments = std::env::args().skip(1);

    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--asm" => {
                let Some(path) = arguments.next() else {
                    eprintln!("c9c: --asm needs a path\n{USAGE}");
                    return ExitCode::from(2);
                };
                assembly_path = Some(PathBuf::from(path));
            }
            "-h" | "--help" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            other if other.starts_with('-') => {
                eprintln!("c9c: unknown option {other}\n{USAGE}");
                return ExitCode::from(2);
            }
            other => paths.push(PathBuf::from(other)),
        }
    }

    let [source_path, rom_path] = paths.as_slice() else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };

    let source = match std::fs::read_to_string(source_path) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("c9c: {}: {error}", source_path.display());
            return ExitCode::FAILURE;
        }
    };

    let compiled = match compile(&source) {
        Ok(compiled) => compiled,
        Err(error) => {
            eprintln!("{}: {error}", source_path.display());
            return ExitCode::FAILURE;
        }
    };

    if let Some(assembly_path) = &assembly_path {
        if let Err(error) = std::fs::write(assembly_path, &compiled.assembly) {
            eprintln!("c9c: {}: {error}", assembly_path.display());
            return ExitCode::FAILURE;
        }
        println!("{} -> {}", source_path.display(), assembly_path.display());
    }

    if let Err(error) = std::fs::write(rom_path, &compiled.rom) {
        eprintln!("c9c: {}: {error}", rom_path.display());
        return ExitCode::FAILURE;
    }

    println!(
        "{} -> {} ({} bytes)",
        source_path.display(),
        rom_path.display(),
        compiled.rom.len()
    );

    ExitCode::SUCCESS
}
