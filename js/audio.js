export default class Audio {
    constructor() {
        this.audioContext = new AudioContext();

        this.oscillator = this.audioContext.createOscillator();
        this.oscillator.type = 'square';
        this.oscillator.frequency.value = 400;
        this.oscillator.start();
    }

    start() {
        this.oscillator.connect(this.audioContext.destination);
    }

    stop() {
        this.oscillator.disconnect();
    }
}
