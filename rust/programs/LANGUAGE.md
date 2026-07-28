# Writing CHIP-8 programs in C8

`TUTORIAL.md` covers the assembler, where one line of source is one instruction
and you keep track of the registers yourself. This covers **C8**, a small
C-like language that does the bookkeeping for you and compiles down to the same
instructions.

The two are not rivals. C8 is easier to read and quicker to change; assembly
lets you say exactly what you mean. `abc123` and `leap` are each written both
ways, in `programs/*.asm` and `programs/*.c8`, and a test asserts that both
routes produce the very same ROM. Read them side by side.

## Running the compiler

```
cargo run --bin c8c -- programs/hello.c8 roms/hello.ch8
```

Add `--asm out.asm` to keep the assembly it wrote, which is the best way to see
what a piece of source turns into:

```
cargo run --bin c8c -- programs/hello.c8 roms/hello.ch8 --asm out.asm
```

Then run it:

```
cargo run
```

## What the machine can do, and so what the language can do

A CHIP-8 has sixteen 8-bit registers, one 12-bit address register, and no
stack for values. There is no multiplication, no division, and nowhere to spill
a temporary. So C8 has:

- no types — every value is a byte
- no local variables — a variable is a register, and there are sixteen
- no arguments and no return values — functions are subroutines
- no `&&` or `||` — use nested `if`s, or `goto`

What is left is close enough to C to read at a glance: expressions, `if` and
`else`, `while`, `do`/`while`, `loop`, `break`, `continue`, `goto`, labels, and
functions.

Three registers are the compiler's own:

| Register | Used for |
| --- | --- |
| `V0` | the accumulator: anything worked out on the way to a statement lands here |
| `V1` | the right hand side of whatever `V0` is being combined with |
| `VF` | the flag, which the machine itself overwrites on carries and collisions |

The other thirteen are yours.

## Hello World

The built in font has glyphs for the sixteen hex digits and nothing else, so
`HELLO WORLD` is out of reach without drawing your own letters. `ABC123` is
not: `A`, `B` and `C` are digits as far as the font is concerned. Here is the
whole program.

```c
const LENGTH  = 6;
const SPACING = 5;
const LEFT    = 17;
const TOP     = 13;

var x @ V2, y @ V3;
var at @ V4;

byte message[] = { $A, $B, $C, 1, 2, 3 };

fn main() {
    clear();

    x = LEFT;
    y = TOP;
    at = 0;

    do {
        draw(x, y, font(message[at]));
        x += SPACING;
        at += 1;
    } while (at != LENGTH);

    loop {}
}
```

That is `programs/abc123.c8`. Compile it and it produces `roms/abc123.ch8`,
byte for byte the same file the assembler makes from `programs/abc123.asm`.

## Building it up, line by line

### Constants

```c
const LENGTH = 6;
const MAX_X  = 64 - PLAYER_W;
```

A `const` is a name for a number, worked out at compile time. Arithmetic on
constants is folded away, so `MAX_X` costs nothing at run time. Constants can
refer to earlier constants.

### Variables

```c
var x @ V2, y @ V3;
var at;
```

A variable **is** a register. `@ V2` pins it to one; leave the pin off and the
compiler picks a free one. Pinning matters more than it looks: `store` and
`restore` work on `V0` through `Vx`, and a program that wants a particular
layout has to say so.

There are no locals and no scopes. Every variable is visible everywhere, like a
global in C.

### Functions

```c
fn main() {
    draw_scene();
}

fn draw_scene() {
    clear();
}
```

**The first function in the file is where the program starts.** The rest are
subroutines, reached with `CALL` and returning with `RET`. They take no
arguments and return nothing — pass values in variables.

A function returns at the closing brace, so you rarely write `return`. It is
there for the early exits:

```c
fn move_left() {
    if (state == IN_PIT) return;
    if (x == 0) return;
    x -= 1;
}
```

### Expressions

