export const CURRENT_REFERENCE_START =
  "<!-- BEGIN CURRENT CHECKPOINT REFERENCE -->";
export const CURRENT_REFERENCE_END =
  "<!-- END CURRENT CHECKPOINT REFERENCE -->";
export const FROZEN_HISTORY_START =
  "<!-- BEGIN FROZEN HISTORICAL EVIDENCE -->";
export const FROZEN_HISTORY_END =
  "<!-- END FROZEN HISTORICAL EVIDENCE -->";

export const CURRENT_REFERENCE_DOCUMENTS = [
  "docs/README.md",
  "docs/current-state-manifest.md",
  "docs/generic-api-dispatcher.md",
  "docs/requirements.md",
  "docs/target-architecture.md",
  "docs/testing-and-acceptance.md",
  "docs/windows-development.md",
];

const FROZEN_HISTORICAL_DOCUMENTS = [
  /^docs\/bgi-.*\.md$/u,
  /^docs\/v86-.*\.md$/u,
  /^docs\/w[23]-.*\.md$/u,
  /^docs\/windows-native-evidence\.md$/u,
];

export function renderCurrentCheckpointReference() {
  return `${CURRENT_REFERENCE_START}
> 変動するcurrent status（API contract数、call数、memory generation、next stop、
> report identity、canonical command）は
> [\`current-checkpoint.json\`](current-checkpoint.json)だけを正本とする。
> 最新値はルートREADMEのgenerated Current Statusを参照する。
${CURRENT_REFERENCE_END}`;
}

function occurrences(content, marker) {
  return content.split(marker).length - 1;
}

function removeDelimitedBlocks(content, start, end, relativeFile, errors) {
  const starts = occurrences(content, start);
  const ends = occurrences(content, end);
  if (starts !== ends) {
    errors.push(`${relativeFile}: ${start} and ${end} are unbalanced`);
    return content;
  }
  let stripped = content;
  while (stripped.includes(start)) {
    const startOffset = stripped.indexOf(start);
    const endOffset = stripped.indexOf(end, startOffset + start.length);
    if (endOffset < 0) {
      errors.push(`${relativeFile}: ${end} must follow ${start}`);
      break;
    }
    const nestedOffset = stripped.indexOf(start, startOffset + start.length);
    if (nestedOffset >= 0 && nestedOffset < endOffset) {
      errors.push(`${relativeFile}: nested ${start} blocks are forbidden`);
      break;
    }
    stripped = `${stripped.slice(0, startOffset)}${stripped.slice(
      endOffset + end.length,
    )}`;
  }
  return stripped;
}

function paragraphs(content) {
  return content
    .replace(/```[\s\S]*?```/gu, "")
    .split(/\r?\n\s*\r?\n/gu)
    .flatMap((paragraph) => {
      const lines = paragraph.split(/\r?\n/gu);
      return lines.some((line) => line.trimStart().startsWith("|"))
        ? lines
        : [paragraph];
    })
    .map((paragraph) => paragraph.replace(/\s+/gu, " ").trim())
    .filter(Boolean);
}

function lineFor(content, fragment) {
  const offset = content.indexOf(fragment);
  return offset < 0 ? 1 : content.slice(0, offset).split("\n").length;
}

const CURRENT_VALUE_PATTERNS = [
  {
    id: "api-call-count-tuple",
    pattern:
      /(?:\b\d[\d,]*\s+(?:API\s+)?contracts?\s*[／/]\s*\d[\d,]*\s+calls?\b|\b\d[\d,]*\s+calls?\s+(?:across|over)\s+\d[\d,]*\s+(?:API\s+)?contracts?\b)/iu,
  },
  {
    id: "current-contract-count",
    pattern:
      /(?:(?:current|active)\s+(?:BGI\s+)?(?:generic\s+)?(?:runtime|checkpoint|profile|session|workload|state|dispatcher)|(?:現在|現行)(?:の)?(?:runtime|checkpoint|profile|session|workload|state|dispatcher)).{0,120}\b\d[\d,]*\s+(?:API\s+)?contracts?\b/iu,
  },
  {
    id: "current-call-count",
    pattern:
      /(?:(?:current|active)\s+(?:BGI\s+)?(?:generic\s+)?(?:runtime|checkpoint|profile|session|workload|state|dispatcher)|(?:現在|現行)(?:の)?(?:runtime|checkpoint|profile|session|workload|state|dispatcher)).{0,120}(?:\b\d[\d,]*-call\b|\b\d[\d,]*\s+calls?\b)/iu,
  },
  {
    id: "current-generation",
    pattern:
      /(?:(?:current|active|final)\s+(?:memory\s+)?generation|(?:現在|現行)(?:の)?(?:memory\s+)?generation)(?:\s+(?:is|was))?\s*[:=]?\s*`?\d[\d,]*`?/iu,
  },
  {
    id: "current-report-identity",
    pattern:
      /(?:(?:current|active)\s+(?:semantic\s+)?(?:[\w-]+\s+){0,3}report|(?:現在|現行)(?:の)?report).{0,180}(?:SHA-?256|\bbytes\b)/iu,
  },
  {
    id: "current-next-stop",
    pattern:
      /(?:(?:current|active|next)\s+(?:structured\s+)?(?:stop|checkpoint|proof)|(?:現在|現行|次(?:の)?).{0,30}(?:stop|checkpoint|proof|境界)).{0,180}(?:`?[A-Z][A-Za-z0-9_]+(?:A|W)?`?\s*(?:\(|境界))/iu,
  },
  {
    id: "current-canonical-command",
    pattern:
      /(?:(?:current|active)\s+canonical\s+(?:reproduction\s+)?command|(?:現在|現行)(?:の)?canonical\s+command).{0,120}(?:npm|node|cargo)\s+run\b/iu,
  },
];

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&");
}

