// Execute the production worker with deterministic browser resources. No
// production exports or copied scheduling/completion implementation are used.
import assert from 'node:assert/strict';
import { build } from 'esbuild';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const directory = resolve('browser/runtime-worker');
const source = resolve(directory, 'game-worker.ts');
const dependencies = {
  '../storage/source-directory.js': "export const gameStorageFingerprint = async () => 'storage';",
  './execution-engine.js': 'export const instantiateExecutionEngine = async () => globalThis.workerObjectWaitDiagnostics.engine;',
  './runtime-bridge.js': 'export const RuntimeBridge = { instantiate: async () => globalThis.workerObjectWaitDiagnostics.bridge };',
  './game-surface.js': "export class GameSurface { backend = 'webgl2'; present() { throw Error('Unexpected presentation'); } }",
  './gdi-text-raster.js': "export const rasterizeGdiText = () => { throw Error('Unexpected GDI work'); };",
  './media-decoder.js': 'export class MediaDecoder {} export class UnsupportedMediaFormat extends Error {}',
  './source-explorer.js': `export const openSourceExecutable = async () => ({size:1,lastModified:0,arrayBuffer:async()=>new Uint8Array([0]).buffer});
    export const browseSourceDirectory=async()=>[]; export const inspectSourcePath=async()=>null;
    export const listSourceDirectory=async()=>[]; export const openSourceDll=async()=>null;
    export const readSourceRange=async()=>new Uint8Array();`,
  './writable-tree.js': 'export const openWritableTree = async () => ({});',
};
const bundle = await build({entryPoints:[source],bundle:true,write:false,platform:'node',format:'esm',
  define:{'import.meta.url':JSON.stringify(pathToFileURL(source).href + "?executionMode=strict")},
  plugins:[{name:'object-wait-resources',setup(plugin){
    plugin.onResolve({filter:/^\./},args=>Object.hasOwn(dependencies,args.path)
      ? {path:args.path,namespace:'object-wait-resource'} : undefined);
    plugin.onLoad({filter:/.*/,namespace:'object-wait-resource'},args=>({contents:dependencies[args.path],loader:'js',resolveDir:directory}));
  }}]});
