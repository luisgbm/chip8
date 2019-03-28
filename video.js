export default class Video {
    canvasWidth = 640;
    width = null;
    canvasHeight = 320;
    height = null;
    pixelSize = 10;
    canvasId = 'canvas';
    domCanvas = null;
    canvasCtx = null;

    constructor(canvasId) {
        this.width = this.canvasWidth / this.pixelSize;
        this.height = this.canvasHeight / this.pixelSize;

        this.canvasId = canvasId;

        this.domCanvas = document.getElementById(this.canvasId);
        this.domCanvas.setAttribute('width', this.canvasWidth);
        this.domCanvas.setAttribute('height', this.canvasHeight);

        this.canvasCtx = this.domCanvas.getContext('2d');
    }

    drawFramebufferInCanvas(framebuffer) {
        let canvas = this.framebufferToCanvas(framebuffer);

        for (let i = 0; i < this.canvasHeight / this.pixelSize; i++) {
            for (let j = 0; j < this.canvasWidth / this.pixelSize; j++) {
                const canvasBit = canvas[i][j];

                this.canvasCtx.beginPath();
                this.canvasCtx.rect(j * this.pixelSize, i * this.pixelSize, this.pixelSize, this.pixelSize);

                if (canvasBit === 1) {
                    this.canvasCtx.fillStyle = 'white';
                } else {
                    this.canvasCtx.fillStyle = 'black';
                }

                this.canvasCtx.fill();
            }
        }
    }

    framebufferToCanvas(framebuffer) {
        let canvas = [];

        for (let i = 0; i < framebuffer.length; i += this.width) {
            canvas.push(framebuffer.slice(i, i + this.width));
        }

        return canvas;
    }
}