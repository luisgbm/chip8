# Writing CHIP-9 programs in C9

`TUTORIAL.md` covers the assembler, where one line of source is one instruction
and you keep track of the registers yourself. This covers **C9**, a small
C-like language that does the bookkeeping for you and compiles down to the same
instructions.

The two are not rivals. C9 is easier to read and quicker to change; assembly
lets you say exactly what you mean. `abc123` and `leap` are each written both
ways, in `programs/*.asm` and `programs/*.c9`, and a test asserts that both
routes produce the very same ROM. Read them side by side.

## Running the compiler

```
cargo run --bin c9c -- programs/hello.c9 roms/hello.ch8
```

Add `--asm out.asm` to keep the assembly it wrote, which is the best way to see
what a piece of source turns into:

```
cargo run --bin c9c -- programs/hello.c9 roms/hello.ch8 --asm out.asm
```

Then run it:

```
cargo run
```

## What the machine can do, and so what the language can do

A CHIP-9 has sixteen 8-bit registers, one 12-bit address register, and a
sixty-four byte data stack that `PUSH` and `POP` reach. It multiplies and
divides. It has no types wider than a byte and nowhere to put a structure. So
C9 has:

- no types — every value is a byte, and arithmetic wraps at 256
- no pointers, no structs, no strings — `byte` arrays and nothing else
- no expressions of unbounded depth — see *One side has to be simple*, below

What is left is close enough to C to read at a glance: expressions with `&&`
and `||`, `if` and `else`, `while`, `do`/`while`, `loop`, `break`, `continue`,
`goto`, labels, and functions that take arguments, keep local variables and
return values.

Three registers are the compiler's own:

| Register | Used for |
| --- | --- |
| `V0` | the accumulator: anything worked out on the way to a statement lands here, and a returned value comes back here |
| `V1` | the right hand side of whatever `V0` is being combined with |
| `VF` | the flag, which the machine itself overwrites on carries, collisions and remainders |

The other thirteen are shared out between your globals, your parameters and
your locals.

## Hello World

CHIP-8's font stops at `F`, because its sixteen glyphs are the sixteen
hexadecimal digits. CHIP-9 carries the idea on into base thirty six: the font
runs `0` to `9` and then `A` to `Z`, so `font(x)` reaches every letter. Here is
`programs/hello.c9`, cut down to its bones.

```c
const GLYPH_W = 5;
const LEFT    = 17;

var x @ V2, y @ V3;

byte message[] = { 'H', 'E', 'L', 'L', 'O', 'W', 'O', 'R', 'L', 'D' };

fn main() {
    clear();

    y = 10;
    write(0, 5);

    y = 18;
    write(5, 5);

    loop {}
}

// Writes `count` letters starting at `from`.
fn write(from, count) {
    var at, letter;

    x = LEFT;
    at = 0;

    while (at < count) {
        letter = message[from + at];
        draw(x, y, font(letter));
        x += GLYPH_W;
        at += 1;
    }
}
```

Four columns is not much room for an alphabet, so `O` is the same shape as `0`,
`S` the same as `5`, and `W` is `M` upside down. That is why the greeting reads
with a zero in it.

`programs/abc123.c9` is the same shape of program without the parameters, and
it is worth reading next to `programs/abc123.asm`: compile one and assemble the
other and you get identical files.

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

A `var` written outside any function is a global, visible everywhere and alive
for the whole run. A `var` written inside a function or inside a `{ ... }`
block is a local: it holds a register only while that block is running, and the
register goes back into the pool at the closing brace.

```c
var count @ V2;             // a global

fn tally() {
    var seen;               // a local, freed when tally() ends

    {
        var scratch;        // freed at this closing brace
        seen = scratch;
    }
}
```

A local shadows a global of the same name, as in C. Two declarations of the
same name in the *same* block are an error.

Locals are cheap but not free: a function's locals and parameters are pushed
onto the data stack around every call it makes, so a function with six locals
pays six `PUSH`es and six `POP`s per call. Globals are never saved, which is
what keeps `leap.c9` down to a bare `CALL`.

### Functions

```c
fn main() {
    draw_scene();
    n = twice(21);
}

fn draw_scene() {
    clear();
}

fn twice(v) {
    return v + v;
}
```

**The first function in the file is where the program starts**, wherever you
put it in the file, and it may not take parameters. The rest are subroutines,
reached with `CALL` and returning with `RET`.

