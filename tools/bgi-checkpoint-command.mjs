import path from "node:path";

const SELECTION = /^[a-z0-9-]+$/u;

function requireSelection(value, label) {
  if (typeof value !== "string" || !SELECTION.test(value)) {
    throw new Error(`${label} must contain only lowercase letters, digits, and hyphens`);
  }
  return value;
}

export function bgiProfileOutputDirectory(profile) {
  requireSelection(profile, "BGI profile");
  if (profile === "baseline") {
    return "v86-bgi-generic-runtime-reentry";
  }
  if (
    !profile.startsWith("kernel32-") &&
    !profile.startsWith("user32-") &&
    !profile.startsWith("ole32-")
  ) {
    throw new Error(`BGI profile must be baseline or a Win32 capability: ${profile}`);
  }
  return `v86-bgi-${profile.replace(/^(?:kernel32|user32|ole32)-/u, "")}`;
}

export function planBgiCheckpointCommand(arguments_, manifest, root) {
  let profile = null;
  let checkpoint = null;
  for (let index = 0; index < arguments_.length; index += 1) {
    const argument = arguments_[index];
    if (argument === "--profile" && index + 1 < arguments_.length) {
      if (profile !== null) {
        throw new Error("--profile may be provided only once");
      }
      profile = requireSelection(arguments_[index + 1], "BGI profile");
      index += 1;
    } else if (argument === "--checkpoint" && index + 1 < arguments_.length) {
      if (checkpoint !== null) {
        throw new Error("--checkpoint may be provided only once");
      }
      checkpoint = requireSelection(arguments_[index + 1], "BGI checkpoint");
      index += 1;
    } else {
      throw new Error(`unsupported BGI checkpoint argument: ${argument}`);
    }
  }
  if (profile !== null && checkpoint !== null) {
    throw new Error("--profile and --checkpoint are mutually exclusive");
  }

  if (checkpoint !== null) {
    return {
      selection: { kind: "historical-checkpoint", checkpoint },
      outputDirectory: `v86-${checkpoint}`,
      runnerArguments: ["--checkpoint", checkpoint],
    };
  }

  const selectedProfile = profile ?? "active";
  const frozenCheckpoint = manifest.historicalBaseline.checkpoint;
  if (selectedProfile === "baseline") {
    return {
      selection: { kind: "historical-profile", profile: "baseline" },
      outputDirectory: bgiProfileOutputDirectory("baseline"),
      runnerArguments: [
        "--checkpoint",
        frozenCheckpoint,
        "--generic-runtime-reentry",
      ],
    };
  }

  const activeProfile = manifest.activeRuntimeCheckpoint.profile;
  const capability = selectedProfile === "active" ? activeProfile : selectedProfile;
  const runnerArguments = [
    "--checkpoint",
    frozenCheckpoint,
    "--generic-runtime-reentry",
    "--runtime-capability",
    capability,
  ];
  if (capability === activeProfile) {
    runnerArguments.push(
      "--runtime-policy",
      path.join(
        root,
        manifest.activeRuntimeCheckpoint.kernel32CompatibilityPolicy.source,
      ),
    );
  }
  return {
    selection: {
      kind: capability === activeProfile ? "active-profile" : "historical-profile",
      profile: capability,
    },
    outputDirectory: bgiProfileOutputDirectory(capability),
    runnerArguments,
  };
}
