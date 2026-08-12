import assert from "node:assert/strict";
import crypto from "node:crypto";

const adapterAbi = Object.freeze({ imports: 22, exports: 820 });
const adapterProfiles = Object.freeze([
  Object.freeze({
    id: "windows-gnu-rust-1.97.0-llvm-22.1.8",
    bytes: 1_864_286,
    sha256: "f44a6865fb1cd4128766b1dcccd4f2af73d59923e0b1b10316cdae5dd293b163",
    hasCanonicalReportFingerprints: true,
  }),
  Object.freeze({
    id: "linux-gnu-rust-1.97.0-llvm-22.1.8",
    bytes: 2_121_800,
    sha256: "8fea5e55f05bae2702edd8e8a92e724a936ef2b4e94ee22a8706d97def1030e1",
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
