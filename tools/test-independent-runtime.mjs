import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import { build } from "esbuild";

// The production Worker and Rust Runtime run unchanged. Test shims select the
// explicit backend and provide immutable source capability bytes; no Windows API,
// scheduler, callback or memory result is mocked.
const resources = {
  "./execution-engine.js": `import { instantiateExecutionEngine as instantiate } from "./browser/runtime-worker/execution-engine.ts";
    export async function instantiateExecutionEngine(url, options) {
      const engine = await instantiate(url, {...options, backend:"recompiler", recompilerNumericMode:"strict",
        recompilerLinkedRegions:${process.argv.includes("--linked-regions")},
        recompilerFpIr:${process.argv.includes("--fp-ir")},
        recompilerTypedNumeric:${process.argv.includes("--typed-numeric")},
        recompilerBindingGlobal:${process.argv.includes("--binding-global")},
        recompilerDirectLinks:${process.argv.includes("--direct-links")},
        recompilerControlGraph:${process.argv.includes("--control-graph")},
        recompilerPredicateIr:${process.argv.includes("--predicate-ir")},
        recompilerFlowIr:${process.argv.includes("--flow-ir")},
        recompilerLoopMemory:${process.argv.includes("--loop-memory")},
        recompilerExpandedCache:${process.argv.includes("--expanded-cache")}});
      const prepare = engine.prepareExecution.bind(engine);
      let prepared = false;
      engine.prepareExecution = async roots => {
        const before = engine.saveCpuContext();
        const result = await prepare(roots);
        if (JSON.stringify(engine.saveCpuContext()) !== JSON.stringify(before)) throw Error("Preparation changed CPU state");
        if (engine.collectChangedPages().length !== 0) throw Error("Preparation wrote guest bytes");
        prepared = true;
        self.postMessage({type:"fixture-boundary",method:"prepareExecution",...result});
        return result;
      };
      for (const method of ["createCpuContext","restoreCpuContext","acknowledgeWriteback","completeTimestampCounter"]) {
        const call = engine[method].bind(engine);
        engine[method] = (...args) => {
          const resumed = method === "restoreCpuContext" ? args[0].snapshot : null;
          const crossesScasThread = resumed?.backend === "recompiler" && resumed.abiVersion === 2 &&
            resumed.words[108] === 1 && engine.saveCpuContext().snapshot.words[28] !== resumed.words[28];
          const result = call(...args);
          if (crossesScasThread) self.postMessage({type:"fixture-boundary",method:"scasContinuationThreadSwitch"});
          self.postMessage({type:"fixture-boundary", method}); return result; };
      }
      const execute = engine.executeBudget.bind(engine);
      engine.executeBudget = budget => {
        if (!prepared) throw Error("Guest executed before code preparation");
        const slice = execute(budget), words = slice.context.snapshot.words;
        if (words[100] === 12 || words[100] === 13) {
          self.postMessage({type:"fixture-boundary",method:"repPartialYield",instructions:slice.instructions,
            remaining:words[1],elements:words[103],eip:words[8]});
          if (slice.instructions === 0) self.postMessage({type:"fixture-boundary",method:"repZeroRetiredYield"});
        }
        return slice;
      };
      return engine;
    }`,
  "./runtime-bridge.js": `import { RuntimeBridge as Actual } from "./browser/runtime-worker/runtime-bridge.ts";
    export const RuntimeBridge = { instantiate: async url => {
      const bridge = await Actual.instantiate(url);
      const put = bridge.storeCpuSnapshot.bind(bridge), get = bridge.loadCpuSnapshot.bind(bridge);
      let lastStoredThread = null;
      bridge.storeCpuSnapshot = (thread, revision, snapshot) => {
        const result = put(thread, revision, snapshot); lastStoredThread = thread;
        if (snapshot.backend === "recompiler" && snapshot.abiVersion === 2 && snapshot.words[108] === 1)
          self.postMessage({type:"fixture-boundary",method:"scasContinuationPark",thread,eip:snapshot.words[8]});
        return result;
      };
      bridge.loadCpuSnapshot = thread => {
        const result = get(thread);
        if (result.snapshot.backend === "recompiler" && result.snapshot.abiVersion === 2 && result.snapshot.words[108] === 1)
          self.postMessage({type:"fixture-boundary",method:"scasContinuationLoad",thread,
            eip:result.snapshot.words[8],lastStoredThread});
        return result;
      };
      for (const method of ["dispatch","completeCallback","servicePageFault","scheduleNextThread"]) {
        const call = bridge[method].bind(bridge);
        bridge[method] = (...args) => { const output = call(...args);
          self.postMessage({type:"fixture-boundary", method, kind:output.metadata.kind}); return output; };
      }
      return bridge;
    }};`,
  "../storage/source-directory.js": `export const gameStorageFingerprint = async () => "b".repeat(64);`,
  "./source-explorer.js": `export async function openSourceExecutable(root, segments, name) {
      const bytes = new Uint8Array(await (await fetch("/"+name)).arrayBuffer());
      return {size:bytes.length,lastModified:0,sourceVersion:"immutable-fixture",arrayBuffer:async()=>bytes.buffer};
    }
    export const browseSourceDirectory = async () => [];
    export const listSourceDirectory = async () => [];
    export const inspectSourcePath = async () => null;
    export const openSourceDll = async () => {throw Error("Unexpected source DLL");};
    export const readSourceRange = async () => {throw Error("Unexpected source read");};`,
};
const bundle = await build({ entryPoints: ["browser/runtime-worker/game-worker.ts"],
  bundle: true, write: false, platform: "browser", format: "esm", target: "chrome140",
  plugins: [{ name: "independent-runtime-resource-fixture", setup(plugin) {
    plugin.onResolve({filter:/^\./}, args => args.importer.endsWith("game-worker.ts") && resources[args.path]
      ? {path:args.path,namespace:"fixture-resource"} : undefined);
    plugin.onLoad({filter:/.*/,namespace:"fixture-resource"}, args => ({contents:resources[args.path],
      loader:"js",resolveDir:process.cwd()}));
  }}],
});
const argument = name => {
  const index = process.argv.indexOf(name);
  if (index < 0) return null;
  assert(process.argv[index + 1] && !process.argv[index + 1].startsWith("--"), `${name} requires a path`);
  return process.argv[index + 1];
};
const selectedFixture = argument("--fixture");
const fixtureNames = selectedFixture ? [path.basename(selectedFixture)] : process.argv.includes("--bootstrap-only") ? ["bootstrap-fs.exe"] :
  ["bootstrap-fs.exe", "enum-display-monitors.exe", "heap-growth.exe", "timer-threads.exe"];
