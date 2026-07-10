export const browserPresentEventSchema = "vine.browser-present";
export const browserPresentEventSchemaVersion = 1;
export const browserPresentEventLogPrefix = "vine-browser-present-event: ";

function requireCondition(condition, message) {
  if (!condition) {
    throw new TypeError(`invalid browser present event: ${message}`);
  }
}

function isInteger(value) {
  return Number.isInteger(value);
}

function colorRefToRgba(colorRef) {
  requireCondition(
    isInteger(colorRef) && colorRef >= 0 && colorRef <= 0x00ffffff,
    "COLORREF must be a 24-bit unsigned integer",
  );
  return [colorRef & 0xff, (colorRef >>> 8) & 0xff, (colorRef >>> 16) & 0xff, 0xff];
}

export function validateFixedW2BrowserPresentEvent(event) {
  requireCondition(event !== null && typeof event === "object", "event must be an object");
  requireCondition(event.schema === browserPresentEventSchema, "unsupported schema");
  requireCondition(
    event.schemaVersion === browserPresentEventSchemaVersion,
    "unsupported schema version",
  );
  requireCondition(event.kind === "pixel-present", "unsupported event kind");
  requireCondition(event.presentSequence === 1, "expected present sequence 1");
  requireCondition(
    event.source?.checkpoint === "fixed-w2-window-pixel",
    "unexpected source checkpoint",
  );
  requireCondition(event.source.paintSequence === 1, "expected paint sequence 1");
  requireCondition(event.frame?.width === 2, "expected frame width 2");
  requireCondition(event.frame?.height === 2, "expected frame height 2");
  requireCondition(event.frame?.pixelEncoding === "rgba8", "expected rgba8 pixels");
  requireCondition(Array.isArray(event.pixels), "pixels must be an array");
  requireCondition(event.pixels.length === 1, "expected exactly one sparse pixel");

  const [pixel] = event.pixels;
  requireCondition(pixel !== null && typeof pixel === "object", "pixel must be an object");
  requireCondition(pixel.x === 1 && pixel.y === 1, "expected pixel coordinate (1,1)");
  requireCondition(Array.isArray(pixel.rgba) && pixel.rgba.length === 4, "invalid RGBA tuple");
  requireCondition(
    pixel.rgba.every((component) => isInteger(component) && component >= 0 && component <= 0xff),
    "RGBA components must be bytes",
  );
  requireCondition(
    pixel.rgba.every((component, index) => component === [0xff, 0, 0, 0xff][index]),
    "expected COLORREF 0x000000FF converted to RGBA red",
  );
  return event;
}

export function createFixedW2BrowserPresentEvent({ bounds, paintSequence, pixels }) {
  requireCondition(
    bounds?.left === 0 && bounds?.top === 0 && bounds?.right === 2 && bounds?.bottom === 2,
    "expected harness-only bounds [0,0,2,2]",
  );
  requireCondition(paintSequence === 1, "expected source paint sequence 1");
  requireCondition(Array.isArray(pixels) && pixels.length === 1, "expected one logical pixel");

  const [pixel] = pixels;
  requireCondition(pixel?.x === 1 && pixel?.y === 1, "expected logical pixel (1,1)");
  const rgba = Object.freeze(colorRefToRgba(pixel.colorRef));
  const event = {
    schema: browserPresentEventSchema,
    schemaVersion: browserPresentEventSchemaVersion,
    kind: "pixel-present",
    presentSequence: 1,
    source: Object.freeze({
      checkpoint: "fixed-w2-window-pixel",
      paintSequence,
    }),
    frame: Object.freeze({
      width: bounds.right - bounds.left,
      height: bounds.bottom - bounds.top,
      pixelEncoding: "rgba8",
    }),
    pixels: Object.freeze([
      Object.freeze({
        x: pixel.x,
        y: pixel.y,
        rgba,
      }),
    ]),
  };
  validateFixedW2BrowserPresentEvent(event);
  return Object.freeze(event);
}