```c
x = 7;              // LD V2, 7
y = x;              // LD V3, V2
x += 1;             // ADD V2, 1
x -= 1;             // LD V0, 1  /  SUB V2, V0
flags ^= 1;
x = 8 - y;          // SUBN, because the constant is on the left
y = x >> 1;         // LD V3, V2  /  SHR V3
```

`+`, `-`, `&`, `|`, `^`, `>>` and `<<` are all there, and so are the `+=` forms.
A few things follow from the hardware rather than from taste:

- **Shifts are by one.** `x >>= 2` is an error, because the machine has one
  shift instruction and it moves one bit.
- **Subtraction has no immediate form.** `x -= 1` has to load the 1 into a
  register first, so it is three bytes where `x += 1` is two.
- **`-` sets the flag.** After `a - b`, `VF` is 1 when `a >= b`. This is how
  comparisons work, below.

### Conditions

```c
if (x != 3) x += 1;
if (x == 3) { x += 1; y += 1; }
if (x >= 4) return;
if (pressed(k)) move_left();
```

Equality against a constant or another variable is one instruction. So is a
key test. The relational operators are not: the machine cannot compare, only
subtract, so `x >= 4` becomes "work out `x - 4`, then look at the flag".

The compiler leans on the machine's skip instructions. A body of exactly one
instruction hangs straight off the skip:

```
    SE   V2, 3          ; if (x != 3) x += 1;
    ADD  V2, 1
```

and anything longer gets a jump around it:

```
    SE   V2, 3          ; if (x == 3) { x += 1; y += 1; }
    JP   _L0
    ADD  V2, 1
    ADD  V3, 1
_L0:
```

There is no `&&` or `||`. Nest the `if`s, or fall through with `goto`.

### Loops

```c
loop { }                            // forever
while (x != 8) { x += 1; }          // test at the top
do { x += 1; } while (x != 8);      // test at the bottom
```

`break` leaves the nearest loop and `continue` starts its next turn. `do`/`while`
is the cheapest of the three, because the test is already where the jump is.

### `goto` and labels

```c
    if (state != ON_FLOOR) goto falling;
    ...
falling:
    ...
```

`goto` is not a last resort here. A CHIP-8 program is a web of jumps, and being
able to write one directly is what lets `leap.c8` come out as exactly the same
instructions a person would have written by hand.

### Memory

```c
byte  message[] = { $A, $B, $C, 1, 2, 3 };
sprite player   = { $70, $70, $F8, $70, $50, $50 };
```

`byte` and `sprite` mean the same thing to the compiler; the two spellings just
say what you meant. Reading one back:

```c
at = 0;
d = message[at];        // LD I, message  /  ADD I, V4  /  LD V0, [I]
d = message[1];         // LD I, message + 1  /  LD V0, [I]
```

`I` is writable, which is worth knowing when a loop draws the same sprite over
and over:

```c
I = floor;              // hoisted out by hand
fx = 0;
do {
    draw(fx, fy, floor);    // the compiler sees I is already right
    fx += 8;
} while (fx != 64);
```

### Drawing

```c
clear();
draw(x, y, player);             // a sprite, as tall as it was declared
draw(x, y, font(digit));        // a font glyph, always five rows
draw(x, y, 3);                  // three rows from wherever I points
```

`draw` is `DRW`, so it draws by exclusive or: drawing the same sprite in the
same place a second time rubs it out. That is how `leap` moves the player.

### The timers

```c
delay = 2;
if (delay != 0) goto wait;
sound = 12;
```

`delay` counts down at 60 Hz and can be read back; `sound` counts down too and
beeps while it does. Reading `delay` is the usual way to pace a game.

### The rest

```c
r = random($0F);        // a random byte, masked
k = key();              // stops the machine until a key goes down
bcd(n);                 // the three digits of n, to I, I+1, I+2
store(V2);              // V0..V2 out to memory at I
restore(V2);            // and back again
```

## Syntax reference

