import assert from "node:assert/strict";
import crypto from "node:crypto";

const adapterAbi = Object.freeze({ imports: 22, exports: 820 });
const adapterProfiles = Object.freeze([
  Object.freeze({
    id: "windows-gnu-rust-1.97.0-llvm-22.1.8",
    bytes: 1_864_286,
    sha256: "290f9c5fab90ced4ec3c2a019d189890e9aca45ba8139c9dd056558a0bc2b8d0",
    hasCanonicalReportFingerprints: true,
  }),
  Object.freeze({
    id: "linux-gnu-rust-1.97.0-llvm-22.1.8",
    bytes: 2_122_503,
    sha256: "a982eff1af67448e4d49cb1b0c4ff00c16a006517cb26287299ba185f8da6734",
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
      exports: adapterAbi.exports,
    },
  };
}
