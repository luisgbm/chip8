//! The command line front end for [`chip8::asm`].
//!
//! ```text
//! cargo run --bin asm -- programs/leap.asm roms/leap.ch8 [--listing]
//! ```

use std::path::PathBuf;
use std::process::ExitCode;

use chip8::asm::assemble;

const USAGE: &str = "usage: asm <source.asm> <output.ch8> [--listing]";

fn main() -> ExitCode {
    let mut paths: Vec<PathBuf> = Vec::new();
    let mut listing = false;

    for argument in std::env::args().skip(1) {
        match argument.as_str() {
            "--listing" => listing = true,
            "-h" | "--help" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            other if other.starts_with('-') => {
                eprintln!("asm: unknown option {other}\n{USAGE}");
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
            eprintln!("asm: {}: {error}", source_path.display());
            return ExitCode::FAILURE;
        }
    };

    let assembly = match assemble(&source) {
        Ok(assembly) => assembly,
        Err(error) => {
            eprintln!("{}: {error}", source_path.display());
            return ExitCode::FAILURE;
        }
    };

    if listing {
        for line in &assembly.listing {
            let bytes: String = line
                .bytes
                .iter()
                .map(|byte| format!("{byte:02X}"))
                .collect();
            println!("{:03X}  {bytes:<10}  {}", line.address, line.source);
        }
    }

    if let Err(error) = std::fs::write(rom_path, &assembly.rom) {
        eprintln!("asm: {}: {error}", rom_path.display());
        return ExitCode::FAILURE;
    }

    println!(
        "{} -> {} ({} bytes)",
        source_path.display(),
        rom_path.display(),
        assembly.rom.len()
    );

    ExitCode::SUCCESS
}
