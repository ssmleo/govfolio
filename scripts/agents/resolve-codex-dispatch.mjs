import { readFileSync } from "node:fs";
import { join, resolve } from "node:path";

import {
  findRepoRoot,
  formatDiagnostics,
  formatEnvelope,
  parseManifest,
  resolveDispatch,
  validateManifest,
} from "./codex-contract-lib.mjs";

function parseArguments(arguments_) {
  const options = {
    role: null,
    triggers: [],
    sectionFile: null,
    sectionHeading: null,
    sourceContext: null,
  };
  let repoRoot = null;
  const values = new Map([
    ["--repo-root", (value) => { repoRoot = value; }],
    ["--role", (value) => { options.role = value; }],
    ["--trigger", (value) => { options.triggers.push(value); }],
    ["--section-file", (value) => { options.sectionFile = value; }],
    ["--section-heading", (value) => { options.sectionHeading = value; }],
    ["--source-context", (value) => { options.sourceContext = value === "none" ? null : value; }],
  ]);
  for (let index = 0; index < arguments_.length; index += 1) {
    const apply = values.get(arguments_[index]);
    const value = arguments_[index + 1];
    if (!apply || value === undefined) throw new Error(`unknown or incomplete argument: ${arguments_[index]}`);
    apply(value);
    index += 1;
  }
  if (!options.role) throw new Error("--role is required");
  return { repoRoot: resolve(repoRoot ?? findRepoRoot(process.cwd())), options };
}

function fail(diagnostics) {
  process.stderr.write(`${formatDiagnostics(diagnostics)}\n`);
  process.exitCode = 1;
}

try {
  const { repoRoot, options } = parseArguments(process.argv.slice(2));
  process.chdir(repoRoot);
  const parsed = parseManifest(
    readFileSync(join(repoRoot, "agents", "skill-routing.json"), "utf8"),
    "agents/skill-routing.json",
  );
  const diagnostics = [...parsed.diagnostics];
  if (parsed.manifest) diagnostics.push(...validateManifest(parsed.manifest, repoRoot));
  if (diagnostics.length > 0) {
    fail(diagnostics);
  } else {
    const resolution = resolveDispatch(parsed.manifest, options);
    if (!resolution.envelope) fail(resolution.diagnostics);
    else process.stdout.write(`${formatEnvelope(resolution.envelope)}\n`);
  }
} catch (error) {
  fail([
    {
      code: "DISPATCH_RESOLUTION_FAILED",
      path: "agents/skill-routing.json",
      message: error.message,
      repair: "use declared role, trigger, and section identifiers",
    },
  ]);
}
