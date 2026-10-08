import assert from "node:assert/strict";
import { build } from "esbuild";

const { outputFiles } = await build({ entryPoints: ["browser/runtime-worker/browser-delay.ts"],
  bundle: true, write: false, platform: "node", format: "esm" });
const { browserDelayMilliseconds: delay } = await import(
  `data:text/javascript;base64,${Buffer.from(outputFiles[0].contents).toString("base64")}`,
);
assert.equal(delay("browser.sleep", [1]), 1);
assert.equal(delay("browser.timer-delay", [0x7fff_ffff]), 0x7fff_ffff);
const active = [1, 25, 123, 0, 100, 2, 80, 81, 0xffff_ffff, 1];
const inactive = [2, 100, 100, 2, 80, 81, 0xffff_ffff, 1];
assert.equal(delay("browser.audio-delay", active), 25);
assert.equal(delay("browser.audio-delay", inactive), 100);
assert.equal(delay("browser.audio-delay", inactive.with(1, 7)), 7,
  "a retry retains the captured timeout and host start while waiting only its remaining delay");
for (const words of [[], [0], [0x8000_0000], [NaN], [1.5], [1, 2]]) {
  assert.throws(() => delay("browser.timer-delay", words));
}
for (const words of [active.slice(0, -1), [...active, 1], active.with(0, 3), active.with(1, 101),
  active.with(4, 0), active.with(4, 101), active.with(5, 0), active.with(5, 33),
  active.with(8, -1), active.with(9, 0x1_0000_0000), active.with(8, NaN), inactive.slice(0, -2)]) {
  assert.throws(() => delay("browser.audio-delay", words));
}
assert.throws(() => delay("unknown", [1]));
console.log("browser delays: finite deadlines, audio retry payloads and malformed boundary rejection passed");
