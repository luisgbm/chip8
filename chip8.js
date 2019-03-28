// Welcome! This is a JS CHIP-8 interpreter
// by Luís Mendes - 12/Dec/2018

import Audio from './audio';

export default class Chip8 {
    constructor (video, keyboard) {
        this.configureTimers();
        this.audio = new Audio();

        this.video = video;
        this.keyboard = keyboard;

        this.reset();
    }

    reset () {
        this.memory = new Array(4096);
        for (let i = 0; i < this.memory.length; i++) {
            this.memory[i] = 0;
        }

        this.v = new Array(16);
        for (let i = 0; i < this.v.length; i++) {
            this.v[i] = 0;
        }

        this.i = 0x0;
        this.pc = 0x200;
        this.sp = 0x0;

        this.stack = new Array(16);
        for (let i = 0; i < this.stack.length; i++) {
            this.stack[i] = 0;
        }

        this.delayTimer = 0x0;
        this.soundTimer = 0x0;
        this.framebuffer = this.createFramebuffer();

        this.font = this.createFont();

        this.mustDraw = false;

        this.start();
    }

    start () {
        this.running = true;
    }

    stop () {
        this.running = false;
    }

    loadProgram (program) {
        for (let i = 0; i < program.length; i++) {
            this.memory[0x200 + i] = program [i];
        }
    }

    createFramebuffer () {
        let framebuffer = new Array(this.video.width * this.video.height);
        for (let i = 0; i < framebuffer.length; i++) {
            framebuffer[i] = 0;
        }

        return framebuffer;
    }

    configureTimers () {
        // If the timers are non-zero, they must decrement at 60Hz (once every 16,667ms)
        // For the purposes of rounding and general convention, I'll use a 15ms interval
        setInterval(() => {
            if (this.delayTimer > 0) {
                this.delayTimer--;
            }

            if (this.soundTimer > 0) {
                this.audio.start();
                this.soundTimer--;
            } else {
                this.audio.stop();
            }
        }, 15);
    }

    createFont () {
        let allChars = [];

        allChars = allChars.concat([0xF0, 0x90, 0x90, 0x90, 0xF0]);
        allChars = allChars.concat([0x20, 0x60, 0x20, 0x20, 0x70]);
        allChars = allChars.concat([0xF0, 0x10, 0xF0, 0x80, 0xF0]);
        allChars = allChars.concat([0xF0, 0x10, 0xF0, 0x10, 0xF0]);
        allChars = allChars.concat([0x90, 0x90, 0xF0, 0x10, 0x10]);
        allChars = allChars.concat([0xF0, 0x80, 0xF0, 0x10, 0xF0]);
        allChars = allChars.concat([0xF0, 0x80, 0xF0, 0x90, 0xF0]);
        allChars = allChars.concat([0xF0, 0x10, 0x20, 0x40, 0x40]);
        allChars = allChars.concat([0xF0, 0x90, 0xF0, 0x90, 0xF0]);
        allChars = allChars.concat([0xF0, 0x90, 0xF0, 0x10, 0xF0]);
        allChars = allChars.concat([0xF0, 0x90, 0xF0, 0x90, 0x90]);
        allChars = allChars.concat([0xE0, 0x90, 0xE0, 0x90, 0xE0]);
        allChars = allChars.concat([0xF0, 0x80, 0x80, 0x80, 0xF0]);
        allChars = allChars.concat([0xE0, 0x90, 0x90, 0x90, 0xE0]);
        allChars = allChars.concat([0xF0, 0x80, 0xF0, 0x80, 0xF0]);
        allChars = allChars.concat([0xF0, 0x80, 0xF0, 0x80, 0x80]);

        // The font is stored in the interpreter area of memory (0x000 to 0x1FF)
        for (let i = 0; i < allChars.length; i++) {
            this.memory[i] = allChars[i];
        }

        return allChars;
    }

    emulateCycle () {
        let opcode = this.memory[this.pc] << 8 | this.memory[this.pc + 1];
        
        if (opcode === 0) return;

        this.pc += 2;

        this.runInstruction(opcode);
    }

