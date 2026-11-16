// Link the selected, separately maintained CPU fork with the Rust Runtime.
// The generated linker crate and binaries stay under .work (or the requested
// external output). No source overlay is applied to the CPU checkout.
import {mkdir, writeFile, copyFile} from 'node:fs/promises';
import {spawnSync} from 'node:child_process';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
const cpu=path.resolve(root,process.env.VINE_V86_FAST_SOURCE??process.env.VINE_V86_SOURCE??'.work/vine-v86');
const link=path.resolve(root,'.work/shared-runtime-link');
const output=path.resolve(process.argv[2]??path.join(root,'.work/browser-runtime/runtime-cpu.wasm'));
await mkdir(link,{recursive:true});
const toml=p=>JSON.stringify(p.replaceAll('\\','/'));
await writeFile(path.join(link,'Cargo.toml'),`[package]
name = "vine-unified-runtime"
version = "0.0.0"
edition = "2024"
publish = false
[workspace]
[lib]
path = "lib.rs"
crate-type = ["cdylib"]
[dependencies]
v86 = { path = ${toml(cpu)} }
vine-runtime-wasm = { path = ${toml(path.join(root,'crates/vine-runtime-wasm'))}, features = ["shared-guest-memory"] }
[profile.release]
opt-level = 3
lto = true
codegen-units = 1
panic = "abort"
`);
await copyFile(path.join(root,'tools/shared-runtime-link.rs'),path.join(link,'lib.rs'));
const target=process.env.VINE_SHARED_TARGET??path.join(link,'target');
// Cargo also links dependency cdylibs. Apply the native objects and the low
// v86 state reservation to every Wasm link, not just the final crate.
const flags=['-C','link-args=--import-table --global-base=4096 --no-stack-first --max-memory=4294967296',
    '-C','link-arg='+path.join(cpu,'build/softfloat.o'),'-C','link-arg='+path.join(cpu,'build/zstddeclib.o')];
const env={...process.env,CARGO_TARGET_DIR:target,CARGO_ENCODED_RUSTFLAGS:flags.join('\x1f')};
function run(command,args){const r=spawnSync(command,args,{cwd:link,env,windowsHide:true,encoding:'utf8',maxBuffer:8e6});if(r.error)throw r.error;if(r.stdout)process.stdout.write(r.stdout);if(r.stderr)process.stderr.write(r.stderr);if(r.status!==0)throw Error(command+' failed: '+r.status);return r.stdout.trim();}
await copyFile(path.join(root,'Cargo.lock'),path.join(link,'Cargo.lock'));
run('cargo',['build','--release','--target','wasm32-unknown-unknown']);
await mkdir(path.dirname(output),{recursive:true});
await copyFile(path.join(target,'wasm32-unknown-unknown/release/vine_unified_runtime.wasm'),output);
console.log('Unified Runtime and CPU: '+output);
