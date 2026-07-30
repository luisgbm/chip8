# chip8

A CHIP-8 interpreter/emulator, written twice: once in JavaScript for the
browser, and once in Rust on top of SDL2.

According to [Wikipedia](https://en.wikipedia.org/wiki/CHIP-8),

>CHIP-8 is an interpreted programming language, developed by Joseph Weisbecker. It was initially used on the COSMAC VIP and Telmac 1800 8-bit microcomputers in the mid-1970s. CHIP-8 programs are run on a CHIP-8 virtual machine. It was made to allow video games to be more easily programmed for these computers.

## [`js/`](js) — the original

Plain JavaScript, no dependencies and no build step: a `<canvas>`, the Web Audio
API and about four files.

🎮 **Test it online now with Pong**: https://luisgbm.github.io/chip8/

*Remember to use a modern browser and allow audio in the site's settings to be
able to listen to all the glorious beeps!*

I put some more programs in the [`js/programs.txt`](js/programs.txt) file, you
should be able to load them by editing the [`js/index.html`](js/index.html)
file. See [`js/README.md`](js/README.md).

## [`rust/`](rust) — the port

The same interpreter in Rust, running on SDL2 instead of the browser. It bundles
every program from `js/programs.txt` behind an intro screen, can load `.ch8`
files from disk, and fixes a handful of bugs the JavaScript version carries —
see [`rust/README.md`](rust/README.md).

```sh
cd rust
cargo run --release
```

It also comes with two ways to write CHIP-8 programs — an assembler, and a
small C-like language called C8 that compiles to CHIP-8 — and a game written
with both, *Leap*. If you want to write a program yourself, start at
[`rust/programs/LANGUAGE.md`](rust/programs/LANGUAGE.md) for the language or
[`rust/programs/TUTORIAL.md`](rust/programs/TUTORIAL.md) for the assembler.