function exactCurrentValuePatterns(checkpoint) {
  const active = checkpoint?.activeRuntimeCheckpoint;
  const evidence = active?.latestEvidence;
  const nextStop = checkpoint?.nextStructuredStop;
  if (active === undefined || evidence === undefined || nextStop === undefined) {
    return [];
  }

  const patterns = [];
  if (Number.isSafeInteger(active.genericDispatcherApiCount)) {
    patterns.push({
      id: "exact-current-contract-count",
      pattern: new RegExp(
        `\\b${active.genericDispatcherApiCount}\\s+(?:API\\s+)?contracts?\\b`,
        "iu",
      ),
    });
  }
  if (Number.isSafeInteger(active.genericDispatcherCallOccurrenceCount)) {
    patterns.push({
      id: "exact-current-call-count",
      pattern: new RegExp(
        `\\b${active.genericDispatcherCallOccurrenceCount}(?:-call\\b|\\s+calls?\\b)`,
        "iu",
      ),
    });
  }
  if (Number.isSafeInteger(evidence.finalMemoryGeneration)) {
    patterns.push({
      id: "exact-current-generation",
      pattern: new RegExp(
        `\\bgeneration\\s*\`?${evidence.finalMemoryGeneration}\`?\\b`,
        "iu",
      ),
    });
  }
  if (typeof evidence.reportSha256 === "string") {
    patterns.push({
      id: "exact-current-report-identity",
      pattern: new RegExp(`\\b${escapeRegExp(evidence.reportSha256)}\\b`, "iu"),
    });
  }
  if (typeof active.canonicalReproductionCommand === "string") {
    patterns.push({
      id: "exact-current-canonical-command",
      pattern: new RegExp(
        escapeRegExp(active.canonicalReproductionCommand),
        "iu",
      ),
    });
  }
  if (typeof nextStop.api === "string") {
    patterns.push({
      id: "exact-current-next-stop",
      pattern: new RegExp(`\\b${escapeRegExp(nextStop.api)}\\b`, "iu"),
    });
  }
  return patterns;
}

export function validateCurrentStateDocuments(documents, checkpoint) {
  const errors = [];
  const expectedReference = renderCurrentCheckpointReference();
  const currentReferenceDocuments = new Set(CURRENT_REFERENCE_DOCUMENTS);
  const currentValuePatterns = [
    ...CURRENT_VALUE_PATTERNS,
    ...exactCurrentValuePatterns(checkpoint),
  ];

  for (const relativeFile of CURRENT_REFERENCE_DOCUMENTS) {
    const content = documents.get(relativeFile);
    if (typeof content !== "string") {
      errors.push(`${relativeFile}: current-state reference document is missing`);
      continue;
    }
    if (occurrences(content, CURRENT_REFERENCE_START) !== 1) {
      errors.push(`${relativeFile}: exactly one current checkpoint reference is required`);
    } else {
      const start = content.indexOf(CURRENT_REFERENCE_START);
      const end = content.indexOf(CURRENT_REFERENCE_END, start);
      const actual = content.slice(start, end + CURRENT_REFERENCE_END.length);
      if (actual !== expectedReference) {
        errors.push(`${relativeFile}: current checkpoint reference block is stale`);
      }
    }
  }

  for (const [relativeFile, content] of documents) {
    if (FROZEN_HISTORICAL_DOCUMENTS.some((pattern) => pattern.test(relativeFile))) {
      continue;
    }
    if (!currentReferenceDocuments.has(relativeFile)) {
      continue;
    }
    let currentContent = removeDelimitedBlocks(
      content,
      CURRENT_REFERENCE_START,
      CURRENT_REFERENCE_END,
      relativeFile,
      errors,
    );
    currentContent = removeDelimitedBlocks(
      currentContent,
      FROZEN_HISTORY_START,
      FROZEN_HISTORY_END,
      relativeFile,
      errors,
    );
    for (const paragraph of paragraphs(currentContent)) {
      for (const { id, pattern } of currentValuePatterns) {
        if (pattern.test(paragraph)) {
          errors.push(
            `${relativeFile}:${lineFor(content, paragraph.slice(0, 40))}: ${id} must come from docs/current-checkpoint.json or be inside a frozen historical block`,
          );
        }
      }
    }
  }
  return errors;
}