    runInstruction (opcode) {
        // All instructions are 2 bytes long

        // nnn or addr - A 12-bit value, the lowest 12 bits of the instruction
        let nnn = opcode & 0xFFF;

        // n or nibble - A 4-bit value, the lowest 4 bits of the instruction
        let n = opcode & 0xF;

        // x - A 4-bit value, the lower 4 bits of the high byte of the instruction
        let x = (opcode & 0xF00) >> 8;

        // y - A 4-bit value, the upper 4 bits of the low byte of the instruction
        let y = (opcode & 0xF0) >> 4;

        // kk or byte - An 8-bit value, the lowest 8 bits of the instruction
        let kk = opcode & 0xFF;

        // u - A 4-bit value, the upper 4 bits of the upper byte of the instruction
        let u = (opcode & 0xF000) >> 12;

        // 00E0 - CLS - Clear the display.
        if (u === 0x00 && kk === 0xE0) {
            this.framebuffer = this.createFramebuffer();
        }
        // 00EE - RET - Return from a subroutine.
        else if (u === 0x00 && kk === 0xEE) {
            // The interpreter sets the program counter to the address at the top of the stack,
            // then subtracts 1 from the stack pointer.
            this.pc = this.stack[--this.sp];
        }
        // 1nnn - JP addr - Jump to location nnn.
        else if (u === 0x1) {
            // The interpreter sets the program counter to nnn.
            this.pc = nnn;
        }
        // 2nnn - CALL addr - Call subroutine at nnn.
        else if (u === 0x2) {
            // The interpreter increments the stack pointer, then puts the current PC on the
            // top of the stack. The PC is then set to nnn.
            this.stack[this.sp++] = this.pc;
            this.pc = nnn;
        }
        // 3xkk - SE Vx, byte - Skip next instruction if Vx = kk.
        else if (u === 0x3) {
            // The interpreter compares register Vx to kk, and if they are equal, increments
            // the program counter by 2.
            if (this.v[x] === kk) {
                
                this.pc += 2;
            }
        }
        // 4xkk - SNE Vx, byte - Skip next instruction if Vx != kk.
        else if (u === 0x4) {
            // The interpreter compares register Vx to kk, and if they are not equal, increments
            // the program counter by 2.
            if (this.v[x] !== kk) {
                
                this.pc += 2;
            }
        }
        // 5xy0 - SE Vx, Vy - Skip next instruction if Vx = Vy.
        else if (u === 0x5) {
            // The interpreter compares register Vx to register Vy, and if they are equal, increments
            // the program counter by 2.
            if (this.v[x] === this.v[y]) {
                
                this.pc += 2;
            }
        }
        // 6xkk - LD Vx, byte - Set Vx = kk.
        else if (u === 0x6) {
            // The interpreter puts the value kk into register Vx.
            this.v[x] = kk;
        }
        // 7xkk - ADD Vx, byte - Set Vx = Vx + kk.
        else if (u === 0x7) {
            // Adds the value kk to the value of register Vx, then stores the result in Vx.
            let val = this.v[x] + kk;

            if (val > 255) {
                val -= 256;
            }

            this.v[x] = val;
        }
        // 8xy0 - LD Vx, Vy - Set Vx = Vy.
        else if (u === 0x8 && n === 0x0) {
            // Stores the value of register Vy in register Vx.
            this.v[x] = this.v[y];
        }
        // 8xy1 - OR Vx, Vy - Set Vx = Vx OR Vy.
        else if (u === 0x8 && n === 0x1) {
            // Performs a bitwise OR on the values of Vx and Vy, then stores the result in Vx.
            this.v[x] |= this.v[y];
        }
        // 8xy2 - AND Vx, Vy - Set Vx = Vx AND Vy.
        else if (u === 0x8 && n === 0x2) {
            // Performs a bitwise AND on the values of Vx and Vy, then stores the result in Vx.
            this.v[x] &= this.v[y];
        }
        // 8xy3 - XOR Vx, Vy - Set Vx = Vx XOR Vy.
        else if (u === 0x8 && n === 0x3) {
            // Performs a bitwise exclusive OR on the values of Vx and Vy, then stores the result in Vx.
            this.v[x] ^= this.v[y];
        }
        // 8xy4 - ADD Vx, Vy - Set Vx = Vx + Vy, set VF = carry.
        else if (u === 0x8 && n === 0x4) {
            // The values of Vx and Vy are added together. If the result is greater than 8 bits
            // (i.e., > 255,) VF is set to 1, otherwise 0. Only the lowest 8 bits of the result
            // are kept, and stored in Vx.
            let temp = this.v[x] + this.v[y];

            if (temp > 0xFF) {
                this.v[0xF] = 1;
                temp -= 256;
            } else {
                this.v[0xF] = 0;
            }

            this.v[x] = temp;
        }
        // 8xy5 - SUB Vx, Vy - Set Vx = Vx - Vy, set VF = NOT borrow.
        else if (u === 0x8 && n === 0x5) {
            // If Vx >= Vy, then VF is set to 1, otherwise 0. Then Vy is subtracted from Vx, and
            // the results stored in Vx.
            if (this.v[x] >= this.v[y]) {
                this.v[0xF] = 1;
            } else {
                this.v[0xF] = 0;
            }

            this.v[x] = this.v[x] - this.v[y];
        }
        // 8xy6 - SHR Vx {, Vy} - Set Vx = Vx SHR 1.
        else if (u === 0x8 && n === 0x6) {
            // If the least-significant bit of Vx is 1, then VF is set to 1, otherwise 0.
            // Then Vx is divided by 2.
            this.v[0xF] = this.v[x] & 0x1;
            this.v[x] = this.v[x] >> 1;
        }
        // 8xy7 - SUBN Vx, Vy - Set Vx = Vy - Vx, set VF = NOT borrow.
        else if (u === 0x8 && n === 0x7) {
            // If Vy >= Vx, then VF is set to 1, otherwise 0. Then Vx is subtracted from Vy,
            // and the results stored in Vx.
            if (this.v[y] >= this.v[x]) {
                this.v[0xF] = 1;
            } else {
                this.v[0xF] = 0;
            }

            this.v[x] = this.v[y] - this.v[x];
        }
        // 8xyE - SHL Vx {, Vy} - Set Vx = Vx SHL 1.
        else if (u === 0x8 && n === 0xE) {
            // If the most-significant bit of Vx is 1, then VF is set to 1, otherwise to 0.
            // Then Vx is multiplied by 2.
            this.v[0xF] = this.v[x] >> 7;
            this.v[x] = this.v[x] << 1;
        }
        // 9xy0 - SNE Vx, Vy - Skip next instruction if Vx != Vy.
        else if (u === 0x9 && n === 0x0) {
            // The values of Vx and Vy are compared, and if they are not equal, the program
            // counter is increased by 2.
            if (this.v[x] !== this.v[y]) {
                this.pc += 2;
            }
        }
        // Annn - LD I, addr - Set I = nnn.
        else if (u === 0xA) {
            // The value of register I is set to nnn.
            this.i = nnn;
        }
        // Bnnn - JP V0, addr - Jump to location nnn + V0.
        else if (u === 0xB) {
            // The program counter is set to nnn plus the value of V0.
            this.pc = nnn + this.v[0];
        }
        // Cxkk - RND Vx, byte - Set Vx = random byte AND kk.
        else if (u === 0xC) {
            // The interpreter generates a random number from 0 to 255, which is then ANDed with the
            // value kk. The results are stored in Vx.
            let rnd = Math.floor(Math.random() * 0xFF);
            this.v[x] = rnd & kk;
        }
        // Dxyn - DRW Vx, Vy, nibble - Display n-byte sprite starting at memory location I at (Vx, Vy),
        // set VF = collision.
        else if (u === 0xD) {
            // The interpreter reads n bytes from memory, starting at the address stored in I. These
            // bytes are then displayed as sprites on screen at coordinates (Vx, Vy). Sprites are XORed
            // onto the existing screen. If this causes any pixels to be erased, VF is set to 1, otherwise
            // it is set to 0. If the sprite is positioned so part of it is outside the coordinates of the
            // display, it wraps around to the opposite side of the screen.
            this.mustDraw = true;

            let sprite = [];
            for (let i = 0; i < n; i++) {
                sprite.push(this.memory[this.i + i].toString(2).padStart(8, '0').split('').map(val => parseInt(val)));
            }

            for (let i = 0; i < sprite[0].length; i++) {
                for (let j = 0; j < sprite.length; j++) {
                    let drawX = this.v[x] + i;

                    if (drawX > this.video.width) {
                        drawX -= this.video.width;
                    } else if (drawX < 0) {
                        drawX += this.video.width;
                    }

                    let drawY = this.v[y] + j;

                    if (drawY > this.video.height) {
                        drawY -= this.video.height;
                    } else if (drawY < 0) {
                        drawY += this.video.height;
                    }

                    let location = drawX + (this.video.width * drawY);

                    let collision = sprite[j][i] ^ this.framebuffer[location];

                    if (!collision > 0) {
                        this.v[0xF] = 1;
                    } else {
                        this.v[0xF] = 0;
                    }

                    this.framebuffer[location] = collision;
                }
            }
        }
        // Ex9E - SKP Vx - Skip next instruction if key with the value of Vx is pressed.
        else if (u === 0xE && kk === 0x9E) {
            // Checks the keyboard, and if the key corresponding to the value of Vx is currently in
            // the down position, PC is increased by 2.
            if (this.keyboard.pressedKeys[this.v[x]]) {
                this.pc += 2;
            }
            
        }
        // ExA1 - SKNP Vx - Skip next instruction if key with the value of Vx is not pressed.
        else if (u === 0xE && kk === 0xA1) {
            // Checks the keyboard, and if the key corresponding to the value of Vx is currently in
            // the up position, PC is increased by 2.
            if (!this.keyboard.pressedKeys[this.v[x]]) {
                this.pc += 2;
            }
        }
        // Fx07 - LD Vx, DT - Set Vx = delay timer value.
        else if (u === 0xF && kk === 0x07) {
            // The value of DT is placed into Vx.
            this.v[x] = this.delayTimer;
        }
        // Fx0A - LD Vx, K - Wait for a key press, store the value of the key in Vx.
        else if (u === 0xF && kk === 0x0A) {
            // All execution stops until a key is pressed, then the value of that key is stored in Vx.
            this.stop();
            this.keyboard.registerWaitCallback((keyCode) => {
                this.v[x] = keyCode;
                this.start();
            });
        }
        // Fx15 - LD DT, Vx - Set delay timer = Vx.
        else if (u === 0xF && kk === 0x15) {
            // DT is set equal to the value of Vx.
            this.delayTimer = this.v[x];
        }
        // Fx18 - LD ST, Vx - Set sound timer = Vx.
        else if (u === 0xF && kk === 0x18) {
            // ST is set equal to the value of Vx.
            this.soundTimer = this.v[x];
        }
        // Fx1E - ADD I, Vx - Set I = I + Vx.
        else if (u === 0xF && kk === 0x1E) {
            // The values of I and Vx are added, and the results are stored in I.
            this.i += this.v[x];
        }
        // Fx29 - LD F, Vx - Set I = location of sprite for digit Vx.
        else if (u === 0xF && kk === 0x29) {
            // The value of I is set to the location for the hexadecimal sprite corresponding
            // to the value of Vx.
            this.i = this.v[x] * 5;
        }
        // Fx33 - LD B, Vx - Store BCD representation of Vx in memory locations I, I+1, and I+2.
        else if (u === 0xF && kk === 0x33) {
            // The interpreter takes the decimal value of Vx, and places the hundreds digit in memory at
            // location in I, the tens digit at location I+1, and the ones digit at location I+2.
            let hundreds = parseInt((this.v[x] / 100) % 10);
            let tens = parseInt((this.v[x] / 10) % 10);
            let ones = parseInt(this.v[x] % 10);

            this.memory[this.i] = hundreds;
            this.memory[this.i + 1] = tens;
            this.memory[this.i + 2] = ones;
        }
        // Fx55 - LD [I], Vx - Store registers V0 through Vx in memory starting at location I.
        else if (u === 0xF && kk === 0x55) {
            // The interpreter copies the values of registers V0 through Vx into memory, starting at the
            // address in I.
            for (let i = 0; i <= x; i++) {
                this.memory[this.i + i] = this.v[i];
            }
        }
        // Fx65 - LD Vx, [I] - Read registers V0 through Vx from memory starting at location I.
        else if (u === 0xF && kk === 0x65) {
            // The interpreter reads values from memory starting at location I into registers V0 through Vx.
            for (let i = 0; i <= x; i++) {
                this.v[i] = this.memory[this.i + i];
            }
        }
    }
};
