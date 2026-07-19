export const SEMANTIC_CALL_DIGEST_ALGORITHM: "sha256-canonical-json-v1";

export function normalizeSemanticCall(call: any): unknown;

export class SemanticCallDigest {
  update(call: any): void;
  count(): number;
  digestHex(): string;
}
