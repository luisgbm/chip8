; =============================================================================
;  ABC123 - the worked example from TUTORIAL.md
; =============================================================================
;
;  Writes ABC123 across the middle of the screen using the interpreter's own
;  font, then stops.  Every glyph in that font is five rows tall and four
;  columns wide, and there is one for each of the sixteen hex digits, which is
;  exactly the six characters this needs.
;
;  Assemble with:  cargo run --bin asm -- programs/abc123.asm roms/abc123.ch8
; =============================================================================

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


; The message, one hex digit per byte.  A, B and C really are the digits
; $A, $B and $C, which is why the font has glyphs for them.
message:
    DB   $A, $B, $C, 1, 2, 3
