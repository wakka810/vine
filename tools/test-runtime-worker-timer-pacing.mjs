import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { fileURLToPath } from "node:url";

import { build } from "esbuild";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const outputDirectory = path.join(root, ".work", "runtime-worker-timer-pacing");
fs.mkdirSync(outputDirectory, { recursive: true });
const output = path.join(outputDirectory, "timer-pacing.mjs");
await build({
  entryPoints: [path.join(root, "browser", "runtime-worker", "timer-pacing.ts")],
  bundle: true,
  format: "esm",
  outfile: output,
  platform: "node",
  target: "node22",
});
const { TimerPacingService } = await import(`${pathToFileURL(output).href}?v=${Date.now()}`);

class FakeTimerDriver {
  nowMilliseconds = 0;
  nextHandle = 1;
  active = new Map();
  callbacks = new Map();

  now() {
    return this.nowMilliseconds;
  }

  setTimeout(callback, delayMilliseconds) {
    const handle = this.nextHandle++;
    const timer = {
      callback,
      deadline: this.nowMilliseconds + delayMilliseconds,
    };
    this.active.set(handle, timer);
    this.callbacks.set(handle, callback);
    return handle;
  }

  clearTimeout(handle) {
    this.active.delete(handle);
  }

  elapse(milliseconds) {
    this.nowMilliseconds += milliseconds;
  }

  runNextDue() {
    const due = [...this.active.entries()]
      .filter(([, timer]) => timer.deadline <= this.nowMilliseconds)
      .sort((left, right) => left[1].deadline - right[1].deadline || left[0] - right[0])[0];
    if (due === undefined) {
      return false;
    }
    this.active.delete(due[0]);
    due[1].callback();
    return true;
  }

  invokeStale(handle) {
    this.callbacks.get(handle)?.();
  }
}

function request(requestId, delayMilliseconds, completed) {
  return {
    requestId,
    threadId: 8192,
    delayMilliseconds,
    complete: () => completed.push(requestId),
  };
}

{
  const driver = new FakeTimerDriver();
  const completed = [];
  const pacing = new TimerPacingService(driver, { state: "active", generation: 1 });
  pacing.schedule(request("1", 5, completed));
  driver.elapse(4);
  assert.equal(driver.runNextDue(), false);
  driver.elapse(1);
  assert.equal(driver.runNextDue(), true);
  assert.deepEqual(completed, ["1"]);
  assert.deepEqual(pacing.metrics(), {
    requests: 1,
    completions: 1,
    cancellations: 0,
    freezes: 0,
    resumes: 0,
    staleCallbacks: 0,
    boundedCatchUps: 0,
    pending: 0,
  });
}

{
  const driver = new FakeTimerDriver();
  const completed = [];
  const pacing = new TimerPacingService(driver, { state: "active", generation: 1 });
  pacing.schedule(request("2", 10, completed));
  driver.elapse(3);
  pacing.updateLifecycle({ state: "frozen", generation: 2 });
  driver.elapse(100);
  assert.equal(driver.runNextDue(), false);
  pacing.updateLifecycle({ state: "active", generation: 3 });
  driver.elapse(6);
  assert.equal(driver.runNextDue(), false);
  driver.elapse(1);
  assert.equal(driver.runNextDue(), true);
  assert.deepEqual(completed, ["2"]);
  assert.equal(pacing.metrics().freezes, 1);
  assert.equal(pacing.metrics().resumes, 1);
}

{
  const driver = new FakeTimerDriver();
  const completed = [];
  const pacing = new TimerPacingService(driver, { state: "active", generation: 1 });
  pacing.schedule(request("3", 5, completed));
  const staleHandle = driver.nextHandle - 1;
  assert.equal(pacing.cancelAll(), 1);
  driver.invokeStale(staleHandle);
  assert.deepEqual(completed, []);
  assert.equal(pacing.metrics().cancellations, 1);
  assert.equal(pacing.metrics().staleCallbacks, 1);
  assert.throws(
    () => pacing.updateLifecycle({ state: "frozen", generation: 1 }),
    /generation is stale/u,
  );
}

{
  const driver = new FakeTimerDriver();
  const completed = [];
  const pacing = new TimerPacingService(driver, { state: "active", generation: 1 });
  pacing.schedule(request("4", 5, completed));
  pacing.schedule(request("5", 5, completed));
  driver.elapse(10);
  pacing.updateLifecycle({ state: "frozen", generation: 2 });
  pacing.updateLifecycle({ state: "active", generation: 3 });
  assert.equal(driver.runNextDue(), true);
  assert.deepEqual(completed, ["4"]);
  assert.equal(pacing.metrics().pending, 1);
  assert.equal(driver.runNextDue(), true);
  assert.deepEqual(completed, ["4", "5"]);
  assert.equal(pacing.metrics().boundedCatchUps, 2);
}

console.log("Runtime Worker timer pacing contract passed");
