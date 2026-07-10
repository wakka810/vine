import { validateFixedW2BrowserPresentEvent } from "./present-event.mjs";

export function createFixedW2CanvasSink(canvas) {
  if (canvas === null || typeof canvas !== "object" || typeof canvas.getContext !== "function") {
    throw new TypeError("fixed W2 Canvas sink requires a canvas element");
  }

  let context = null;
  let presentCount = 0;

  return Object.freeze({
    get presentCount() {
      return presentCount;
    },

    present(event) {
      if (presentCount !== 0) {
        throw new Error("fixed W2 Canvas sink accepts exactly one present event");
      }
      validateFixedW2BrowserPresentEvent(event);

      canvas.width = event.frame.width;
      canvas.height = event.frame.height;
      context = canvas.getContext("2d", { alpha: true, willReadFrequently: true });
      if (context === null) {
        throw new Error("Canvas 2D context is unavailable");
      }

      const image = context.createImageData(event.frame.width, event.frame.height);
      for (const pixel of event.pixels) {
        const offset = (pixel.y * event.frame.width + pixel.x) * 4;
        image.data.set(pixel.rgba, offset);
      }
      context.putImageData(image, 0, 0);
      presentCount += 1;

      return Object.freeze({
        presentCount,
        presentedPixels: event.pixels.length,
        width: canvas.width,
        height: canvas.height,
      });
    },

    readPixel(x, y) {
      if (presentCount !== 1 || context === null) {
        throw new Error("fixed W2 Canvas sink has not presented an event");
      }
      return Object.freeze(Array.from(context.getImageData(x, y, 1, 1).data));
    },
  });
}
