# Writing CHIP-8 programs

A tutorial for the assembler that lives in [`src/asm.rs`](../src/asm.rs). By the
end of it you will have written a program that puts `ABC123` on the screen using
the interpreter's own font, and you will know enough to read
[`leap.asm`](leap.asm), the game in this folder.

No prior assembly experience is assumed. CHIP-8 is a good first machine: it has
sixteen registers, thirty five instructions, and you can hold all of it in your
head at once.

- [Running the assembler](#running-the-assembler)
- [The machine](#the-machine)
- [The program](#the-program)
- [Building it up, line by line](#building-it-up-line-by-line)
- [Syntax reference](#syntax-reference)
- [Instruction reference](#instruction-reference)
- [Things that catch people out](#things-that-catch-people-out)
- [Where to go next](#where-to-go-next)

## Running the assembler

```console
$ cargo run --bin asm -- programs/abc123.asm roms/abc123.ch8
programs/abc123.asm -> roms/abc123.ch8 (34 bytes)
```

Add `--listing` to see what each line turned into, which is the fastest way to
check that an instruction assembled the way you meant it to:

```console
$ cargo run --bin asm -- programs/abc123.asm roms/abc123.ch8 --listing
200  00E0        CLS
202  6211        LD V2, LEFT
204  630D        LD V3, TOP
...
```

The left column is the address the instruction was placed at, the middle column
is the bytes, and the right column is your source.

Run the result with either of:

```console
$ cargo run --release -- roms/abc123.ch8
$ cargo run --release                     # then pick it off the menu
```

## The machine

Everything a CHIP-8 program can touch:

| | |
|---|---|
| **Registers** | `V0` to `VF`, one byte each. `VF` is special: several instructions overwrite it with a flag, so never keep anything there. |
| **`I`** | A twelve bit address register. Sprites are drawn from wherever `I` points. |
| **Memory** | 4096 bytes. Your program is loaded at `$200` and the assembler starts counting from there. Below that is the interpreter's own area, which is where the built in font sits. |
| **Screen** | 64 by 32 pixels, one bit each. Nothing but sprites can be drawn, and they are drawn with XOR. |
| **Stack** | Sixteen entries, used only by `CALL` and `RET`. You cannot read it. |
| **Delay timer** | Counts down to zero at 60 Hz. Programs use it as a clock. |
| **Sound timer** | The same, but the machine beeps while it is above zero. |
| **Keypad** | Sixteen keys, `0` to `F`. On this interpreter they are laid out as `1234` / `QWER` / `ASDF` / `ZXCV` on the host keyboard, matching the original hardware, so keypad `5` is host `W`. |

There are no signed numbers, no multiplication, no memory to speak of, and no
way to print. Everything is sprites.

### The built in font

The interpreter keeps a sprite for each of the sixteen hex digits in low memory.
Each is five rows tall and four columns wide. You never need to know where they
are, because `LD F, Vx` points `I` at the glyph for the digit in `Vx`:

```
LD  V0, $C      ; the digit C
LD  F, V0       ; I now points at the glyph for C
DRW V1, V2, 5   ; draw it, five rows tall
```

That font is the whole reason this tutorial spells `ABC123` and not something
friendlier: `A` to `F` are digits, so they have glyphs, but `H`, `L`, `O` and
the rest of the alphabet do not. If you want words you have to draw the letters
yourself, which is what `leap.asm` does for its `GAME OVER` screen.

## The program

Here is the whole thing, and it is only fourteen instructions.
[`abc123.asm`](abc123.asm):

```asm
LENGTH      = 6         ; how many characters there are to draw
CHAR_H      = 5         ; every glyph in the built in font is five rows tall
SPACING     = 5         ; four columns wide, so this leaves a one column gap
LEFT        = 17        ; (64 - (LENGTH * SPACING - 1)) / 2, near enough
TOP         = 13        ; (32 - CHAR_H) / 2, near enough


start:
    CLS
    LD   V2, LEFT               ; where the next glyph goes
    LD   V3, TOP
    LD   V4, 0                  ; how far along the message we are

next_char:
    LD   I, message             ; I has to be set from scratch every time,
    ADD  I, V4                  ; because Fx1E moves it and Fx65 may too
    LD   V0, [I]                ; V0 = the digit stored at message + V4

    LD   F, V0                  ; point I at the font glyph for that digit
    DRW  V2, V3, CHAR_H         ; and put it on screen

    ADD  V2, SPACING            ; move along for the next one
    ADD  V4, 1
    SE   V4, LENGTH
    JP   next_char

stop:
    JP   stop                   ; a CHIP-8 program has nowhere to return to


message:
    DB   $A, $B, $C, 1, 2, 3
```

Assemble and run it and you get:

```
.................####.###..####...#..####.####..................
.................#..#.#..#.#.....##.....#....#..................
.................####.###..#......#..####.####..................
.................#..#.#..#.#......#..#.......#..................
.................#..#.###..####..###.####.####..................
```

## Building it up, line by line

### Constants

```asm
LENGTH      = 6
```

`NAME = value` gives a name to a number. Nothing is emitted; the assembler just
remembers it. A constant may be built out of numbers, other constants and
labels, using `+` and `-`:

```asm
HOLE_X      = 28
FOOT_X      = 2
HOLE_MIN    = HOLE_X - FOOT_X       ; 26
```

Naming your numbers is worth the effort. Half of reading assembly is working out
what a bare `26` meant, and the other half is finding the three other places you
were supposed to change when you edited it.

### Labels

```asm
next_char:
```

A label is a name for the address of whatever comes next. Labels may be used
before they are written, because the assembler makes two passes: the first works
out where everything lands, the second fills the addresses in.

That is all a label is. There are no functions, no scopes and no locals; `CALL
draw_scene` just sets the program counter to whatever address `draw_scene:`
ended up at, and `RET` sets it back.

### Clearing the screen

```asm
    CLS
```

The screen is not cleared for you, so a program that draws without clearing
first will draw on top of whatever the last program left behind.

### Setting registers up

```asm
    LD   V2, LEFT
    LD   V3, TOP
    LD   V4, 0
```

`LD Vx, byte` puts a constant in a register. This is the most common instruction
in any CHIP-8 program by a wide margin.

`V2` and `V3` hold the position of the next glyph, and `V4` counts through the
message. Which register holds what is entirely up to you, so write it down at
the top of the file. Both programs in this folder do.

### Reading a byte out of memory

```asm
    LD   I, message
    ADD  I, V4
    LD   V0, [I]
```

There is no `LD Vx, [address]`. Reading memory always goes through `I`:

1. `LD I, message` points `I` at the first byte of the message.
2. `ADD I, V4` moves it along by the offset in `V4`.
3. `LD V0, [I]` fills `V0` from there. This instruction fills `V0` up to `Vx`,
   so `LD V3, [I]` would load four bytes into `V0`, `V1`, `V2` and `V3`.

Note that `I` is set up again on every trip round the loop rather than being
nudged along. That is deliberate: on the original hardware `LD Vx, [I]` left `I`
pointing past the bytes it read, and interpreters disagree about it to this day.
Setting `I` from scratch works everywhere.

### Drawing

```asm
    LD   F, V0
    DRW  V2, V3, CHAR_H
```

`DRW Vx, Vy, n` draws `n` rows starting at the address in `I`, at the pixel
position held in `Vx` and `Vy`. Each row is one byte, so a sprite is always
eight pixels wide and one to fifteen rows tall.

Two things about `DRW` are worth burning into memory:

- **It draws with XOR.** A pixel that was on and is drawn on again goes off.
  Drawing the same sprite twice in the same place rubs it out again, which is
  how everything moves: erase at the old position, draw at the new one.
- **It sets `VF`.** `VF` becomes 1 if any lit pixel was turned off, which is how
  you do collision detection, and 0 otherwise. Either way, whatever you had in
  `VF` is gone.

### Looping

```asm
    ADD  V2, SPACING
    ADD  V4, 1
    SE   V4, LENGTH
    JP   next_char
stop:
```

There are no loops and no `if`. Instead there are four *skip* instructions, and
every one of them skips the single instruction after it when its condition
holds:

| | skips the next instruction when |
|---|---|
| `SE Vx, byte` / `SE Vx, Vy` | they are equal |
| `SNE Vx, byte` / `SNE Vx, Vy` | they are not equal |
| `SKP Vx` | the key numbered `Vx` is held down |
| `SKNP Vx` | it is not |

So `SE V4, LENGTH` followed by `JP next_char` reads as "if `V4` is not `LENGTH`
yet, go round again". Getting the sense backwards is the single most common
mistake in CHIP-8, so read every skip out loud.

For a two way branch, put a `JP` on both sides:

```asm
    SE   V6, ON_FLOOR
    JP   falling            ; taken when V6 is not ON_FLOOR
    JP   standing           ; taken when it is
```

### Stopping

```asm
stop:
    JP   stop
```

A CHIP-8 program has nothing to return to, so it has to end in a loop that jumps
to itself. Leaving it out means the program runs off into memory that was never
written, which this interpreter reports as `unknown opcode 0000` rather than
letting it wander.

### Data

```asm
message:
    DB   $A, $B, $C, 1, 2, 3
```

`DB` lays down bytes exactly as written. Sprites, message text and lookup tables
are all just `DB`.

Data has to sit somewhere the program counter never reaches, which in practice
means after the final `JP`. There is no separate data section, and nothing stops
you from executing your sprites or drawing your code.

Writing a sprite is easier if you draw it in the comments:

```asm
player:                         ; .###.
    DB   $70                    ; .###.
    DB   $70                    ; #####
    DB   $F8                    ; .###.
    DB   $70                    ; .#.#.
    DB   $50                    ; .#.#.
    DB   $50
```

Each byte is one row, most significant bit on the left, so `$F8` is `11111000`.

## Syntax reference

```asm
; a comment, running to the end of the line

NAME = 12 + 4               ; a constant
OTHER = NAME - here         ; may refer to constants above it and to any label

label:                      ; a label on a line of its own
other: CLS                  ; or in front of an instruction

    LD V0, $1F              ; one instruction per line, operands split by commas
    DB $FF, 128, label      ; raw bytes
```

- Mnemonics and register names may be written in any case: `ld v0, dt` is fine.
- Numbers may be written as `31`, `0x1F`, `$1F` or `#1F`.
- Anywhere a number is expected you may write a sum or difference of numbers,
  constants and labels: `LD V0, WIDTH - 1`.
- Indentation and blank lines are ignored.

## Instruction reference

All 35 opcodes of the original CHIP-8, and nothing else, so anything this
assembles will run on any interpreter.

| Opcode | Written as | Does |
|---|---|---|
| `00E0` | `CLS` | Clear the screen |
| `00EE` | `RET` | Return from a subroutine |
| `0nnn` | `SYS addr` | Call machine code. No interpreter implements it |
| `1nnn` | `JP addr` | Jump |
| `2nnn` | `CALL addr` | Call a subroutine |
| `3xkk` | `SE Vx, byte` | Skip the next instruction if `Vx` equals the byte |
| `4xkk` | `SNE Vx, byte` | Skip if it does not |
| `5xy0` | `SE Vx, Vy` | Skip if the two registers are equal |
| `6xkk` | `LD Vx, byte` | `Vx` = byte |
| `7xkk` | `ADD Vx, byte` | `Vx` += byte, wrapping, and **`VF` is not touched** |
| `8xy0` | `LD Vx, Vy` | `Vx` = `Vy` |
| `8xy1` | `OR Vx, Vy` | `Vx` \|= `Vy` |
| `8xy2` | `AND Vx, Vy` | `Vx` &= `Vy` |
| `8xy3` | `XOR Vx, Vy` | `Vx` ^= `Vy` |
| `8xy4` | `ADD Vx, Vy` | `Vx` += `Vy`, and `VF` = 1 on carry |
| `8xy5` | `SUB Vx, Vy` | `Vx` -= `Vy`, and `VF` = 1 when there was **no** borrow, that is when `Vx` >= `Vy` |
| `8xy6` | `SHR Vx` | `Vx` >>= 1, and `VF` = the bit shifted out |
| `8xy7` | `SUBN Vx, Vy` | `Vx` = `Vy` - `Vx`, `VF` as for `SUB` |
| `8xyE` | `SHL Vx` | `Vx` <<= 1, and `VF` = the bit shifted out |
| `9xy0` | `SNE Vx, Vy` | Skip if the two registers differ |
| `Annn` | `LD I, addr` | `I` = address |
| `Bnnn` | `JP V0, addr` | Jump to address + `V0` |
| `Cxkk` | `RND Vx, byte` | `Vx` = a random byte AND the byte |
| `Dxyn` | `DRW Vx, Vy, n` | Draw `n` rows from `I` at (`Vx`, `Vy`), and `VF` = 1 if anything was rubbed out |
| `Ex9E` | `SKP Vx` | Skip if the key numbered `Vx` is down |
| `ExA1` | `SKNP Vx` | Skip if it is not |
| `Fx07` | `LD Vx, DT` | `Vx` = the delay timer |
| `Fx0A` | `LD Vx, K` | Wait for a key, then put its number in `Vx` |
| `Fx15` | `LD DT, Vx` | Delay timer = `Vx` |
| `Fx18` | `LD ST, Vx` | Sound timer = `Vx`, and the machine beeps until it runs out |
| `Fx1E` | `ADD I, Vx` | `I` += `Vx` |
| `Fx29` | `LD F, Vx` | `I` = the font glyph for the digit in `Vx` |
| `Fx33` | `LD B, Vx` | Write `Vx` as three decimal digits at `I`, `I+1`, `I+2` |
| `Fx55` | `LD [I], Vx` | Store `V0` up to `Vx` at `I` |
| `Fx65` | `LD Vx, [I]` | Load `V0` up to `Vx` from `I` |

`SHR` and `SHL` take an optional second register, which interpreters disagree
about. Written with one operand the assembler emits `8xx6` and `8xxE`, which
mean the same thing either way, so use the one operand form.

## Things that catch people out

**Skips only skip one instruction.** And that instruction is two bytes, so a
skip in front of a `DB` of odd length will land in the middle of something.

**`VF` is not yours.** `ADD Vx, Vy`, `SUB`, `SUBN`, `SHR`, `SHL` and `DRW` all
write to it. Read it in the instruction straight after the one that set it, and
never store anything there.

**Subtraction is how you compare.** There is no "less than" instruction, so
comparisons go through `SUB` and the flag it leaves:

```asm
    LD   V0, V2         ; is the player past the near edge of the pit?
    LD   V1, HOLE_MIN
    SUB  V0, V1         ; VF = 1 when V2 >= HOLE_MIN
    SE   VF, 1
    RET                 ; no
```

Remember that `SUB` also clobbers the register you subtract into, so work on a
copy.

**Everything is a byte.** `7xkk` and `8xy4` wrap round at 255 and there is
nothing below zero, so a value that has to go negative has to be biased.
`leap.asm` stores vertical speed with 8 meaning "not moving", so 4 means four
half pixels a tick upwards and 12 means four downwards.

**Pace yourself with the delay timer.** How many instructions run per frame
varies between interpreters, so a program that just loops will run at wildly
different speeds. Set the delay timer and wait for it instead:

```asm
tick:
    LD   V0, DT
    SE   V0, 0
    JP   tick           ; still counting down, go round again
    LD   V0, 2
    LD   DT, V0         ; two frames, so the game runs at 30 Hz
```

**Sprites are eight pixels wide, always.** Narrower ones are drawn by leaving
the low bits of each row clear; the columns are still drawn, they are just XORed
with zero.

## Where to go next

Read [`leap.asm`](leap.asm). It is about 190 instructions and uses everything
above: the delay timer for pacing, `SKP`/`SKNP` for input, subtraction for
comparisons, biased values for velocity, `CALL` for subroutines, XOR for moving
a sprite around, and hand drawn letter sprites for the words the font does not
have.

Then read [`leap.c8`](leap.c8), which is the same game written in
[C8](LANGUAGE.md), the C-like language that compiles to these instructions.
Compiling it produces the very same ROM, so it is a line by line answer to
"what does this assembly look like in a language with `if` and `while`".

The other thing worth doing is reading the interpreter itself.
[`src/cpu.rs`](../src/cpu.rs) is one `match` over the opcode, and every
instruction in the table above is about three lines of Rust.