const assets = new Map([
  ["/worker.js", bundle.outputFiles[0].contents],
  ["/runtime-core.wasm", await readFile(argument("--runtime") ?? "target/wasm32-unknown-unknown/release/vine_runtime_wasm.wasm")],
  ["/vine_recompiler.wasm", await readFile(argument("--compiler") ?? "target/wasm32-unknown-unknown/release/vine_recompiler.wasm")],
]);
const fixtures = [];
for (const name of fixtureNames) {
  const bytes = await readFile(selectedFixture ?? `.work/toolchain-spike/${name}`);
  assets.set(`/${name}`, bytes); fixtures.push({name, sha256:createHash("sha256").update(bytes).digest("hex")});
}
const server = http.createServer((request,response) => {
  const pathname = new URL(request.url,"http://localhost").pathname;
  const bytes = pathname === "/" ? Buffer.from("<!doctype html><title>Independent Runtime integration</title>") : assets.get(pathname);
  response.writeHead(bytes ? 200 : 404, {"Content-Type":pathname.endsWith(".js") ? "text/javascript" :
    pathname.endsWith(".wasm") ? "application/wasm" : "text/html", "Cache-Control":"no-store",
    "Cross-Origin-Embedder-Policy":"require-corp","Cross-Origin-Opener-Policy":"same-origin"});
  response.end(bytes ?? "Not found");
});
await new Promise(resolve=>server.listen(0,"127.0.0.1",resolve));
const url = `http://127.0.0.1:${server.address().port}/`;
const profile = await mkdtemp(path.join(os.tmpdir(),"vine-independent-runtime-"));
let chrome, socket;
try {
  const executable = [process.env.VINE_CHROME,
    process.env.ProgramFiles && path.join(process.env.ProgramFiles,"Google/Chrome/Application/chrome.exe"),
    process.env.LOCALAPPDATA && path.join(process.env.LOCALAPPDATA,"Google/Chrome/Application/chrome.exe")]
    .find(candidate=>candidate && fs.existsSync(candidate));
  assert(executable,"Chrome was not found; set VINE_CHROME");
  const address = await new Promise((resolve,reject)=> {
    const timeout=setTimeout(()=>reject(Error("Chrome DevTools did not start")),15000); let errors="";
    chrome=spawn(executable,["--headless=new","--no-first-run","--no-default-browser-check",
      "--disable-background-networking","--remote-debugging-port=0",`--user-data-dir=${profile}`,"about:blank"],
    {stdio:["ignore","ignore","pipe"],windowsHide:true});
    chrome.stderr.on("data",chunk=> {errors+=String(chunk); const match=/DevTools listening on (ws:\/\/\S+)/u.exec(errors);
      if(match){clearTimeout(timeout);resolve(new URL(match[1]));}});
    chrome.once("error",reject);
  });
  const target=await (await fetch(`http://${address.host}/json/new?${encodeURIComponent(url)}`,{method:"PUT"})).json();
  socket=new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((resolve,reject)=>{socket.addEventListener("open",resolve,{once:true});socket.addEventListener("error",reject,{once:true});});
  let serial=0;const pending=new Map();
  socket.addEventListener("message",event=>{const result=JSON.parse(event.data);pending.get(result.id)?.(result);});
  const send=(method,params)=>new Promise((resolve,reject)=>{
    const id=++serial;const timeout=setTimeout(()=>{pending.delete(id);reject(Error(`${method} timed out`));},60000);
    pending.set(id,result=>{clearTimeout(timeout);pending.delete(id);result.error ? reject(Error(result.error.message)) : resolve(result.result);});
    socket.send(JSON.stringify({id,method,params}));
  });
  await send("Runtime.enable",{});
  await send("Page.navigate",{url});
  const result=await send("Runtime.evaluate",{awaitPromise:true,returnByValue:true,expression:`(async()=>{
    const results=[];
    for(const fixture of ${JSON.stringify(fixtures)}) {
      const worker=new Worker("/worker.js?executionMode=strict",{type:"module"}); const counts={}, repProgress=[]; let preparation;
      try {
        const completion=new Promise((resolve,reject)=>{
          const timeout=setTimeout(()=>reject(Error("Fixture timed out: "+fixture.name)),45000);
          worker.onmessage=({data})=>{
            if(data.type==="fixture-boundary") {
              counts[data.method]=(counts[data.method]??0)+1;
              if(data.method==="prepareExecution") preparation={milliseconds:data.milliseconds,limited:data.limited};
              if(data.method==="repPartialYield" && repProgress.length<32) repProgress.push(data);
            }
            if(data.type==="game-error"){clearTimeout(timeout);reject(Error(fixture.name+": "+data.detail));}
            if(data.type==="game-exit"){clearTimeout(timeout);resolve(data);}
          };worker.onerror=event=>{clearTimeout(timeout);reject(Error(event.message));};
        });
        const canvas=new OffscreenCanvas(640,480);
        worker.postMessage({type:"launch-executable",root:{kind:"directory"},segments:[],name:fixture.name,
          sha256:fixture.sha256,storageFingerprint:"b".repeat(64),canvas},[canvas]);
        results.push({name:fixture.name,...await completion,counts,repProgress,preparation});
      } catch(error) {
        results.push({name:fixture.name,error:String(error),counts});
      } finally {worker.terminate();}
    }
    return results;
  })()`});
  assert(!result.exceptionDetails,JSON.stringify(result.exceptionDetails));
  const rows=result.result.value;
  const report={backend:"independent",chrome:true,linkedRegions:process.argv.includes("--linked-regions"),
    directLinks:process.argv.includes("--direct-links"),
    controlGraph:process.argv.includes("--control-graph"),
    fpIr:process.argv.includes("--fp-ir"),typedNumeric:process.argv.includes("--typed-numeric"),
    expandedCache:process.argv.includes("--expanded-cache"),windowsState:"Rust Runtime",
    compilerSha256:createHash("sha256").update(assets.get("/vine_recompiler.wasm")).digest("hex"),
    runtimeSha256:createHash("sha256").update(assets.get("/runtime-core.wasm")).digest("hex"),
    fixtures:rows};
  const json=JSON.stringify(report,null,2);
  console.log(json);
  const outputIndex=process.argv.indexOf("--output");
  if(outputIndex>=0) {
    assert(process.argv[outputIndex+1],"--output requires a new report path");
    await writeFile(process.argv[outputIndex+1],json+"\n",{flag:"wx"});
  }
  for(const row of rows) {
    assert(!row.error,row.error);
    assert.equal(row.counts.prepareExecution,1, `${row.name}: prepares code before the first guest instruction`);
    assert.equal(row.exitCode,0,`${row.name} guest failure`);
    if(row.name==="enum-display-monitors.exe") assert(row.counts.completeCallback>0,"actual callback was not reached");
    if(row.name==="heap-growth.exe") assert(row.counts.servicePageFault>0,"actual lazy memory fault was not reached");
    if(row.name==="timer-threads.exe") assert(row.counts.createCpuContext>=2 && row.counts.restoreCpuContext>=2,
      "actual guest thread creation/switch was not reached");
    if(process.argv.includes("--require-rep-progress")) {
      assert(row.counts.repPartialYield>0 && row.counts.repZeroRetiredYield>0,
        "actual incomplete REP did not yield with zero whole instructions retired");
      assert(row.counts.servicePageFault>0,"REP fixture did not reach actual lazy page faults");
    }
    if(process.argv.includes("--require-timestamp-counter")) {
      assert.equal(row.counts.completeTimestampCounter,3,"fixture must complete three real-clock observations");
      assert(row.counts.scheduleNextThread>0,"timestamp fixture did not cross actual Sleep scheduling");
    }
    if(process.argv.includes("--require-scas-context")) {
      assert(row.counts.scasContinuationPark>0 && row.counts.scasContinuationLoad>0,
        "SCAS continuation must cross actual Rust parked CPU storage");
      assert(row.counts.scasContinuationThreadSwitch>0,
        "A parked SCAS must resume after a different guest FS/thread context");
      assert(row.counts.createCpuContext>0 && row.counts.restoreCpuContext>0 && row.counts.repPartialYield>1,
        "SCAS fixture must yield and switch actual guest threads");
    }
  }
} finally {
  socket?.close();
  if(chrome && chrome.exitCode===null) {chrome.kill();await new Promise(resolve=>chrome.once("exit",resolve));}
  await new Promise(resolve=>server.close(resolve));
  const cleanupTarget = path.resolve(profile), temporaryRoot = path.resolve(os.tmpdir());
  assert(path.dirname(cleanupTarget) === temporaryRoot &&
    path.basename(cleanupTarget).startsWith("vine-independent-runtime-"), "invalid test profile cleanup target");
  await rm(cleanupTarget,{recursive:true,force:true,maxRetries:10,retryDelay:100});
}