| Form | Means |
| --- | --- |
| `const NAME = expr;` | a compile time number |
| `var a, b @ V3;` | variables, optionally pinned to a register |
| `byte name[] = { ... };` | bytes in memory |
| `sprite name = { ... };` | the same, spelled for sprites |
| `fn name() { ... }` | a subroutine; the first one is the entry point |
| `label:` | a jump target |
| `goto label;` | jump |
| `return;` | `RET` |
| `if (c) s` / `else s` | |
| `while (c) s` | test at the top |
| `do s while (c);` | test at the bottom |
| `loop s` | forever |
| `break;` / `continue;` | leave, or restart, the nearest loop |
| `// ...` and `/* ... */` | comments |

Numbers can be written `31`, `0x1F`, `$1F`, `#1F` or `0b11111`.

## Builtins

| Call | Compiles to |
| --- | --- |
| `clear()` | `CLS` |
| `draw(x, y, sprite)` | `LD I, sprite` + `DRW` |
| `draw(x, y, font(d))` | `LD F, Vd` + `DRW ..., 5` |
| `draw(x, y, n)` | `DRW ..., n` from wherever `I` is |
| `pressed(k)` | `SKP` / `SKNP`, only inside an `if` |
| `key()` | `LD Vx, K` |
| `random(mask)` | `RND` |
| `bcd(v)` | `LD B, Vv` |
| `store(Vx)` / `restore(Vx)` | `LD [I], Vx` / `LD Vx, [I]` |

## What the compiler does for you

It is a small compiler and it does exactly two clever things, both aimed at not
emitting work that has already been done.

**It remembers what is in `V0` and in `I`.** It follows every path through the
program, and where every path agrees on the value, a load of that same value is
dropped. This is why the second `draw` of the same sprite has no `LD I`, and
why a comparison can reuse the subtraction the one before it did:

```c
if (x < HOLE_MIN) return;
if (x - HOLE_MIN >= HOLE_W) return;      // x - HOLE_MIN is already in V0
```

```
    LD   V0, V2
    LD   V1, HOLE_MIN
    SUB  V0, V1
    SE   VF, 1
    RET
    LD   V1, HOLE_W         ; and straight on with the second test
    SUB  V0, V1
    SE   VF, 0
    RET
```

Where paths disagree it emits the load, so this never changes what a program
does — only how many bytes it takes to say it.

**It will not reuse a stale flag.** Dropping a subtraction would drop the `VF`
it set, so the compiler only does it when `VF` is written again before anything
reads it. Two identical comparisons in a row therefore still subtract twice.

That is the whole of it. There is no register allocator, no inlining and no
instruction scheduling. What you write is what you get, minus the loads you did
not need.

## Things that catch people out

- **`V0` and `V1` are not yours.** Anything you leave in them is gone by the
  next statement that computes something. Pin a variable to them only when you
  know nothing else is happening, as `draw_scene` in `leap.c8` does.
- **`VF` is not yours either.** `ADD`, `SUB`, `SHR`, `SHL` and `DRW` all write
  it.
- **`x -= 1` is bigger than `x += 1`.** No subtract immediate.
- **`if (a > b)` costs the same as `if (a < b)`** — the compiler swaps the sides
  — but both cost five instructions against two for `!=`.
- **The first function is the entry point**, wherever you put it in the file.
  Moving it changes where the program starts.
- **Registers written out as `V2` only work where a register is meant**, which
  in practice is `var ... @ V2` and `store`/`restore`.

## Where to go next

- `programs/abc123.c8` and `programs/abc123.asm` — the same six characters,
  both ways.
- `programs/leap.c8` and `programs/leap.asm` — a whole game, both ways. Compile
  one and assemble the other and you get identical files.
- `TUTORIAL.md` — the assembler, and the machine underneath all of this.
- `src/lang/` — the compiler. `lexer.rs`, `parser.rs` and `codegen.rs`, in that
  order; the interesting part is `analyse` at the bottom of `codegen.rs`.