Parameters are registers, named. They behave exactly like locals: they are live
for the body of the function and handed back at the end. Functions of the same
arity share the same registers, which is safe because the caller saves its own
before it calls.

`return expr;` puts the value in `V0` and returns; a bare `return;` just
returns. A function ends with an implicit `return`, so most functions never
write one.

Here is the whole calling convention, from `show` in `programs/times.c9`
calling itself:

```
    PUSH V5             ; show's own parameter, saved by show
    LD   V0, V5         ; work out the argument
    LD   V1, 10
    DIV  V0, V1
    LD   V5, V0         ; hand it over in the parameter register
    CALL show
    POP  V5             ; and take the parameter back
```

The caller saves every register it owns — its parameters and its locals, but
not the globals — then evaluates the arguments, then calls. With more than one
argument the earlier ones go via the data stack, because working out the second
would otherwise trample the first:

```
    LD   V0, 0
    PUSH V0             ; first argument, parked
    LD   V0, 5
    LD   V5, V0         ; last argument, handed straight over
    POP  V4             ; and now the first one into its register
    CALL write
```

Because the arguments travel on the stack rather than in registers, recursion
works, and so does a call inside an argument: `twice(twice(3))` is fine.

### Expressions

```c
x = 7;              // LD V2, 7
y = x;              // LD V3, V2
x += 1;             // ADD V2, 1
x -= 1;             // LD V0, 1  /  SUB V2, V0
flags ^= 1;
x = 8 - y;          // SUBN, because the constant is on the left
y = x >> 1;         // LD V3, V2  /  SHR V3
n = a * b;          // MUL, which CHIP-8 did not have
q = n / 10;         // DIV
r = n % 10;         // DIV, then read the remainder out of VF
```

`+`, `-`, `*`, `/`, `%`, `&`, `|`, `^`, `>>` and `<<` are all there, and so are
the `+=` forms of each. A few things follow from the hardware rather than from
taste:

- **Shifts are by one.** `x >>= 2` is an error, because the machine has one
  shift instruction and it moves one bit.
- **Subtraction has no immediate form.** `x -= 1` has to load the 1 into a
  register first, so it is three bytes where `x += 1` is two.
- **`-` sets the flag.** After `a - b`, `VF` is 1 when `a >= b`. This is how
  comparisons work, below.
- **`/` and `%` are the same instruction.** `DIV` leaves the quotient in the
  destination and the remainder in `VF`, so `%` is a `DIV` and then a read of
  `VF`. Dividing by zero halts the machine.
- **Everything is a byte.** `20 * 20` is 144, not 400.

#### One side has to be simple

The compiler has one accumulator and one scratch register, so it works an
expression out left to right and combines each step with something that fits in
`V1`. The **right** operand of every operator therefore has to be a constant, a
variable or an array element; the **left** operand can be as complicated as you
like.

```c
cx = ((n - 1) / ROWS) * COLUMN_W + 1;   // fine: the depth is all on the left
k = 1 + fact(n);                        // not allowed
k = fact(n) + 1;                        // the same thing, written round
```

If you genuinely need both sides to be complicated, put one in a variable
first.

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

There is `&&` and `||`, and both short circuit. `a && b` only tests `b` when
`a` held; `a || b` only tests `b` when `a` did not. Neither produces a value —
they exist to be tested, and belong in an `if`, a `while` or a `do`/`while`.

```c
if (a == 1 && b == 2) { c = 1; c = 2; }
```

```
    SE   V2, 1
    JP   _L0            ; a failed, so skip the whole thing
    SE   V3, 2
    JP   _L0            ; b failed, same place
    LD   V4, 1
    LD   V4, 2
_L0:
```

`||` needs one more label, because the left hand side succeeding has to jump
*into* the body:

```c
if (a == 1 || b == 2) { c = 3; c = 4; }
```

```
    SNE  V2, 1
    JP   _L2            ; a held, so go straight in
    SE   V3, 2
    JP   _L1            ; neither held
_L2:
    LD   V4, 3
    LD   V4, 4
_L1:
```

They nest and mix as you would expect, `&&` binding tighter than `||`, and
`!` in front of either flips it.

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

`goto` is not a last resort here. A CHIP-9 program is a web of jumps, and being
able to write one directly is what lets `leap.c9` come out as exactly the same
instructions a person would have written by hand.

### Memory

```c
byte  message[] = { 'H', 'E', 'L', 'L', 'O' };
byte  digits[]  = { $A, $B, $C, 1, 2, 3 };
sprite player   = { $70, $70, $F8, $70, $50, $50 };
```

