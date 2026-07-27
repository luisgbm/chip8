; =============================================================================
;  LEAP - a platform game for the CHIP-8
; =============================================================================
;
;  A floor runs across the screen with a pit in the middle.  Walk up to the
;  pit, jump it, and try not to drop in.  Fall and the screen reads GAME OVER
;  for three seconds before the game starts again.
;
;      keypad 4 (host Q) ... walk left
;      keypad 6 (host E) ... walk right
;      keypad 5 (host W) ... jump
;
;  Assemble with:  cargo run --bin asm -- programs/leap.asm roms/leap.ch8
;
;  Registers
;      V0, V1  scratch
;      V2      player x
;      V3      player y, in pixels, which is what gets drawn
;      V4      player y, in half pixels, which is what the physics uses
;      V5      vertical velocity, biased so that 8 means standing still
;      V6      0 in the air, 1 standing on the floor, 2 down the pit
;      V7, V8  where the player sprite currently is on screen
;      VA      result of over_hole
;      VB      gravity is applied on every other tick, this says which
;      VC      key being tested
;      VD, VE  scratch for drawing text
;
;  Vertical position is kept in half pixels so that a jump can arc smoothly
;  over sixteen ticks instead of snapping between whole pixels.
; =============================================================================

; ---- pacing -----------------------------------------------------------------
TICK        = 2         ; frames per game tick, so the game runs at 30 Hz
OVER_TIME   = 180       ; frames the game over screen stays up, three seconds

; ---- the world --------------------------------------------------------------
FLOOR_Y     = 26        ; top row of the floor
HOLE_X      = 28        ; leftmost column of the pit
HOLE_W      = 8         ; and how wide it is, so 28..35, the middle of 0..63

; ---- states held in V6 ------------------------------------------------------
IN_AIR      = 0
ON_FLOOR    = 1
IN_PIT      = 2         ; below floor level with walls either side, no way out

; ---- the player -------------------------------------------------------------
PLAYER_H    = 6
PLAYER_W    = 5         ; the sprite is 8 wide but only 5 columns are used
FOOT_X      = 2         ; the column the player is balanced on
START_X     = 4
MAX_X       = 64 - PLAYER_W
START_Y     = 20        ; FLOOR_Y - PLAYER_H
FLOOR_SUB   = 40        ; START_Y * 2
DEAD_SUB    = 64        ; once y reaches 32 the player is off the screen
HOLE_MIN    = HOLE_X - FOOT_X

; The player has to be somewhere in here to fit down the pit without the
; sprite overlapping, and so rubbing out, the lip on either side.
PIT_LEFT    = HOLE_X
PIT_RIGHT   = HOLE_X + HOLE_W - PLAYER_W

; ---- velocities, all biased by 8 so they can be negative --------------------
ZERO_VY     = 8         ; not moving
JUMP_VY     = 4         ; four half pixels per tick upwards
FALL_VY     = 9         ; the nudge given when walking off the edge
MAX_VY      = 12        ; terminal velocity, two pixels per tick

; ---- keys -------------------------------------------------------------------
KEY_LEFT    = 4
KEY_RIGHT   = 6
KEY_JUMP    = 5

TEXT_Y      = 13


; =============================================================================
;  Start of a life
; =============================================================================
new_game:
    CALL draw_scene
    LD   V2, START_X
    LD   V3, START_Y
    LD   V4, FLOOR_SUB
    LD   V5, ZERO_VY
    LD   V6, ON_FLOOR
    LD   VB, 0
    LD   V7, V2
    LD   V8, V3
    LD   I, player
    DRW  V7, V8, PLAYER_H


; =============================================================================
;  One tick of the game
; =============================================================================
tick:
    LD   V0, DT                 ; the delay timer paces the whole game
    SE   V0, 0
    JP   tick
    LD   V0, TICK
    LD   DT, V0

; ---- input ------------------------------------------------------------------
    LD   VC, KEY_LEFT
    SKNP VC
    CALL move_left
    LD   VC, KEY_RIGHT
    SKNP VC
    CALL move_right
    LD   VC, KEY_JUMP
    SKNP VC
    CALL try_jump

; ---- physics ----------------------------------------------------------------
    SE   V6, ON_FLOOR
    JP   falling

    CALL over_hole              ; standing: is there still floor underfoot?
    SE   VA, 1
    JP   draw_step
    LD   V6, IN_AIR             ; no, so step off the edge
    LD   V5, FALL_VY
    LD   VB, 0
    JP   draw_step

falling:
    LD   V0, 1                  ; gravity pulls on every other tick, which
    XOR  VB, V0                 ; stretches the jump out without making it
    SE   VB, 0                  ; any higher
    JP   no_gravity
    LD   V0, MAX_VY
    SE   V5, V0
    ADD  V5, 1
no_gravity:

    LD   V0, V5                 ; VF says which way the player is moving
    LD   V1, ZERO_VY
    SUB  V0, V1
    SE   VF, 0
    JP   going_down

    LD   V0, V5                 ; going up by ZERO_VY - V5 half pixels
    LD   V1, ZERO_VY
    SUBN V0, V1
    SUB  V4, V0
    JP   after_move

going_down:
    ADD  V4, V0
    SE   V6, IN_PIT             ; once down the pit there is nothing to land on
    JP   check_floor
    JP   check_dead

