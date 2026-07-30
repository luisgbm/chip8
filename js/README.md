# chip8 — JavaScript

The original CHIP-8 interpreter, written entirely in JavaScript with no
dependencies and no build step. It runs in the browser, drawing to a `<canvas>`
and beeping through the Web Audio API.

🎮 **Test it online now with Pong**: https://luisgbm.github.io/chip8/

*Remember to use a modern browser and allow audio in the site's settings to be
able to listen to all the glorious beeps!*

## Running it locally

The files are ES modules, so they have to be served over HTTP rather than opened
from disk. Anything that serves a directory will do:

```sh
cd js
python -m http.server 8000
```

Then open <http://localhost:8000/>.

## Controls

The page loads *Pong*, which is two player: `1` and `4` move the left paddle,
`F` and `Z` the right one.

Those are the keys the CHIP-8 program asks for, but the mapping in
[`keyboard.js`](keyboard.js) numbers the keypad `1` to `16` rather than `0x0` to
`0xF`, so what a program asks for and what the keyboard sends are one apart.
That is one of the bugs the Rust port fixes; see
[`../rust/README.md`](../rust/README.md).

## Running a different program

There is no interface for picking a program — the ROM is an array literal in the
`<script>` block at the bottom of [`index.html`](index.html):

```js
const pong = [0x6a, 0x02, 0x6b, 0x0c, /* ... */];

chip8.loadProgram(pong);
```

[`programs.txt`](programs.txt) has more of them, one array per program: *IBM
Logo*, *Guess*, *Computer*, *Space Invaders*, a few test programs, and the
*Pong* that is already on the page. Paste one in and change the argument to
`loadProgram`.

The Rust port in [`../rust`](../rust) puts all of these behind an intro screen
and can load `.ch8` files from disk, if editing the page to change program gets
old.

## Layout

| File                             | What it is                                    |
|----------------------------------|-----------------------------------------------|
| [`chip8.js`](chip8.js)           | The interpreter: memory, registers, opcodes   |
| [`video.js`](video.js)           | The 64x32 framebuffer and the canvas          |
| [`audio.js`](audio.js)           | The buzzer                                    |
| [`keyboard.js`](keyboard.js)     | The sixteen key hexadecimal keypad            |
| [`index.html`](index.html)       | The page, the ROM and the animation loop      |
| [`programs.txt`](programs.txt)   | More programs, as JavaScript byte arrays      |

`index.html` runs ten instructions per `requestAnimationFrame`, so roughly 600
instructions a second on a 60 Hz display, and repaints the canvas only when the
interpreter says the framebuffer changed.

## Known bugs

This version is kept as it was written. It has a handful of real bugs — a
collision flag that is set almost every time a sprite is drawn, sprites that
wrap and write outside the framebuffer, arithmetic that escapes `0..=255`,
timers that run about 11% fast, and the keypad numbering above. They are all
listed, with what the correct behaviour is, under *Bugs fixed* in
[`../rust/README.md`](../rust/README.md), which is where they are fixed.
