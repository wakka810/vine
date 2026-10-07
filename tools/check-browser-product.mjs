import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { createDevSourceHandler } from "./dev-source-server.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const assets = path.join(root, ".work", "browser-runtime");
const serve = process.argv.includes("--serve");
const arguments_ = process.argv.slice(2);
let gameRoot;
let servePort = 4173;
const seen = new Set();
for (let index = 0; index < arguments_.length; index += 1) {
  const argument = arguments_[index];
  if (seen.has(argument)) throw Error(`Duplicate option: ${argument}`);
  seen.add(argument);
  if (["--serve", "--scheduler-only", "--keyboard-only"].includes(argument)) continue;
  if (!["--dev-game-root", "--port"].includes(argument)) throw Error(`Unknown option: ${argument}`);
  if (!serve) throw Error(`${argument} requires --serve`);
  const value = arguments_[++index];
  if (!value || value.startsWith("--")) throw Error(`${argument} requires a value`);
  if (argument === "--dev-game-root") gameRoot = value;
  else {
    if (!/^(?:0|[1-9][0-9]*)$/u.test(value) || Number(value) > 65535) throw Error("Invalid --port");
    servePort = Number(value);
  }
}
const devSource = gameRoot === undefined ? null : await createDevSourceHandler(gameRoot);
const schedulerOnly = process.argv.includes("--scheduler-only");
const keyboardOnly = process.argv.includes("--keyboard-only");
const mime = { ".html": "text/html", ".js": "text/javascript", ".mjs": "text/javascript",
  ".wasm": "application/wasm" };