check_floor:
    LD   V0, V4                 ; has the player reached floor level?
    LD   V1, FLOOR_SUB
    SUB  V0, V1
    SE   VF, 1
    JP   after_move
    CALL over_hole
    SE   VA, 0
    JP   enter_pit

    LD   V4, FLOOR_SUB          ; landed, so stand back up on the floor
    LD   V5, ZERO_VY
    LD   V6, ON_FLOOR
    LD   VB, 0
    JP   after_move

enter_pit:                      ; past the lip of the pit, so that is that
    LD   V6, IN_PIT
    LD   V0, V2                 ; slide the player fully inside the pit, so
    LD   V1, PIT_LEFT           ; that on the way down the sprite never
    SUB  V0, V1                 ; overlaps, and so rubs out, the floor
    SE   VF, 0
    JP   pit_right
    LD   V2, PIT_LEFT
    JP   check_dead
pit_right:
    LD   V0, V2
    LD   V1, PIT_RIGHT
    SUB  V0, V1
    SE   VF, 1
    JP   check_dead
    LD   V2, PIT_RIGHT

check_dead:
    LD   V0, V4
    LD   V1, DEAD_SUB
    SUB  V0, V1
    SE   VF, 1
    JP   after_move
    JP   game_over

after_move:
    LD   V3, V4                 ; half pixels back to pixels
    SHR  V3

; ---- draw, but only when the player actually moved --------------------------
draw_step:
    SNE  V2, V7
    JP   same_x
    JP   move_sprite
same_x:
    SE   V3, V8
    JP   move_sprite
    JP   tick

move_sprite:
    LD   I, player
    DRW  V7, V8, PLAYER_H       ; rub the player out
    LD   V7, V2
    LD   V8, V3
    DRW  V7, V8, PLAYER_H       ; and draw it again where it is now
    JP   tick


; =============================================================================
;  Game over
; =============================================================================
game_over:
    CLS
    LD   V0, 12
    LD   ST, V0
    CALL draw_text
    LD   V0, OVER_TIME
    LD   DT, V0
over_wait:
    LD   V0, DT
    SE   V0, 0
    JP   over_wait
    JP   new_game


; =============================================================================
;  Subroutines
; =============================================================================

; Walk one pixel left, unless already against the edge or down the pit.
move_left:
    SNE  V6, IN_PIT
    RET
    SNE  V2, 0
    RET
    LD   V0, 1
    SUB  V2, V0
    RET

; Walk one pixel right, unless already against the edge or down the pit.
move_right:
    SNE  V6, IN_PIT
    RET
    SNE  V2, MAX_X
    RET
    ADD  V2, 1
    RET

; Jump, but only from solid ground.
try_jump:
    SE   V6, ON_FLOOR
    RET
    LD   V5, JUMP_VY
    LD   V6, IN_AIR
    LD   VB, 0
    LD   V0, 2
    LD   ST, V0
    RET

; VA = 1 when the player's middle column is over the pit, that is when
; V2 + FOOT_X is somewhere in HOLE_X .. HOLE_X + HOLE_W - 1.
over_hole:
    LD   VA, 0
    LD   V0, V2
    LD   V1, HOLE_MIN
    SUB  V0, V1                 ; VF = 1 once the player is past the near edge
    SE   VF, 1
    RET
    LD   V1, HOLE_W
    SUB  V0, V1                 ; VF = 1 once the player is past the far edge
    SE   VF, 0
    RET
    LD   VA, 1
    RET

; Clear the screen and lay down the floor, then punch the pit back out of it.
draw_scene:
    CLS
    LD   I, floor
    LD   V1, FLOOR_Y
    LD   V0, 0
floor_loop:
    DRW  V0, V1, 2
    ADD  V0, 8
    SE   V0, 64
    JP   floor_loop
    LD   V0, HOLE_X
    DRW  V0, V1, 2
    RET

; GAME OVER, in letters the built in font does not have.
draw_text:
    LD   VE, TEXT_Y
    LD   VD, 7
    LD   I, char_g
    DRW  VD, VE, 5
    LD   VD, 13
    LD   I, char_a
    DRW  VD, VE, 5
    LD   VD, 19
    LD   I, char_m
    DRW  VD, VE, 5
    LD   VD, 25
    LD   I, char_e
    DRW  VD, VE, 5
    LD   VD, 34
    LD   I, char_o
    DRW  VD, VE, 5
    LD   VD, 40
    LD   I, char_v
    DRW  VD, VE, 5
    LD   VD, 46
    LD   I, char_e
    DRW  VD, VE, 5
    LD   VD, 52
    LD   I, char_r
    DRW  VD, VE, 5
    RET


; =============================================================================
;  Sprites
; =============================================================================

player:                         ; .###.
    DB   $70                    ; .###.
    DB   $70                    ; #####
    DB   $F8                    ; .###.
    DB   $70                    ; .#.#.
    DB   $50                    ; .#.#.
    DB   $50

floor:
    DB   $FF, $FF

char_g:
    DB   $70, $80, $B8, $88, $70
char_a:
    DB   $70, $88, $F8, $88, $88
char_m:
    DB   $88, $D8, $A8, $88, $88
char_e:
    DB   $F8, $80, $F0, $80, $F8
char_o:
    DB   $70, $88, $88, $88, $70
char_v:
    DB   $88, $88, $88, $50, $20
char_r:
    DB   $F0, $88, $F0, $A0, $90
