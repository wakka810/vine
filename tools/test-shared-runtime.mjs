import assert from "node:assert/strict";
import { readFile, mkdir } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import { build } from "esbuild";

// Real PE32 code, the production adapter, and the combined Rust module. No
// copies are injected into the CPU: its stores must be visible to Windows APIs.
async function main() {
  const bundle = await build({ stdin: { contents: `
    export { V86Mirror } from './browser/runtime-worker/v86-mirror.ts';
    export { RuntimeBridge } from './browser/runtime-worker/runtime-bridge.ts';
    export { decodeReflection, decodeHostDelta } from './browser/runtime-worker/memory-transport.ts';`,
    resolveDir: process.cwd(), loader: 'ts' }, bundle: true, write: false, platform: 'node', format: 'esm' });
  const { V86Mirror, RuntimeBridge, decodeReflection, decodeHostDelta } = await import(
    'data:text/javascript;base64,' + Buffer.from(bundle.outputFiles[0].contents).toString('base64'));
  const bytes = await readFile(process.env.VINE_SHARED_WASM ?? '.work/browser-runtime/runtime-cpu.wasm');
  const output = '.work/shared-runtime-test';
  await mkdir(output, {recursive:true});
  function run(command,args) {
    const result=spawnSync(command,args,{encoding:'utf8',windowsHide:true});
    assert.equal(result.status,0,result.stderr || result.error?.message);
  }
  run('llvm-dlltool',['-m','i386','-k','-d','fixtures/pe32/shared-memory.def','-l',`${output}/kernel32.lib`]);
  for(let fault=0;fault<=5;fault++) {
    run('clang',['--target=i686-pc-windows-msvc','-O1','-Wall','-Wextra','-Werror','-ffreestanding',
      '-fno-builtin','-fno-stack-protector',`-DVINE_FAULT_CASE=${fault}`,'-c','fixtures/pe32/shared-memory.c',
      '-o',`${output}/memory${fault}.obj`]);
    run('lld-link',['/nodefaultlib','/machine:x86','/subsystem:windows,6.0','/entry:fixture_entry',
      '/dynamicbase','/nxcompat','/timestamp:0','/section:.testcode,ERW','/section:.testro,R',
      `/out:${output}/memory${fault}.exe`,`${output}/memory${fault}.obj`,`${output}/kernel32.lib`]);
  }
  const fixtures=['bootstrap-fs','heap-growth',...Array.from({length:6},(_,i)=>`memory${i}`)];
  for (const fixture of fixtures) {
    const shared = await V86Mirror.instantiateSharedRuntime(bytes);
    const bridge = RuntimeBridge.fromExports(shared.exports);
    const prepared = bridge.prepareExecutable(await readFile(`${fixture.startsWith('memory') ? output : '.work/toolchain-spike'}/${fixture}.exe`), 'folder/test.exe');
    const arena = bridge.sharedGuestMemory();
    assert(arena && arena.guestBytes === 0x80000000);
    assert.equal(prepared.payload.length, 0, 'initial pages have no duplicate payload');
    assert.throws(() => decodeReflection(prepared.control, prepared.payload), /memory mode mismatch/);
    const reflection = decodeReflection(prepared.control, prepared.payload, arena);
    assert(reflection.pages.length > 0);
    const engine = V86Mirror.attachSharedRuntime(shared, {maximumPhysicalPages: prepared.metadata.maximumPhysicalPages});
    engine.initialize(reflection, prepared.metadata);
    const initial = engine.saveCpuContext();
    assert.throws(() => engine.executeBudget(1), /acknowledged|mapping generation/);
    assert.deepEqual(engine.saveCpuContext(), initial, 'unacknowledged CPU execution does not mutate state');
    bridge.acknowledgeFast(reflection.generation);
    let retired = 0, returned = false, dispatches = 0, deltas = 0, allocatedBase = 0;
    const fault = fixture.startsWith('memory') ? Number(fixture.at(-1)) : 0;
    function apply(output) {
      assert.equal(output.payload.length, 0, 'host changes carry metadata only');
      if (!output.control.length) return;
      const delta = decodeHostDelta(output.control, output.payload, arena);
      engine.applyMemoryDelta(delta);
      assert.throws(() => engine.executeBudget(1), /acknowledged|mapping generation/);
      bridge.acknowledgeFast(delta.generation);
      deltas++;
    }
    const start = performance.now();
    for (let i = 0; i < 10000; i++) {
      engine.setRuntimeClock(retired, performance.now() - start);
      const slice = engine.executeBudget(4096);
      retired += slice.instructions;
      assert.equal(engine.collectChangedPageViews().length, 0);
      assert.equal(shared.exports.vine_dirty_page_count(), 0, 'common stores do not create a writeback journal');
      if (slice.stop.kind === 'budget-yield') { await Promise.resolve(); continue; }
      if (slice.stop.kind === 'fault' && slice.stop.fault === 'page-fault') {
        if (fault) {
          const expected = fault === 4 ? 0xfffff000 : fault === 3 || fault === 5 ? allocatedBase + 4096 : allocatedBase;
          if(fault!==1) assert.equal(slice.stop.faultAddress, expected);
          const raw = new Uint8Array(arena.memory.buffer, arena.guestBase + allocatedBase,8192);
          assert.equal(raw[0],0x5a,'faulting CPU store did not mutate authoritative bytes');
          if(fault===1) assert.equal(new Uint8Array(arena.memory.buffer)[arena.guestBase + slice.stop.faultAddress],0x5a);
          if(fault===3 || fault===5) assert.deepEqual([...raw.slice(4094,4098)],[0x33,0x22,0x11,0x88]);
          if(fault===5) {
            const state=slice.context.snapshot.words;
            assert.equal(state[89],7,'faulting FSTP preserves TOP');
            assert.equal(state[88],0x7f,'faulting FSTP preserves the occupied stack slot');
            assert.deepEqual(Array.from(state.slice(118,121)),[0,0x80000000,0x3fff],'faulting FSTP preserves raw80 ST(0)');
          }
          returned=true; break;
        }
        apply(bridge.servicePageFault(slice.stop.faultAddress, slice.stop.errorCode));
        continue;
      }
      if (slice.context.instructionPointer === prepared.metadata.entrySentinel) {
        assert.equal(slice.context.eax, 0, `${fixture} guest result`);
        returned = true; break;
      }
      assert(slice.stop.kind === 'runtime-output' || slice.stop.kind === 'runtime-dispatch' ||
        (slice.stop.kind === 'trap' && slice.stop.trap === 'invalid-opcode'), JSON.stringify(slice.stop));
      let receipt;
      if(slice.stop.kind === 'runtime-output') receipt=bridge.readPendingDispatch();
      else {
        bridge.synchronizeForwardClock(retired, performance.now() - start);
        receipt = bridge.dispatch(slice.context.instructionPointer, slice.context.esp,
          engine.readGuestU32(slice.context.esp), retired, slice.context);
      }
      dispatches++;
      if(fixture.startsWith('memory') && !allocatedBase) allocatedBase=receipt.metadata.completion.eax;
      assert.equal(receipt.metadata.kind, 'api-complete');
      assert.equal(receipt.metadata.completion.executionBoundary, undefined);
      apply(receipt);
      if(dispatches===3) shared.exports.memory.grow(1);
      engine.applyStoppedCpuResume(receipt.metadata.completion);
    }
    assert(returned, 'guest exceeded its instruction limit');
    assert.equal(shared.exports.memory.buffer.byteLength > 0x80000000, true);
    const direct=shared.exports.vine_shared_direct_calls();
    if(fixture.startsWith('memory')) assert(direct>=2048,'synchronous Win32 APIs execute inside Wasm');
    console.log(`${fixture}: ${fault ? 'precise fault, original bytes intact' : 'returned 0'}; ${direct} native APIs, ${dispatches} host boundaries, ${deltas} metadata updates, no CPU writeback`);
  }
}
main().catch(error => { console.error(error.message); process.exitCode = 1; });
