import assert from "node:assert/strict";
import { pathToFileURL } from "node:url";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { build } from "esbuild";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const outputDirectory = path.join(root, ".work", "runtime-worker-state-machine");
fs.mkdirSync(outputDirectory, { recursive: true });
const output = path.join(outputDirectory, "execution-state.mjs");
await build({
  entryPoints: [path.join(root, "browser", "runtime-worker", "execution-state.ts")],
  bundle: true,
  format: "esm",
  outfile: output,
  platform: "node",
  target: "node22",
});
const {
  ExecutionStateMachine,
  ExecutionTransitionError,
  HostRequestIdentityError,
  PendingHostRequestTable,
} = await import(`${pathToFileURL(output).href}?v=${Date.now()}`);

const normal = new ExecutionStateMachine();
normal.transition("running", { cause: "slice", threadId: 8192 });
normal.transition("budget-yield", { cause: "budget", threadId: 8192 });
normal.transition("ready", { cause: "yield-complete", threadId: 8192 });
assert.deepEqual(normal.snapshot(), {
  state: "ready",
  transitionCount: 3,
  lastTransition: {
    sequence: 3,
    from: "budget-yield",
    to: "ready",
    cause: "yield-complete",
    threadId: 8192,
    requestId: null,
  },
});
const beforeIllegal = normal.snapshot();
assert.throws(
  () => normal.transition("host-completion", { cause: "illegal", threadId: 8192 }),
  ExecutionTransitionError,
);
assert.deepEqual(normal.snapshot(), beforeIllegal);

const preempted = new ExecutionStateMachine();
preempted.transition("running", { cause: "slice", threadId: 8192 });
preempted.transition("budget-yield", { cause: "budget", threadId: 8192 });
preempted.transition("thread-switch", {
  cause: "instruction-timeslice-expired",
  threadId: 8193,
});
preempted.transition("ready", {
  cause: "preemptive-thread-context-restored",
  threadId: 8193,
});
assert.deepEqual(preempted.snapshot().lastTransition, {
  sequence: 4,
  from: "thread-switch",
  to: "ready",
  cause: "preemptive-thread-context-restored",
  threadId: 8193,
  requestId: null,
});

const memoryFault = new ExecutionStateMachine();
memoryFault.transition("running", { cause: "slice", threadId: 8192 });
memoryFault.transition("memory-fault", { cause: "page-fault", threadId: 8192 });
memoryFault.transition("ready", { cause: "stack-growth-complete", threadId: 8192 });
assert.deepEqual(memoryFault.snapshot().lastTransition, {
  sequence: 3,
  from: "memory-fault",
  to: "ready",
  cause: "stack-growth-complete",
  threadId: 8192,
  requestId: null,
});

const host = new ExecutionStateMachine();
host.transition("running", { cause: "slice", threadId: 8192 });
host.transition("api-trap", { cause: "api", threadId: 8192 });
host.transition("waiting-host", {
  cause: "host-request",
  threadId: 8192,
  requestId: "1",
});
const table = new PendingHostRequestTable();
let delivered = null;
let rejected = null;
table.register({
  requestId: "1",
  threadId: 8192,
  service: "browser.test",
  completionPolicy: "resume-ready",
  cancelState: "active",
  resolve: (value) => {
    delivered = value;
  },
  reject: (error) => {
    rejected = error;
  },
});
assert.throws(
  () => table.complete("1", 8193, 42),
  (error) =>
    error instanceof HostRequestIdentityError && error.code === "host-request-wrong-thread",
);
assert.equal(table.size(), 1);
const pending = table.complete("1", 8192, 42);
host.transition("host-completion", {
  cause: "host-completion",
  threadId: pending.threadId,
  requestId: pending.requestId,
});
pending.resolve({ result: 42, completionPolicy: pending.completionPolicy });
host.transition("ready", {
  cause: "host-resume",
  threadId: pending.threadId,
  requestId: pending.requestId,
});
assert.deepEqual(delivered, { result: 42, completionPolicy: "resume-ready" });
assert.equal(rejected, null);
assert.throws(
  () => table.complete("1", 8192, 42),
  (error) =>
    error instanceof HostRequestIdentityError && error.code === "host-completion-duplicate",
);
assert.equal(host.state(), "ready");

