use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// Points the linker at the SDL2 import library vendored in `lib/` and copies
/// `SDL2.dll` next to the produced executable so `cargo run` works unmodified.
fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let lib_dir = manifest_dir.join("lib");

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", lib_dir.display());
    println!("cargo:rustc-link-search=native={}", lib_dir.display());

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let dll = lib_dir.join("SDL2.dll");
    if !dll.exists() {
        println!("cargo:warning=lib/SDL2.dll not found; the binary will need SDL2.dll on PATH");
        return;
    }

    // OUT_DIR is target/<profile>/build/<crate>-<hash>/out, so the executable
    // directory is four components up.
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let Some(exe_dir) = out_dir.ancestors().nth(3) else {
        return;
    };

    copy_if_stale(&dll, &exe_dir.join("SDL2.dll"));
    copy_if_stale(&dll, &exe_dir.join("deps").join("SDL2.dll"));
}

fn copy_if_stale(from: &Path, to: &Path) {
    if let Some(parent) = to.parent() {
        if !parent.exists() {
            return;
        }
    }

    let up_to_date = matches!(
        (fs::metadata(from).and_then(|m| m.modified()), fs::metadata(to).and_then(|m| m.modified())),
        (Ok(src), Ok(dst)) if dst >= src
    );

    if !up_to_date {
        let _ = fs::copy(from, to);
    }
}
