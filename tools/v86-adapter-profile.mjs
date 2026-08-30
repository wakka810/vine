import assert from "node:assert/strict";
import crypto from "node:crypto";

const adapterAbi = Object.freeze({ imports: 22 });
const adapterProfiles = Object.freeze([
  Object.freeze({
    id: "windows-gnu-rust-1.97.0-llvm-22.1.8",
    bytes: 1_864_286,
    sha256: "290f9c5fab90ced4ec3c2a019d189890e9aca45ba8139c9dd056558a0bc2b8d0",
    exports: 820,
    hasCanonicalReportFingerprints: true,
  }),
  Object.freeze({
    id: "linux-gnu-rust-1.97.0-llvm-22.1.8",
    bytes: 2_122_503,
    sha256: "a982eff1af67448e4d49cb1b0c4ff00c16a006517cb26287299ba185f8da6734",
    exports: 820,
    hasCanonicalReportFingerprints: false,
  }),
  Object.freeze({
    id: "windows-gnu-page-table-dirty-bypass-v13",
    bytes: 1_860_684,
    sha256: "d7faadb387c09913e0ecb266a01981864e4c6e330f56cd1df80adccc35feb661",
    exports: 824,
    hasCanonicalReportFingerprints: false,
  }),
  Object.freeze({
    id: "windows-gnu-packed-f32-fallback-v20",
    bytes: 1_872_295,
    sha256: "46855e6fb2bc7cfbb86e1ba94ac391c5a650af09375c4bdf657d5a5374cd3ba0",
    exports: 824,
    hasCanonicalReportFingerprints: false,
  }),
]);

export function resolveV86AdapterProfile(wasmBytes) {
  const sha256 = crypto.createHash("sha256").update(wasmBytes).digest("hex");
  const profile = adapterProfiles.find(
    (candidate) => candidate.bytes === wasmBytes.byteLength && candidate.sha256 === sha256,
  );
  assert(
    profile,
    `unsupported v86 adapter artifact: ${wasmBytes.byteLength} bytes, sha256 ${sha256}`,
  );
  return {
    id: profile.id,
    hasCanonicalReportFingerprints: profile.hasCanonicalReportFingerprints,
    wasm: {
      bytes: profile.bytes,
      sha256: profile.sha256,
      imports: adapterAbi.imports,
      exports: profile.exports,
    },
  };
}
