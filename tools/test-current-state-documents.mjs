import assert from "node:assert/strict";

import {
  CURRENT_REFERENCE_DOCUMENTS,
  CURRENT_REFERENCE_END,
  CURRENT_REFERENCE_START,
  FROZEN_HISTORY_END,
  FROZEN_HISTORY_START,
  renderCurrentCheckpointReference,
  validateCurrentStateDocuments,
} from "./current-state-documents.mjs";

function baselineDocuments() {
  return new Map(
    CURRENT_REFERENCE_DOCUMENTS.map((file) => [
      file,
      `# ${file}\n\n${renderCurrentCheckpointReference()}\n`,
    ]),
  );
}

function errorsFor(file, body) {
  const documents = baselineDocuments();
  documents.set(
    file,
    `${documents.get(file)}\n${body}\n`,
  );
  return validateCurrentStateDocuments(documents);
}

const currentCheckpoint = {
  activeRuntimeCheckpoint: {
    genericDispatcherApiCount: 47,
    genericDispatcherCallOccurrenceCount: 285,
    canonicalReproductionCommand: "npm run check:runtime:bgi-browser",
    latestEvidence: {
      finalMemoryGeneration: 396,
      reportSha256:
        "50d7ae673691a3036f328284f1e276404d8b7bec07256d16f793236f4bfbe1eb",
    },
  },
  nextStructuredStop: {
    api: "KERNEL32.dll!SetThreadAffinityMask",
  },
};

assert.deepEqual(validateCurrentStateDocuments(baselineDocuments()), []);

{
  const documents = baselineDocuments();
  documents.set("docs/requirements.md", "# requirements\n");
  assert(
    validateCurrentStateDocuments(documents).some((error) =>
      error.includes("exactly one current checkpoint reference is required"),
    ),
  );
}

{
  const documents = baselineDocuments();
  documents.set(
    "docs/requirements.md",
    documents
      .get("docs/requirements.md")
      .replace("ルートREADME", "README"),
  );
  assert(
    validateCurrentStateDocuments(documents).some((error) =>
      error.includes("current checkpoint reference block is stale"),
    ),
  );
}

assert(
  errorsFor(
    "docs/requirements.md",
    "The active runtime has 47 API contracts / 285 calls.",
  ).some((error) => error.includes("api-call-count-tuple")),
);

for (const [body, id] of [
  ["Captured 47 API contracts.", "exact-current-contract-count"],
  ["Captured Node/Chrome 285-call equivalence.", "exact-current-call-count"],
  ["The captured generation `396` was stable.", "exact-current-generation"],
  [
    "Artifact 50d7ae673691a3036f328284f1e276404d8b7bec07256d16f793236f4bfbe1eb.",
    "exact-current-report-identity",
  ],
  ["Run `npm run check:runtime:bgi-browser`.", "exact-current-canonical-command"],
  ["The consumer calls SetThreadAffinityMask.", "exact-current-next-stop"],
]) {
  const documents = baselineDocuments();
  documents.set(
    "docs/requirements.md",
    `${documents.get("docs/requirements.md")}\n${body}\n`,
  );
  assert(
    validateCurrentStateDocuments(documents, currentCheckpoint).some((error) =>
      error.includes(id),
    ),
  );
}

assert(
  errorsFor(
    "docs/requirements.md",
    "The old current runtime has 46 contracts / 284 calls.",
  ).some((error) => error.includes("api-call-count-tuple")),
);

assert(
  errorsFor(
    "docs/requirements.md",
    "The current next stop is `GetCurrentThread()`.",
  ).some((error) => error.includes("current-next-stop")),
);

assert(
  errorsFor(
    "docs/requirements.md",
    "The final memory generation is `396`.",
  ).some((error) => error.includes("current-generation")),
);

assert(
  errorsFor(
    "docs/requirements.md",
    "The active semantic report has SHA-256 `50d7`.",
  ).some((error) => error.includes("current-report-identity")),
);

assert(
  errorsFor(
    "docs/requirements.md",
    "The current canonical command is `npm run check:runtime:bgi-browser`.",
  ).some((error) => error.includes("current-canonical-command")),
);

assert.deepEqual(
  errorsFor(
    "docs/requirements.md",
    `${FROZEN_HISTORY_START}\nThe old current runtime had 46 contracts / 284 calls and its current next stop was \`GetCurrentThread()\`.\n${FROZEN_HISTORY_END}`,
  ),
  [],
);

assert(
  errorsFor(
    "docs/requirements.md",
    "The old current runtime had 46 contracts / 284 calls and its current next stop was `GetCurrentThread()`.",
  ).length > 0,
);

{
  const documents = baselineDocuments();
  documents.set(
    "docs/bgi-frozen-example.md",
    "# Frozen evidence\n\nThe current runtime had 46 contracts / 284 calls.\n",
  );
  assert.deepEqual(validateCurrentStateDocuments(documents), []);
}

{
  const documents = baselineDocuments();
  documents.set(
    "docs/requirements.md",
    `${documents.get("docs/requirements.md")}\n${FROZEN_HISTORY_START}\nunclosed\n`,
  );
  assert(
    validateCurrentStateDocuments(documents).some((error) =>
      error.includes("are unbalanced"),
    ),
  );
}

{
  const documents = baselineDocuments();
  documents.set(
    "docs/requirements.md",
    `${documents.get("docs/requirements.md")}\n${FROZEN_HISTORY_END}\nreversed\n${FROZEN_HISTORY_START}\n`,
  );
  assert(
    validateCurrentStateDocuments(documents).some((error) =>
      error.includes("must follow"),
    ),
  );
}

assert.equal(CURRENT_REFERENCE_START.includes("CURRENT CHECKPOINT"), true);
assert.equal(CURRENT_REFERENCE_END.includes("CURRENT CHECKPOINT"), true);

console.log("current-state document contract tests passed");
