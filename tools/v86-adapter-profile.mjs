import assert from "node:assert/strict";
import crypto from "node:crypto";

const adapterAbi = Object.freeze({ imports: 22, exports: 809 });
const adapterProfiles = Object.freeze([
  Object.freeze({
    id: "windows-gnu-rust-1.97.0-llvm-22.1.8",
    bytes: 1_882_836,
    sha256: "f68a706655c81bde9b12982bd67cb00f19b1dae0e9ca5ddfaf94e0e74c4e7e26",
    hasCanonicalReportFingerprints: true,
  }),
  Object.freeze({
    id: "linux-gnu-rust-1.97.0-llvm-22.1.8",
    bytes: 2_141_050,
    sha256: "ac8349cbaccd31d6f5c68a84012fc7bc3a73eda2c06e519331396b43d8c42540",
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