const workerModule=`data:text/javascript;base64,${Buffer.from(bundle.outputFiles[0].contents).toString('base64')}`;
const empty=new Uint8Array(), main=8192, other=8193, entrySentinel=0x70000001, callbackSentinel=0x70000002;
const initialIp=0x401000, waitIp=0x401100, otherIp=0x402000;
const output=metadata=>({metadata,control:empty,payload:empty,presentation:empty,audio:empty,mediaBytes:empty});
function reflection(){const bytes=new Uint8Array(36),v=new DataView(bytes.buffer);bytes.set(new TextEncoder().encode('VINEREFL'));v.setUint32(8,4096,true);v.setBigUint64(12,1n,true);v.setUint32(24,16,true);return bytes;}
const scenarios=[
  {name:'signal-cancels-timer',signal:true},
  {name:'idle-timer-wakes'},
  {name:'early-timer-retries-same-id',retry:true},
  {name:'latched-signal-beats-queued-timeout',signalAfterTimer:true},
  {name:'parked-owner-resume-and-thread-exit-clock',parked:true},
  {name:'natural-thread-exit-completes-finite-join',parked:true,joinOnExit:true},
  {name:'no-object-wait-no-ready-query',sleep:true},
  {name:'unknown-ready-request',badReady:true,error:'Ready object wait has no pending browser request'},
  {name:'wrong-completion-owner',badOwner:true,error:'Browser completion disagrees with its guest thread'},
  {name:'retry-with-different-id',badRetry:true,error:'Browser timer retry disagrees with its pending request'},
];
const globals=['self','OffscreenCanvas','workerObjectWaitDiagnostics','setTimeout','clearTimeout','performance'];
const saved=Object.fromEntries(globals.map(key=>[key,Object.getOwnPropertyDescriptor(globalThis,key)]));
const nativeSetTimeout=globalThis.setTimeout,nativeClearTimeout=globalThis.clearTimeout;
let passed=0;
try {
  for(const scenario of scenarios){
    const events=[],timers=new Map(),timerDelays=[],acks=[],resumes=[],restores=[];
    let nextTimer=1,now=0,listener,resolveTerminal,ready=0n,hostPending=false,completed=false;
    let attempts=0,dispatches=0,executions=0,queries=0,retiredOther=false,runtimeOwner=main;
    let context={thread:main,instructionPointer:initialIp,eax:0,esp:0x9000};
    const parkedSnapshots = new Map();
    const cpuContext = value => {
      const words = new Uint32Array(128);
      words.set([value.thread, value.instructionPointer, value.eax, value.esp]);
      return {...value, snapshot:{backend:'recompiler', abiVersion:2, words}};
    };
    const terminal=new Promise(resolve=>{resolveTerminal=resolve;});
    const signal=scenario.signal||scenario.parked;
    const event=(name,detail)=>events.push({name,detail});
    globalThis.setTimeout=(callback,ms)=>{
      assert.equal(typeof callback,'function');assert(ms===5||ms===7);
      const id=nextTimer++;timers.set(id,callback);timerDelays.push(ms);event('arm',id);
      if(!signal&&!scenario.badReady)queueMicrotask(()=>{
        if(!timers.has(id))return;
        timers.delete(id);now+=ms;event('timer',id);
        if(scenario.signalAfterTimer)ready=1n;
        callback();
      });
      return id;
    };
    globalThis.clearTimeout=id=>{assert(timers.has(id),'Cancel only a live timer');timers.delete(id);event('cancel',id);};
    Object.defineProperty(globalThis,'performance',{configurable:true,value:{now:()=>now}});
    globalThis.self={addEventListener(type,callback){assert.equal(type,'message');listener=callback;},postMessage(message){if(message.type==='game-exit'||message.type==='game-error')resolveTerminal(message);}};
    globalThis.OffscreenCanvas=class OffscreenCanvas {};
    const resume=(value,kind)=>{resumes.push({thread:context.thread,value:{...value},kind});event('resume',context.thread);context={...context,instructionPointer:value.instructionPointer,esp:value.stackPointer,eax:value.eax??context.eax};};
    const completion={instructionPointer:entrySentinel,stackPointer:0x9000,eax:0};
    globalThis.workerObjectWaitDiagnostics={
      bridge:{
        inspectExecutable:()=>({sha256:'a'.repeat(64),guestImports:[]}),
        prepareExecutable:()=>({metadata:{maximumPhysicalPages:1,runtime:{threadId:main},entrySentinel,callbackSentinel},control:reflection(),payload:empty}),
        acknowledgeFast(value){assert.equal(value,1n);},
        storeCpuSnapshot(threadId, revision, snapshot){
          assert(!(threadId===other&&retiredOther), 'Retired thread cannot park a CPU');
          const previous=parkedSnapshots.get(threadId);
          assert.equal(revision,previous?.revision??0n,'Worker must use the Runtime snapshot revision');
          assert.equal(snapshot.words[0],threadId,'Snapshot must belong to the selected CPU');
          parkedSnapshots.set(threadId,{snapshot:{...snapshot,words:Uint32Array.from(snapshot.words)},revision:revision+1n});
          return revision+1n;
        },
        loadCpuSnapshot(threadId){
          const parked=parkedSnapshots.get(threadId);assert(parked,'Only Rust-owned parked snapshots can be restored');
          return {...parked,snapshot:{...parked.snapshot,words:Uint32Array.from(parked.snapshot.words)}};
        },
        synchronizeForwardClock(instructions,elapsed){assert(instructions>0);assert.equal(elapsed,now);event('clock',{instructions,elapsed,owner:runtimeOwner,retiredOther});},
        dispatch(){
          dispatches++;event('dispatch',context.thread);
          if(scenario.parked&&dispatches===1)return output({kind:'api-complete',createdThread:{threadId:other,entryPoint:otherIp,initialStackPointer:0xa000,fsBase:0xb000,returnSentinel:entrySentinel},completion:{instructionPointer:waitIp,stackPointer:0x9000,eax:0}});
          if(scenario.parked&&context.thread===other){if(!scenario.joinOnExit)ready=1n;return output({kind:'api-complete',completion:{instructionPointer:entrySentinel,stackPointer:0xa000,eax:0}});}
          assert.equal(context.thread,main);assert(!hostPending);hostPending=true;if(scenario.signal)ready=1n;
          return output({kind:'host-request',requestId:'1',threadId:main,service:scenario.sleep?'browser.sleep':'browser.object-wait',arguments:[5]});
        },
        readyObjectWaitRequestId(){queries++;assert(hostPending,'No getter after the last object completion');return scenario.badReady?99n:ready;},
        completeHost(requestId,acknowledgement){
          assert.equal(requestId,1n);assert(hostPending);assert.equal(events.at(-1).name,scenario.sleep?'host-completion-unlogged':'clock', 'Object completion must synchronize clock first');
          if(scenario.joinOnExit){assert(retiredOther,'Join may complete only after natural thread exit');assert.equal(runtimeOwner,other,'Clock sync must tolerate a retained exited owner while the waiter remains live');assert.equal(events.at(-1).detail.retiredOther,true);}
          attempts++;acks.push(acknowledgement);event('complete',{requestId,acknowledgement});
          if((scenario.retry||scenario.badRetry)&&attempts===1)return output({kind:'host-retry',threadId:main,requestId:scenario.badRetry?'2':'1',service:'browser.object-wait',arguments:[7]});
          if(scenario.badOwner)return output({kind:'host-complete',threadId:other,completion});
          completed=true;hostPending=false;ready=0n;runtimeOwner=main;
          return output({kind:'host-complete',threadId:main,completion});
        },
        scheduleNextThread(){
          if(scenario.parked&&!retiredOther){runtimeOwner=other;return output({kind:'thread-scheduled',threadId:other});}
          if(completed){runtimeOwner=main;return output({kind:'thread-scheduled',threadId:main});}
          event('idle');return output({kind:'scheduler-idle'});
        },
        finishThread(code){assert.equal(code,0);assert.equal(context.thread,other);assert.equal(events.at(-1).name,'clock','Thread exit must refresh clock before Rust signal');assert.equal(events.at(-1).detail.elapsed,now);retiredOther=true;parkedSnapshots.delete(other);event('exit-other');if(scenario.joinOnExit){assert(hostPending);assert.equal(attempts,0);ready=1n;now+=1;}return output({kind:'thread-exited'});},
        stopContext:instructionPointer=>({kind:'stop-context',threadId:runtimeOwner,instructionPointer,lastError:0,module:null,symbol:null}),
      },
      engine:{
        initialize(value){assert.equal(value.generation,1n);},
        saveCpuContext:()=>cpuContext(context),
        restoreCpuContext(value){const [thread,instructionPointer,eax,esp]=value.snapshot.words;restores.push(thread);event('restore',thread);context={thread,instructionPointer,eax,esp};},
        createCpuContext(created){return cpuContext({thread:created.threadId,instructionPointer:created.entryPoint,eax:0,esp:created.initialStackPointer});},
        executeBudget(budget){assert.equal(budget,131072);assert(++executions<10,'Bounded worker lifecycle');assert(!(context.thread===other&&retiredOther),'No guest instruction may execute in a retired context');if(scenario.parked&&context.thread===other&&context.instructionPointer===entrySentinel)now+=scenario.joinOnExit?1:11;
          return {stop:context.instructionPointer===entrySentinel?{kind:'trap',trap:'invalid-opcode'}:{kind:'runtime-dispatch'},context:{...context},instructions:1};},
        collectChangedPageViews:()=>[],readGuestU32:()=>0x403000,
        applyCpuResume:value=>resume(value,'host'),applyStoppedCpuResume:value=>resume(value,'api'),
      },
    };
    // Sleep is the existing non-object control and does not synchronize here.
    if(scenario.sleep){const complete=globalThis.workerObjectWaitDiagnostics.bridge.completeHost;globalThis.workerObjectWaitDiagnostics.bridge.completeHost=(id,ack)=>{event('host-completion-unlogged');return complete(id,ack);};}
    await import(`${workerModule}#${scenario.name}`);
    listener({data:{type:'launch-executable',canvas:new OffscreenCanvas(),root:{kind:'directory'},segments:[],name:'fixture.exe',sha256:'a'.repeat(64),storageFingerprint:'storage'}});
    let watchdog;const message=await Promise.race([terminal,new Promise((_,reject)=>{watchdog=nativeSetTimeout(()=>reject(Error('Worker did not finish '+scenario.name)),2000);})]).finally(()=>nativeClearTimeout(watchdog));
    assert.equal(timers.size,0,'Completed/failed worker releases every browser timer');
    if(scenario.error){assert.equal(message.type,'game-error');assert(message.detail.startsWith(scenario.error),message.detail);assert.equal(resumes.filter(x=>x.kind==='host').length,0);}
    else {assert.equal(message.type,'game-exit',message.detail);assert.equal(message.exitCode,0);assert.equal(resumes.filter(x=>x.kind==='host').length,1);}
    if(scenario.signal){assert.deepEqual(acks,[0]);assert.equal(events.filter(x=>x.name==='timer').length,0);assert.equal(events.filter(x=>x.name==='cancel').length,1);}
    if(scenario.retry){assert.deepEqual(acks,[1,1]);assert.deepEqual(timerDelays,[5,7]);assert.equal(events.filter(x=>x.name==='timer').length,2);}
    if(scenario.signalAfterTimer)assert.deepEqual(acks,[0],'Latched Rust winner overrides an already queued timeout');
    if(scenario.sleep){assert.equal(queries,0);assert.deepEqual(acks,[0]);}
    if(scenario.name==='idle-timer-wakes'){assert.deepEqual(acks,[1]);assert(events.some(x=>x.name==='idle'));}
    if(scenario.badReady){assert.equal(attempts,0);assert.equal(events.filter(x=>x.name==='cancel').length,1);}
    if(scenario.parked){assert.deepEqual(acks,[0]);assert(retiredOther);assert.deepEqual(restores,[other,main,other,main],'Parked owner resumes then active thread is restored before scheduling');assert.equal(resumes.find(x=>x.kind==='host').thread,main);}
    if(scenario.joinOnExit){const exit=events.findIndex(x=>x.name==='exit-other'),complete=events.findIndex(x=>x.name==='complete');assert(exit<complete);assert(events.slice(exit+1,complete).some(x=>x.name==='clock'&&x.detail.owner===other&&x.detail.retiredOther));assert.equal(events.filter(x=>x.name==='timer').length,0);assert.equal(events.filter(x=>x.name==='cancel').length,1);}
    passed++;
  }
}finally{for(const [key,descriptor]of Object.entries(saved)){if(descriptor===undefined)delete globalThis[key];else Object.defineProperty(globalThis,key,descriptor);}}
console.log(JSON.stringify({passed:true,scenarios:passed,workerSource:source,productionExportsAdded:false}));