`byte` and `sprite` mean the same thing to the compiler; the two spellings just
say what you meant. Reading one back:

```c
at = 0;
d = message[at];        // LD I, message  /  ADD I, V4  /  LD V0, [I]
d = message[1];         // LD I, message + 1  /  LD V0, [I]
```

An index can be any expression, so `message[from + at]` is fine.

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

### Character literals

```c
letter = 'H';           // 17
digit  = '7';           // 7
```

A character in quotes is its **base thirty six** value, which is exactly its
index in the font: `'0'`–`'9'` are 0–9 and `'A'`–`'Z'` are 10–35. So
`draw(x, y, font('H'))` draws an H. Lowercase is accepted and means the same
thing.

### The data stack

```c
push(x);
y = pop();
```

CHIP-9 has sixty-four bytes of data stack, separate from the call stack. The
compiler uses it to save registers around calls, and you can use it yourself
for anything a register will not hold. Overflowing it or popping an empty one
halts the machine.

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
| `fn name(a, b) { ... }` | a subroutine; the first one is the entry point |
| `label:` | a jump target |
| `goto label;` | jump |
| `return;` / `return e;` | `RET`, with the value in `V0` |
| `if (c) s` / `else s` | |
| `while (c) s` | test at the top |
| `do s while (c);` | test at the bottom |
| `loop s` | forever |
| `break;` / `continue;` | leave, or restart, the nearest loop |
| `{ ... }` | a block, and a scope for the `var`s in it |
| `// ...` and `/* ... */` | comments |

Numbers can be written `31`, `0x1F`, `$1F`, `#1F` or `0b11111`, and `'H'` is a
character in base thirty six.

Operators, tightest first: `!` and unary `-`; then `*` `/` `%`; `+` `-`;
`<<` `>>`; `<` `<=` `>` `>=`; `==` `!=`; `&`; `^`; `|`; `&&`; and `||` loosest
of all. Assignment is `=` or any of `+ - * / % & | ^ << >>` followed by `=`.

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
| `push(v)` / `pop()` | `PUSH` / `POP`, the data stack |

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
does — only how many bytes it takes to say it. It also tracks *which registers*
a remembered value was worked out from, and forgets it the moment one of them
is written, so `x = 9` between two `x + 1`s is never elided over. A `CALL` makes
it forget everything.

**It will not reuse a stale flag.** Dropping a subtraction would drop the `VF`
it set, so the compiler only does it when `VF` is written again before anything
reads it. Two identical comparisons in a row therefore still subtract twice.

That is the whole of it. There is no inlining and no instruction scheduling.
What you write is what you get, minus the loads you did not need.

## Things that catch people out

- **`V0` and `V1` are not yours.** Anything you leave in them is gone by the
  next statement that computes something. Pin a variable to them only when you
  know nothing else is happening, as `draw_scene` in `leap.c9` does.
- **`VF` is not yours either.** `ADD`, `SUB`, `MUL`, `DIV`, `SHR`, `SHL` and
  `DRW` all write it.
- **`x -= 1` is bigger than `x += 1`.** No subtract immediate.
- **`if (a > b)` costs the same as `if (a < b)`** — the compiler swaps the sides
  — but both cost five instructions against two for `!=`.
- **The first function is the entry point**, wherever you put it in the file,
  and it cannot take parameters.
- **The right hand side of an operator has to be simple.** Put the deep half of
  an expression on the left.
- **Calls are not free.** Every register the caller owns is pushed and popped
  around the call. Globals are not, so they are the cheap way to pass a value
  that everything needs.
- **Registers written out as `V2` only work where a register is meant**, which
  in practice is `var ... @ V2` and `store`/`restore`.
- **Thirteen registers is not many.** Deeply nested blocks full of locals will
  run out, and the compiler will say so.

## Where to go next

- `programs/hello.c9` — the alphabet font, a function with two parameters and
  two locals.
- `programs/times.c9` — `*`, `/`, `%` and a function that calls itself.
- `programs/abc123.c9` and `programs/abc123.asm` — the same six characters,
  both ways.
- `programs/leap.c9` and `programs/leap.asm` — a whole game, both ways. Compile
  one and assemble the other and you get identical files.
- `TUTORIAL.md` — the assembler, and the machine underneath all of this.
- `src/lang/` — the compiler. `lexer.rs`, `parser.rs` and `codegen.rs`, in that
  order; the interesting part is `analyse` at the bottom of `codegen.rs`.