const server = http.createServer(async (request, response) => {
  try {
    if (devSource && await devSource(request, response)) return;
    const pathname = new URL(request.url ?? "/", "http://127.0.0.1").pathname;
    const file = path.resolve(assets, pathname === "/" ? "index.html" : pathname.slice(1));
    if (!file.startsWith(`${path.resolve(assets)}${path.sep}`)) throw Error("Path escapes browser assets");
    let bytes = await readFile(file);
    if (devSource && file === path.join(assets, "index.html")) {
      bytes = Buffer.from(bytes.toString("utf8").replace("<head>",
        '<head>\n<meta name="vine-dev-source" content="enabled">'));
    }
    response.writeHead(200, {
      "Content-Type": mime[path.extname(file)] ?? "application/octet-stream",
      "Cache-Control": "no-store",
      "Cross-Origin-Embedder-Policy": "require-corp",
      "Cross-Origin-Opener-Policy": "same-origin",
    });
    response.end(bytes);
  } catch {
    response.writeHead(404);
    response.end("not found");
  }
});
await new Promise((resolve, reject) => {
  server.once("error", reject);
  server.listen(serve ? servePort : 0, "127.0.0.1", resolve);
});
const url = `http://127.0.0.1:${server.address().port}/`;
if (serve) {
  console.log(`Vine game selector: ${url}`);
  if (devSource) console.log("Read-only development source enabled (Linux descriptor-relative reads; saves remain in OPFS)");
  await new Promise((resolve) => { process.once("SIGINT", resolve); process.once("SIGTERM", resolve); });
  await new Promise((resolve) => server.close(resolve));
  await devSource?.close();
} else {
  let chrome;
  const profile = await mkdtemp(path.join(os.tmpdir(), "vine-chrome-"));
  try {
    const candidates = [process.env.VINE_CHROME,
      process.env.ProgramFiles && path.join(process.env.ProgramFiles, "Google/Chrome/Application/chrome.exe"),
      process.env.LOCALAPPDATA && path.join(process.env.LOCALAPPDATA, "Google/Chrome/Application/chrome.exe")];
    const executable = candidates.find((candidate) => candidate && fs.existsSync(candidate));
    assert(executable, "Chrome was not found; set VINE_CHROME");
    const started = new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(Error("Chrome DevTools did not start")), 15000);
      let errors = "";
      chrome = spawn(executable, ["--headless=new", "--no-first-run", "--no-default-browser-check",
        ...(process.platform === "linux" ? ["--no-sandbox", "--disable-dev-shm-usage", "--disable-gpu"] : []),
        "--disable-background-networking", "--remote-debugging-port=0", `--user-data-dir=${profile}`,
        "--window-size=1280,800", "about:blank"], { stdio: ["ignore", "ignore", "pipe"] });
      chrome.stderr.on("data", (chunk) => {
        errors += String(chunk);
        const address = /DevTools listening on (ws:\/\/\S+)/u.exec(errors)?.[1];
        if (address) { clearTimeout(timer); resolve(address); }
      });
      chrome.once("error", reject);
      chrome.once("exit", (code) => reject(Error(`Chrome exited ${code}: ${errors}`)));
    });
    const browserSocket = new URL(await started);
    const targetResponse = await fetch(`http://${browserSocket.host}/json/new?about:blank`,
      { method: "PUT" });
    assert(targetResponse.ok, `Chrome refused test tab: ${targetResponse.status}`);
    const target = await targetResponse.json();
    const socket = new WebSocket(target.webSocketDebuggerUrl);
    await new Promise((resolve, reject) => {
      socket.addEventListener("open", resolve, { once: true });
      socket.addEventListener("error", reject, { once: true });
    });
    let serial = 0;
    const pending = new Map();
    socket.addEventListener("message", (event) => {
      const response = JSON.parse(event.data);
      const callback = pending.get(response.id);
      if (callback) { pending.delete(response.id); callback(response); }
    });
    const send = (method, params, timeout = 60000) => new Promise((resolve, reject) => {
      const id = ++serial;
      const timer = setTimeout(() => { pending.delete(id); reject(Error(`${method} timed out`)); }, timeout);
      pending.set(id, (response) => {
        clearTimeout(timer);
        if (response.error) reject(Error(`${method}: ${response.error.message}`));
        else resolve(response.result);
      });
      socket.send(JSON.stringify({ id, method, params }));
    });
    await send("Runtime.enable", {});
    await send("Page.enable", {});
    await send("Page.addScriptToEvaluateOnNewDocument", {
      source: `(() => {
        const NativeWorker = window.Worker;
        window.__vineInspections = [];
        window.Worker = class extends NativeWorker {
          constructor(...arguments_) {
            super(...arguments_);
            this.addEventListener('message', ({data}) => {
              if (data?.type === 'inspected-executable') window.__vineInspections.push(data);
            });
          }
        };
      })()`,
    });
    await send("Page.bringToFront", {});
    await send("Page.navigate", {url});
    for (let attempt = 0; attempt < 60; attempt++) {
      try {
        const ready = await send("Runtime.evaluate", {
          expression: `location.href === ${JSON.stringify(url)} && document.readyState === "complete" && Boolean(document.querySelector("#choose"))`,
          returnByValue: true,
        });
        if (ready.result?.value === true) break;
      } catch (error) {
        if (!/Cannot find default execution context|Execution context was destroyed/u.test(String(error))) throw error;
      }
      if (attempt === 59) throw Error("Chrome game tab did not finish loading");
      await new Promise(resolve => setTimeout(resolve, 50));
    }
    const audio = keyboardOnly ? null : await send("Runtime.evaluate", {
      expression: `(async () => {
        const context = new AudioContext({latencyHint:'interactive'});
        await context.resume();
        await context.audioWorklet.addModule(new URL('./pcm-worklet.js', location.href));
        const node = new AudioWorkletNode(context, 'vine-pcm', {numberOfInputs:0,
          numberOfOutputs:1,outputChannelCount:[2]});
        const analyser = new AnalyserNode(context, {fftSize:256});
        node.connect(analyser); analyser.connect(context.destination);
        const command = (operation, pcmBytes) => {
          const bytes = new ArrayBuffer(48 + pcmBytes), view = new DataView(bytes);
          [operation,1,0,1,44100,16,-6000,0,44100,0,1,pcmBytes].forEach((word,i)=>view.setUint32(i*4,word,true));
          for (let i=0;i<pcmBytes/2;i++) view.setInt16(48+i*2,Math.round(Math.sin(i*2*Math.PI*440/44100)*30000),true);
          return bytes;
        };
        const submit = bytes => new Promise((resolve,reject) => {
          node.port.onmessage = ({data}) => data.type==='ack' && (data.accepted ? resolve() : reject(Error('Chrome rejected PCM')));
          node.port.postMessage({type:'command',requestId:'1',bytes},[bytes]);
        });
        try {
          await submit(command(1, 4410*2));
          const samples = new Float32Array(256);
          let audible = false;
          for (let attempt=0;attempt<50;attempt++) {
            await new Promise(resolve=>setTimeout(resolve,10));
            analyser.getFloatTimeDomainData(samples);
            if (samples.some(value=>Math.abs(value)>0.0001)) { audible=true; break; }
          }
          if (!audible || context.state !== 'running') throw Error('Chrome worklet produced no PCM output');
          await submit(command(5,0));
          const stopTime = context.currentTime;
          const drainSeconds = (analyser.fftSize + 128) / context.sampleRate;
          const stopDeadline = Date.now() + 2000;
          let peak = Infinity, silent = false;
          // ACK retires the voice; sample advancement must replace the analyser's old window.
          while (Date.now() < stopDeadline) {
            await new Promise(resolve=>setTimeout(resolve,10));
            analyser.getFloatTimeDomainData(samples);
            peak = Math.max(...samples.map(value=>Math.abs(value)));
            if (context.state !== 'running') break;
            if (context.currentTime - stopTime >= drainSeconds && peak <= 0.00001) {
              silent = true; break;
            }
          }
          if (!silent) {
            const reason = context.state !== 'running' ? 'Chrome audio output stopped unexpectedly' :
              context.currentTime - stopTime < drainSeconds ? 'Chrome audio clock did not advance after Stop' :
              'Chrome Stop retained audible PCM after sample advancement';
            throw Error(reason + ' ' + JSON.stringify({
              state:context.state,stopTime,currentTime:context.currentTime,drainSeconds,peak,
              sampleRate:context.sampleRate,baseLatency:context.baseLatency,outputLatency:context.outputLatency}));
          }
          return true;
        } finally { node.disconnect(); analyser.disconnect(); node.port.close(); await context.close(); }
      })()`, awaitPromise: true, returnByValue: true, userGesture: true,
    });
    if (audio !== null && (audio.exceptionDetails || audio.result?.value !== true)) {
      throw Error(audio.exceptionDetails?.exception?.description ?? 'Chrome audio output failed');
    }
    const result = await send("Runtime.evaluate", {
      expression: `(async () => {
        const wait = async (check, label) => {
          const deadline = Date.now() + 15000;
          while (Date.now() < deadline) {
            const value = check(); if (value) return value;
            await new Promise(resolve => setTimeout(resolve, 40));
          }
          throw Error('Timed out waiting for ' + label + ': ' + document.querySelector('#status')?.textContent);
        };
        await wait(() => document.querySelector('#choose'), 'game selector');
        const fixtureNames = ${keyboardOnly} ? ['keyboard-input.exe'] : ['browser-vertical.exe', 'fls-get-value.exe', 'fls-exit-thread.exe',
          'enum-display-monitors.exe', 'd3d9-display.exe', 'pointer-input.exe', 'source-read.exe', 'bootstrap-fs.exe',
          'keyboard-input.exe', 'sleep-delay.exe', 'cursor-state.exe', 'timer-wait.exe', 'timer-threads.exe',
          'dll-lifecycle.exe', 'message-box.exe', 'heap-growth.exe', 'directshow.exe'];
        const root = await navigator.storage.getDirectory();
        const selected = await root.getDirectoryHandle('selected-game', {create:true});
        for (const name of fixtureNames) {
          const bytes = await fetch('./' + name).then(response => response.arrayBuffer());
          const stream = await (await selected.getFileHandle(name, {create:true})).createWritable();
          await stream.write(bytes); await stream.close();
        }
        window.showDirectoryPicker = async () => selected;
        const button = async (name) => wait(() => [...document.querySelectorAll('#listing button')]
          .find(element => element.textContent.includes(name)), name);
        const inspect = async (name, label) => {
          const index = window.__vineInspections.length;
          (await button(name)).click();
          const message = await wait(() => window.__vineInspections[index], label);
          if (message.result === null || !/^[0-9a-f]{64}$/.test(message.storageFingerprint)) {
            throw Error('Executable inspection failed: ' + message.error);
          }
          await wait(() => !document.querySelector('#launch').disabled, label + ' launch ready');
          return message.storageFingerprint;
        };
        const runKeyboardFixture = async () => {
          const fingerprint = await inspect('keyboard-input.exe', 'keyboard inspection');
          document.querySelector('#launch').click();
          // Launch awaits its real audio resource. Releasing the activation key
          // and changing focus in that interval must not consume guest input IDs.
          window.dispatchEvent(new KeyboardEvent('keyup', {code:'Enter', key:'Enter', bubbles:true}));
          window.dispatchEvent(new Event('blur'));
          window.dispatchEvent(new Event('focus'));
          const status = document.querySelector('#status');
          const deadline = Date.now() + 15000;
          while (Date.now() < deadline) {
            const detail = status.textContent;
            if (status.getAttribute('role') === 'alert') throw Error('Keyboard fixture failed: ' + detail);
            if (detail === 'Guest exited with code 0') return fingerprint;
            if (detail.startsWith('Guest exited with code ')) throw Error('Keyboard fixture failed: ' + detail);
            if (detail.includes('Running guest instructions')) {
              if (!document.hasFocus()) throw Error('Browser game tab is not focused for keyboard input');
              // Running precedes window creation; retry through the actual browser input path.
              window.dispatchEvent(new KeyboardEvent('keydown', {code:'KeyA', key:'a', repeat:false, bubbles:true}));
              window.dispatchEvent(new KeyboardEvent('keydown', {code:'KeyA', key:'a', repeat:true, bubbles:true}));
              window.dispatchEvent(new KeyboardEvent('keyup', {code:'KeyA', key:'a', repeat:false, bubbles:true}));
            }
            await new Promise(resolve => setTimeout(resolve, 100));
          }
          throw Error('Timed out waiting for guest keyboard messages and characters: ' + status.textContent);
        };
        if (${keyboardOnly}) {
          document.querySelector('#choose').click();
          return {fingerprint: await runKeyboardFixture()};
        }
        const bad = await (await selected.getFileHandle('bad.exe', {create:true})).createWritable();
        await bad.write(new TextEncoder().encode('invalid PE')); await bad.close();
        const folder = await selected.getDirectoryHandle('fixtures', {create:true});
        await folder.getDirectoryHandle('empty', {create:true});
        const storage = await folder.getDirectoryHandle('storage', {create:true});
        const asset = await fetch('./source-read-v1.dat').then(response => response.arrayBuffer());
        const source = await storage.getFileHandle('source-read-v1.dat', {create:true});
        const stream = await source.createWritable(); await stream.write(asset); await stream.close();
        const writeProbe = await storage.getFileHandle('write-probe.dat', {create:true});
        const probeStream = await writeProbe.createWritable();
        await probeStream.write(asset); await probeStream.close();
        const wildcardSource = await storage.getFileHandle('wildcard-only.dat', {create:true});
        const wildcardStream = await wildcardSource.createWritable();
        await wildcardStream.write(asset); await wildcardStream.close();
        const original = new Uint8Array(asset);
        document.querySelector('#choose').click();
        if (${schedulerOnly}) {
          const fingerprint = await inspect('timer-threads.exe', 'timer threads inspection');
          document.querySelector('#launch').click();
          await wait(() => document.querySelector('#status').textContent.includes('Guest exited with code 0'),
            'timer waits and all-idle thread join/event wake');
          return {fingerprint};
        }
        const fingerprint = await inspect('browser-vertical.exe', 'PE inspection');
        if (!fingerprint) throw Error('No per-directory writable tree');
        const NativeAudioContext = window.AudioContext, NativeWorker = window.Worker;
        let releaseAudio, pendingContext, workletReady = false, audioClosed = 0, workerFailed = false, workerTerminated = 0;
        const audioGate = new Promise(resolve => { releaseAudio = resolve; });
        const failedWorkerMessages = [];
        try {
          // A real failed worker fetch can precede the native audio worklet load.
          // Hold only readiness so its late resource must still be closed.
          window.AudioContext = new Proxy(NativeAudioContext, {construct(target, arguments_) {
            const context = Reflect.construct(target, arguments_, target);
            const addModule = context.audioWorklet.addModule.bind(context.audioWorklet), close = context.close.bind(context);
            context.audioWorklet.addModule = async (...arguments_) => {
              await addModule(...arguments_);
              workletReady = true;
              await audioGate;
            };
            context.close = () => { audioClosed++; return close(); };
            pendingContext = context;
            return context;
          }});
          window.Worker = new Proxy(NativeWorker, {construct(target, arguments_) {
            const workerUrl = new URL(arguments_[0], location.href);
            if (workerUrl.pathname.endsWith('/game-worker.js')) {
              arguments_[0] = new URL('./missing-game-worker.js', location.href);
            }
            const worker = Reflect.construct(target, arguments_, target);
            const postMessage = worker.postMessage.bind(worker), terminate = worker.terminate.bind(worker);
            worker.postMessage = (...arguments_) => {
              failedWorkerMessages.push(arguments_[0]?.type);
              return postMessage(...arguments_);
            };
            worker.terminate = () => { workerTerminated++; terminate(); };
            worker.addEventListener('error', () => { workerFailed = true; });
            return worker;
          }});
          document.querySelector('#launch').click();
          await wait(() => workerFailed && workletReady && document.querySelector('#status').getAttribute('role') === 'alert',
            'worker load failure during native audio setup');
          const errorDetail = document.querySelector('#status').textContent;
          releaseAudio();
          await wait(() => audioClosed > 0, 'late native audio closure');
          window.dispatchEvent(new KeyboardEvent('keyup', {code:'Enter', key:'Enter', bubbles:true}));
          window.dispatchEvent(new Event('focus'));
          if (workerTerminated !== 1 || audioClosed !== 1 || failedWorkerMessages.length !== 0 ||
              document.querySelector('#game-surface') !== null ||
              document.querySelector('#status').getAttribute('role') !== 'alert' ||
              document.querySelector('#status').textContent !== errorDetail) {
            throw Error('Failed worker was retained or late audio/input revived its launch');
          }
        } finally {
          releaseAudio();
          window.AudioContext = NativeAudioContext;
          window.Worker = NativeWorker;
          if (pendingContext && pendingContext.state !== 'closed') await pendingContext.close();
        }
        await inspect('browser-vertical.exe', 'inspection after worker load failure');
        document.querySelector('#launch').click();
        await wait(() => document.querySelector('#status').textContent.includes('Imported source DLL vine_browser_fixture.dll is absent'), 'missing DLL stops explicitly');
        const invalidLibrary = await (await selected.getFileHandle('vine_browser_fixture.dll', {create:true})).createWritable();
        await invalidLibrary.write(new TextEncoder().encode('not a PE32 DLL'));
        await invalidLibrary.close();
        await inspect('browser-vertical.exe', 'invalid DLL executable reinspection');
        document.querySelector('#launch').click();
        await wait(() => document.querySelector('#status').getAttribute('role') === 'alert', 'malformed source DLL rejection');
        const originalLibraries = new Map();
        for (const name of ['vine_browser_fixture.dll', 'lifecycle_engine.dll', 'lifecycle_codec.dll']) {
          const bytes = new Uint8Array(await fetch('./' + name).then(response => response.arrayBuffer()));
          originalLibraries.set(name, bytes);
          const stream = await (await selected.getFileHandle(name, {create:true})).createWritable();
          await stream.write(bytes); await stream.close();
        }
        await inspect('browser-vertical.exe', 'guest DLL executable reinspection');
        document.querySelector('#launch').click();
        await wait(() => document.querySelector('#status').textContent.includes('Guest exited with code 0'), 'guest source DLL imports and execution');
        await inspect('dll-lifecycle.exe', 'DLL lifecycle inspection');
        document.querySelector('#launch').click();
        await wait(() => document.querySelector('#status').textContent.includes('Guest exited with code 0'), 'transitive source DLL process attach');
        for (const [name, originalBytes] of originalLibraries) {
          const sourceBytes = new Uint8Array(await (await (await selected.getFileHandle(name)).getFile()).arrayBuffer());
          if (sourceBytes.length !== originalBytes.length ||
              sourceBytes.some((byte, index) => byte !== originalBytes[index])) {
            throw Error('Guest DLL loading modified immutable source ' + name);
          }
        }
        let sameBytesFingerprint;
        for (const name of ['fls-get-value.exe', 'enum-display-monitors.exe', 'd3d9-display.exe',
                            'source-read.exe', 'fls-exit-thread.exe', 'bootstrap-fs.exe', 'heap-growth.exe', 'directshow.exe']) {
          const gameFingerprint = await inspect(name, name + ' inspection');
          if (name === 'fls-get-value.exe') {
            sameBytesFingerprint = gameFingerprint;
          }
          if (name === 'source-read.exe') {
            const {openWritableTree} = await import('./writable-tree.js');
            const saves = await openWritableTree(gameFingerprint);
            await saves.createDirectory('GuestState');
            await saves.createEmpty('GuestState/retire.dat');
            await saves.writeExisting('GuestState/retire.dat', Uint8Array.of(77), 0);
          }
          document.querySelector('#launch').click();
          await wait(() => document.querySelector('#status').textContent.includes('Guest exited with code 0'), name + ' execution');
          if (name === 'source-read.exe') {
            const {openWritableTree} = await import('./writable-tree.js');
            const saves = await openWritableTree(gameFingerprint);
            if (await saves.stat('GuestState/retire.dat') !== null) {
              throw Error('Guest DeleteFileA did not remove the OPFS file');
            }
            if ((await saves.stat('GuestState/Slot01'))?.kind !== 'directory') {
              throw Error('Guest CreateDirectoryA did not reach OPFS');
            }
            const emptySave = await saves.stat('fixtures/storage/empty-save.dat');
            if (emptySave?.kind !== 'file' || emptySave.byteLength !== 0) {
              throw Error('Guest CreateFileA did not durably create its empty OPFS file');
            }
            let sourceSaveCreated = true;
            try { await storage.getFileHandle('empty-save.dat', {create:false}); }
            catch (error) { if (error?.name !== 'NotFoundError') throw error; sourceSaveCreated = false; }
            if (sourceSaveCreated) throw Error('Guest CreateFileA modified the source directory');
            const guestSave = await saves.read('fixtures/storage/guest-save.dat', 0, 5);
            if ([...guestSave ?? []].join() !== '49,0,255,66,7') {
              throw Error('Guest WriteFile did not durably store the save bytes');
            }
            let changedSource = true;
            try { await selected.getDirectoryHandle('GuestState', {create:false}); }
            catch (error) { if (error?.name !== 'NotFoundError') throw error; changedSource = false; }
            if (changedSource) throw Error('Guest directory creation modified the source root');
            await inspect('source-read.exe', 'guest OPFS directory reinspection');
            document.querySelector('#launch').click();
            await wait(() => document.querySelector('#status').textContent.includes('Guest exited with code 0'), 'guest OPFS directory relaunch');
            const restarted = await openWritableTree(gameFingerprint);
            if ((await restarted.stat('GuestState/Slot01'))?.kind !== 'directory') {
              throw Error('Guest directory was lost during relaunch');
            }
            const reopenedSave = await restarted.stat('fixtures/storage/empty-save.dat');
            if (reopenedSave?.kind !== 'file' || reopenedSave.byteLength !== 0) {
              throw Error('Guest file was lost during relaunch');
            }
            if ([...(await restarted.read('fixtures/storage/guest-save.dat', 0, 5)) ?? []].join() !== '49,0,255,66,7') {
              throw Error('Guest save did not survive a fresh worker/runtime');
            }
          }
          if (name === 'd3d9-display.exe') {
            await wait(() => {
              const canvas = document.querySelector('#game-surface');
              if (canvas?.width !== 200 || canvas.height !== 100) return false;
              const probe = document.createElement('canvas');
              probe.width = 1; probe.height = 1;
              const context = probe.getContext('2d');
              context.drawImage(canvas, 100, 50, 1, 1, 0, 0, 1, 1);
              const quad = [...context.getImageData(0, 0, 1, 1).data].join();
              context.drawImage(canvas, 5, 5, 1, 1, 0, 0, 1, 1);
              const background = [...context.getImageData(0, 0, 1, 1).data].join();
              return quad === '85,119,153,255' && background === '17,51,85,255';
            }, 'PE32 Present WebGL2 clear and textured quad pixels');
          }
        }
        await inspect('pointer-input.exe', 'pointer fixture inspection');
        document.querySelector('#launch').click();
        await wait(() => {
          const canvas = document.querySelector('#game-surface');
          if (canvas?.width !== 200 || canvas.height !== 100) return false;
          const probe = document.createElement('canvas');
          probe.width = 1; probe.height = 1;
          const context = probe.getContext('2d');
          context.drawImage(canvas, 100, 50, 1, 1, 0, 0, 1, 1);
          return [...context.getImageData(0, 0, 1, 1).data].join() === '85,119,153,255';
        }, 'pointer fixture visible frame');
        const pointerCanvas = document.querySelector('#game-surface');
        if (!document.hasFocus()) throw Error('Browser game tab is not focused for pointer input');
        const bounds = pointerCanvas.getBoundingClientRect();
        const at = {clientX: bounds.left + bounds.width / 2, clientY: bounds.top + bounds.height / 2,
          pointerId: 1, isPrimary: true, button: 0, bubbles: true};
        pointerCanvas.dispatchEvent(new PointerEvent('pointermove', at));
        pointerCanvas.dispatchEvent(new PointerEvent('pointerdown', at));
        await new Promise(resolve => setTimeout(resolve, 20));
        pointerCanvas.dispatchEvent(new PointerEvent('pointerup', at));
        await wait(() => document.querySelector('#status').textContent.includes('Guest exited with code 0'), 'guest pointer message and window callback');
        await inspect('cursor-state.exe', 'cursor inspection');
        document.querySelector('#launch').click();
        await wait(() => document.querySelector('#status').textContent.includes('Guest exited with code 0'), 'logical cursor state');
        await inspect('timer-threads.exe', 'timer threads inspection');
        document.querySelector('#launch').click();
        await wait(() => document.querySelector('#status').textContent.includes('Guest exited with code 0'), 'timer waits and all-idle thread join/event wake');
        await inspect('timer-wait.exe', 'timer wait inspection');
        document.querySelector('#launch').click();
        await wait(() => document.querySelector('#status').textContent.includes('Guest exited with code 0'), 'finite waitable timer');
        await inspect('sleep-delay.exe', 'Sleep inspection');
        document.querySelector('#launch').click();
        await wait(() => document.querySelector('#status').textContent.includes('Guest exited with code 0'), 'deferred Sleep and elapsed tick count');
        await runKeyboardFixture();
        await inspect('message-box.exe', 'message box inspection');
        document.querySelector('#launch').click();
        await wait(() => document.querySelector('#status').textContent.includes('Running guest instructions'),
          'message box input source');
        window.dispatchEvent(new KeyboardEvent('keydown', {code:'ControlLeft', key:'Control', repeat:false, bubbles:true}));
        const message = await wait(() => document.querySelector('dialog[open]'), 'message box resource');
        if (message.querySelector('h2')?.textContent !== 'Vine message fixture' ||
            message.querySelector('p')?.textContent !== 'あ\\r\\nMessage fixture' ||
            message.querySelectorAll('button').length !== 1) {
          throw Error('Rust CP932 message box did not render its content and buttons');
        }
        // Keep the dialog open while a guest background thread completes real Sleep.
        await new Promise(resolve => setTimeout(resolve, 150));
        window.dispatchEvent(new KeyboardEvent('keyup', {code:'ControlLeft', key:'Control', repeat:false, bubbles:true}));
        if (!message.open || document.querySelector('#status').textContent.includes('Guest exited')) {
          throw Error('Message box returned before an actual button choice');
        }
        message.querySelector('button').click();
        const confirmation = await wait(() => {
          const dialog = document.querySelector('dialog[open]');
          return dialog?.querySelector('h2')?.textContent === 'Vine button fixture' && dialog;
        }, 'message box button selection');
        if (confirmation.querySelectorAll('button').length !== 3 ||
            document.activeElement?.textContent !== 'いいえ') {
          throw Error('Rust message box default button did not receive browser focus');
        }
        [...confirmation.querySelectorAll('button')].find(button => button.textContent === 'キャンセル').click();
        await wait(() => document.querySelector('#status').textContent === 'Guest exited with code 0',
          'message box selected results and concurrent guest Sleep');
        if (document.querySelector('dialog')) throw Error('Message box resource survived completion');
        // The same LoadLibraryA call must distinguish missing input from a DLL
        // that exists but cannot yet be executed. Never hide missing loader support
        // behind ERROR_MOD_NOT_FOUND when the selected source contains the module.
        const dependency = await (await selected.getFileHandle('missing-dependency.dll', {create:true})).createWritable();
        await dependency.write(new Uint8Array([77, 90])); await dependency.close();
        await inspect('source-read.exe', 'existing DLL inspection');
        document.querySelector('#launch').click();
        await wait(() => document.querySelector('#status').textContent.includes('guest module loading is not implemented'), 'existing DLL unsupported stop');
        (await button('bad.exe')).click();
        await wait(() => document.querySelector('#status').textContent.includes('Executable rejected'), 'invalid PE stop');
        const after = new Uint8Array(await (await source.getFile()).arrayBuffer());
        if (after.length !== original.length || !after.every((byte, i) => byte === original[i])) {
          throw Error('Guest read modified source file');
        }
        const probeAfter = new Uint8Array(await (await writeProbe.getFile()).arrayBuffer());
        if (probeAfter.length !== original.length || !probeAfter.every((byte, i) => byte === original[i])) {
          throw Error('Guest write attempt modified unregistered source file');
        }
        const wildcardAfter = new Uint8Array(await (await wildcardSource.getFile()).arrayBuffer());
        if (wildcardAfter.length !== original.length || !wildcardAfter.every((byte, i) => byte === original[i])) {
          throw Error('Guest wildcard search modified source file');
        }
        const {GameSurface} = await import('./game-surface.js');
        const surfaceCanvas = new OffscreenCanvas(16, 8);
        const surface = new GameSurface(surfaceCanvas);
        const gl = surfaceCanvas.getContext('webgl2');
        const sample = (x, y) => {
          const bytes = new Uint8Array(4);
          gl.readPixels(x, y, 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, bytes);
          return [...bytes].join();
        };
        const quadBytes = new Uint8Array(112), vertexView = new DataView(quadBytes.buffer);
        [[0,0,0,0], [16,0,1,0], [0,8,0,1], [16,8,1,1]].forEach(([x,y,u,v], index) => {
          const base = index * 28;
          vertexView.setFloat32(base, x, true); vertexView.setFloat32(base + 4, y, true);
          vertexView.setFloat32(base + 12, 1, true); vertexView.setUint32(base + 16, 0xffffffff, true);
          vertexView.setFloat32(base + 20, u, true); vertexView.setFloat32(base + 24, v, true);
        });
        const draw = {vertices:[...quadBytes], texture:{width:4,height:4,pixels:null,fills:[
          {left:0,top:0,right:4,bottom:2,colorRef:0x0000ff},
          {left:0,top:2,right:4,bottom:4,colorRef:0x00ff00}]}};
        surface.present({width:16,height:8,colorArgb:0xff113355,draws:[draw]});
        if (sample(4,6) !== '255,0,0,255' || sample(4,1) !== '0,255,0,255') {
          throw Error('WebGL2 texture color channels or vertical coordinates are incorrect');
        }
        const valid = {width:16,height:8,colorArgb:0xff113355,draws:[]};
        const nanVertices = [...quadBytes]; nanVertices.splice(0,4,0,0,0xc0,0x7f);
        const badFrames = [{...valid,width:0}, {...valid,height:4097}, {...valid,colorArgb:-1},
          {...valid,draws:[{...draw,vertices:nanVertices}]},
          {...valid,draws:[{...draw,vertices:[1]}]},
          {...valid,draws:[{...draw,texture:{width:4096,height:4096,fills:[]}}]},
          {...valid,draws:[{...draw,texture:{width:4,height:4,fills:[{left:0,top:0,right:5,bottom:4,colorRef:0}]}}]}];
        for (const frame of badFrames) {
          let rejected = false;
          try { surface.present(frame); } catch { rejected = true; }
          if (!rejected || surfaceCanvas.width !== 16 || surfaceCanvas.height !== 8 ||
              sample(4,6) !== '255,0,0,255') throw Error('Malformed presentation changed the surface');
        }
        const snapshot = new Uint8Array(80);
        for (let y = 0; y < 4; y++) for (let x = 0; x < 4; x++) {
          snapshot.set(y < 2 ? [0,0,255,0] : [0,255,0,0], y * 20 + x * 4);
        }
        const pixelDraw = {...draw, texture:{width:4,height:4,fills:[],pixels:{offset:0,length:80,pitch:20}}};
        surface.present({...valid,draws:[pixelDraw]}, snapshot);
        if (sample(4,6) !== '255,0,0,255' || sample(4,1) !== '0,255,0,255') {
          throw Error('Binary BGRX upload did not preserve pitch, channel order or opaque alpha');
        }
        for (const pixels of [{offset:1,length:80,pitch:20}, {offset:0,length:79,pitch:20},
                              {offset:0,length:80,pitch:12}, {offset:-1,length:80,pitch:20}]) {
          let rejected = false;
          try { surface.present({...valid,draws:[{...pixelDraw,texture:{...pixelDraw.texture,pixels}}]}, snapshot); }
          catch { rejected = true; }
          if (!rejected || sample(4,6) !== '255,0,0,255') throw Error('Malformed binary range changed the frame');
        }
        const loss = gl.getExtension('WEBGL_lose_context');
        if (!loss) throw Error('Chrome lacks the context-loss test extension');
        loss.loseContext();
        await new Promise(resolve => setTimeout(resolve, 20));
        let lostRejected = false;
        try { surface.present(valid); } catch { lostRejected = true; }
        if (!lostRejected) throw Error('Lost WebGL2 context silently presented');
        const {rasterizeGdiText} = await import('./gdi-text-raster.js');
        const rasterRequest = [1,0,0,1,0,1,0,0,0,37,19,100,37,(-19)>>>0,40,760,1,97];
        const raster = rasterizeGdiText(rasterRequest);
        if (raster.length !== 760 || !raster.some(value => value !== 0)) {
          throw Error('Bounded unprofiled glyph did not rasterize');
        }
        for (let row = 0; row < 19; row++) {
          if (raster.slice(row * 40 + 37, row * 40 + 40).some(value => value !== 0)) {
            throw Error('Glyph raster wrote DIB row padding');
          }
        }
        for (const corrupt of [[15, 0xffffffff], [14, 37], [9, NaN], [16, 3]]) {
          const badRequest = [...rasterRequest]; badRequest[corrupt[0]] = corrupt[1];
          let rejected = false;
          try { rasterizeGdiText(badRequest); } catch { rejected = true; }
          if (!rejected) throw Error('Malformed glyph raster request was accepted');
        }
        const {openWritableTree} = await import('./writable-tree.js');
        const tree = await openWritableTree(fingerprint);
        await tree.createDirectory('saves');
        await tree.createEmpty('saves\\\\slot.dat');
        await tree.writeExisting('saves\\\\slot.dat', Uint8Array.of(0, 92, 255), 0);
        await tree.writeExisting('saves/slot.dat', Uint8Array.of(11), 1);
        const reopened = await openWritableTree(fingerprint);
        if ((await reopened.stat('saves')).kind !== 'directory' ||
            (await reopened.stat('saves/slot.dat')).byteLength !== 3 ||
            [...await reopened.read('saves/slot.dat', 0, 3)].join() !== '0,11,255') {
          throw Error('OPFS partial save did not survive tree reopen');
        }
        await tree.writeExisting('SAVES/SLOT.DAT', Uint8Array.of(42), 2);
        if ([...await reopened.read('saves/slot.dat', 0, 3)].join() !== '0,11,42' ||
            (await reopened.stat('SAVES/SLOT.DAT')).byteLength !== 3) {
          throw Error('Windows-case save path did not resolve the existing OPFS file');
        }
        await tree.createDirectory('SAVES');
        let missingWriteRejected = false;
        try { await tree.writeExisting('SAVES/absent.dat', Uint8Array.of(1), 0); }
        catch (error) { missingWriteRejected = error?.name === 'NotFoundError'; }
        if (!missingWriteRejected || await tree.stat('saves/absent.dat')) {
          throw Error('WriteFile-style OPFS write recreated a missing file');
        }
        await tree.createEmpty('saves/sparse.dat');
        await tree.writeExisting('saves/sparse.dat', Uint8Array.of(17), 2);
        if ([...await reopened.read('saves/sparse.dat', 0, 3)].join() !== '0,0,17') {
          throw Error('OPFS sparse save did not survive tree reopen');
        }
        const executable = await (await selected.getFileHandle('fls-get-value.exe')).getFile();
        const copy = await (await selected.getFileHandle('copy.exe', {create:true})).createWritable();
        await copy.write(await executable.arrayBuffer()); await copy.close();
        window.showDirectoryPicker = async () => selected;
        document.querySelector('#choose').click();
        await button('copy.exe');
        const copyFingerprint = await inspect('copy.exe', 'identical-byte copy inspection');
        if (!sameBytesFingerprint || !copyFingerprint || copyFingerprint === sameBytesFingerprint) {
          throw Error('Same-directory executables with identical bytes share save state');
        }
        document.querySelector('#launch').click();
        await wait(() => document.querySelector('#status').textContent.includes('Guest exited with code 0'), 'identical-byte copy execution');
        const second = await root.getDirectoryHandle('other-game', {create:true});
        const other = await (await second.getFileHandle('fls-get-value.exe', {create:true})).createWritable();
        await other.write(await executable.arrayBuffer()); await other.close();
        window.showDirectoryPicker = async () => second;
        document.querySelector('#choose').click();
        await wait(() => document.querySelector('#location').textContent === 'other-game', 'second source root');
        const secondFingerprint = await inspect('fls-get-value.exe', 'second inspection');
        if (!secondFingerprint || secondFingerprint === sameBytesFingerprint) throw Error('Separate roots share save state');
        return {fingerprint, secondFingerprint};
      })()`,
      awaitPromise: true, returnByValue: true, userGesture: true,
    }, 90000);
    if (result.exceptionDetails) throw Error(result.exceptionDetails.exception?.description ?? JSON.stringify(result.exceptionDetails));
    assert.match(result.result.value.fingerprint, /^[0-9a-f]{64}$/u);
    if (keyboardOnly) {
      console.log("Chrome PE32 keyboard down/repeat/up and translated characters reached the guest window procedure");
    } else if (schedulerOnly) {
      console.log("Chrome PE32 timer waits and all-idle thread join/event wake passed");
    } else {
      assert.match(result.result.value.secondFingerprint, /^[0-9a-f]{64}$/u);
      console.log("Chrome selected-PE32 execution, source DLL imports/attach/reuse, concurrent large heaps/free/reuse, WebGL2 pixels, real Web Audio PCM/Stop, input, timer waits/Sleep, callbacks, COM, immutable source, guest OPFS save/close/fresh-runtime/load, directory persistence, and isolated OPFS primitives passed");
    }
    socket.close();
  } finally {
    chrome?.kill();
    await rm(profile, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
    server.close();
  }
}