const multiThread = new ExecutionStateMachine();
multiThread.transition("running", { cause: "initial-slice", threadId: 8192 });
multiThread.transition("api-trap", { cause: "initial-host-api", threadId: 8192 });
multiThread.transition("thread-switch", {
  cause: "initial-wait-second-runnable",
  threadId: 8193,
  requestId: "4",
});
multiThread.transition("ready", {
  cause: "second-context-restored",
  threadId: 8193,
  requestId: "4",
});
multiThread.transition("running", { cause: "second-slice", threadId: 8193 });
multiThread.transition("thread-exit", { cause: "second-return", threadId: 8193 });
multiThread.transition("waiting-host", {
  cause: "no-runnable-thread",
  threadId: 8192,
  requestId: "4",
});
multiThread.transition("host-completion", {
  cause: "initial-host-complete",
  threadId: 8192,
  requestId: "4",
});
multiThread.transition("thread-switch", {
  cause: "initial-runnable",
  threadId: 8192,
  requestId: "4",
});
multiThread.transition("ready", {
  cause: "initial-context-restored",
  threadId: 8192,
  requestId: "4",
});
assert.equal(multiThread.state(), "ready");

const callback = new ExecutionStateMachine();
callback.transition("running", { cause: "slice", threadId: 8192 });
callback.transition("api-trap", { cause: "callback-api", threadId: 8192 });
callback.transition("guest-callback", { cause: "callback-enter", threadId: 8192 });
callback.transition("api-trap", { cause: "nested-api", threadId: 8192 });
callback.transition("guest-callback", { cause: "nested-api-complete", threadId: 8192 });
callback.transition("callback-return", { cause: "nested-callback-return", threadId: 8192 });
callback.transition("guest-callback", { cause: "outer-callback-resume", threadId: 8192 });
callback.transition("callback-return", { cause: "outer-callback-return", threadId: 8192 });
callback.transition("ready", { cause: "callback-stack-empty", threadId: 8192 });
assert.equal(callback.state(), "ready");

const asyncCallback = new ExecutionStateMachine();
asyncCallback.transition("running", { cause: "slice", threadId: 8192 });
asyncCallback.transition("api-trap", { cause: "callback-api", threadId: 8192 });
asyncCallback.transition("guest-callback", { cause: "callback-enter", threadId: 8192 });
asyncCallback.transition("api-trap", { cause: "callback-async-api", threadId: 8192 });
asyncCallback.transition("waiting-host", {
  cause: "callback-host-request",
  threadId: 8192,
  requestId: "3",
});
const asyncCallbackTable = new PendingHostRequestTable();
let asyncCallbackDelivery = null;
asyncCallbackTable.register({
  requestId: "3",
  threadId: 8192,
  service: "browser.callback-async",
  completionPolicy: "resume-callback",
  cancelState: "active",
  resolve: (delivery) => {
    asyncCallbackDelivery = delivery;
  },
  reject: () => {
    throw new Error("callback request was rejected");
  },
});
const callbackPending = asyncCallbackTable.complete("3", 8192, 7);
asyncCallback.transition("host-completion", {
  cause: "callback-host-completion",
  threadId: callbackPending.threadId,
  requestId: callbackPending.requestId,
});
callbackPending.resolve({
  result: 7,
  completionPolicy: callbackPending.completionPolicy,
});
asyncCallback.transition("guest-callback", {
  cause: "callback-host-resume",
  threadId: callbackPending.threadId,
  requestId: callbackPending.requestId,
});
assert.deepEqual(asyncCallbackDelivery, {
  result: 7,
  completionPolicy: "resume-callback",
});
assert.equal(asyncCallback.state(), "guest-callback");

const cancelled = new ExecutionStateMachine();
cancelled.transition("running", { cause: "slice", threadId: 8192 });
cancelled.transition("api-trap", { cause: "api", threadId: 8192 });
cancelled.transition("waiting-host", {
  cause: "host-request",
  threadId: 8192,
  requestId: "2",
});
const cancelledTable = new PendingHostRequestTable();
let cancellationError = null;
cancelledTable.register({
  requestId: "2",
  threadId: 8192,
  service: "browser.cancel",
  completionPolicy: "resume-ready",
  cancelState: "active",
  resolve: () => {
    throw new Error("cancelled request resolved");
  },
  reject: (error) => {
    cancellationError = error;
  },
});
assert.equal(cancelledTable.rejectAll(new Error("cancelled")), 1);
cancelled.stop({ cause: "cancelled", threadId: 8192, requestId: "2" });
assert.equal(cancelledTable.size(), 0);
assert.equal(cancellationError?.message, "cancelled");
assert.throws(
  () => cancelledTable.complete("2", 8192, 42),
  (error) =>
    error instanceof HostRequestIdentityError && error.code === "host-completion-late",
);
assert.equal(cancelled.state(), "stopped");

console.log("Runtime Worker execution state contract passed");
