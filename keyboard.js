export default class Keyboard {
    waitCallback = null;

    pressedKeys = {};

    keyMap = {
        49: 0x1,  // 1
        50: 0x2,  // 2
        51: 0x3,  // 3
        52: 0x4,  // 4
        81: 0x5,  // Q
        87: 0x6,  // W
        69: 0x7,  // E
        82: 0x8,  // R
        65: 0x9,  // A
        83: 0xA,  // S
        68: 0xB,  // D
        70: 0xC,  // F
        90: 0xD,  // Z
        88: 0xE,  // X
        67: 0xF,  // C
        86: 0x10  // V
    }

    convertKeys(keyCode) {
        return this.keyMap[keyCode];
    }

    isValidKey(keyCode) {
        return this.keyMap[keyCode] !== undefined;
    }

    keyDownCallback(event) {
        if (!this.isValidKey(event.keyCode)) {
            return;
        }

        let convertedKey = this.convertKeys(event.keyCode);

        this.pressedKeys[convertedKey] = true;

        if (this.waitCallback) {
            this.waitCallback(convertedKey);
            this.unregisterWaitCallback();
        }
    }

    keyUpCallback(event) {
        if (!this.isValidKey(event.keyCode)) {
            return;
        }

        let convertedKey = this.convertKeys(event.keyCode);

        delete this.pressedKeys[convertedKey];
    }

    registerWaitCallback(callback) {
        this.waitCallback = callback;
    }

    unregisterWaitCallback() {
        this.waitCallback = null;
    }
}
