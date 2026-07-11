import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import {
  findRepoRoot,
  formatDiagnostics,
  parseManifest,
  validateManifest,
} from "./codex-contract-lib.mjs";

function parseArguments(arguments_) {
  let repoRoot = null;
  for (let index = 0; index < arguments_.length; index += 1) {
    if (arguments_[index] === "--repo-root" && arguments_[index + 1]) {
      repoRoot = arguments_[index + 1];
      index += 1;
    } else {
      throw new Error(`unknown or incomplete argument: ${arguments_[index]}`);
    }
  }
  return resolve(repoRoot ?? findRepoRoot(process.cwd()));
}

function sortedRoleNames(directory, suffix, include) {
  if (!existsSync(directory)) return [];
  return readdirSync(directory, { withFileTypes: true })
    .filter((entry) => entry.isFile() && entry.name.endsWith(suffix) && include(entry))
    .map((entry) => entry.name.slice(0, -suffix.length))
    .sort();
}

function roleSetDiagnostics(manifest, repoRoot) {
  const expected = manifest.roles.map(({ id }) => id).sort();
  const sets = [
    {
      path: "agents/roles",
      actual: sortedRoleNames(join(repoRoot, "agents", "roles"), ".md", (entry) => !entry.name.startsWith("_")),
    },
    {
      path: ".claude/agents",
      actual: sortedRoleNames(join(repoRoot, ".claude", "agents"), ".md", () => true),
    },
    {
      path: ".codex/agents",
      actual: sortedRoleNames(join(repoRoot, ".codex", "agents"), ".toml", (entry) =>
        readFileSync(join(repoRoot, ".codex", "agents", entry.name), "utf8")
          .replace(/\r\n/g, "\n")
          .startsWith("# GENERATED: govfolio-codex-skill-contract; DO NOT EDIT\n")),
    },
  ];
  return sets
    .filter(({ actual }) => JSON.stringify(actual) !== JSON.stringify(expected))
    .map(({ path, actual }) => ({
      code: "ROLE_SET_MISMATCH",
      message: `${path} does not match the governed manifest role set`,
      path,
      role: null,
      skill: null,
      expected,
      actual,
      repair: "repair agents/skill-routing.json role parity and rerun the renderer",
    }));
}

try {
  const repoRoot = parseArguments(process.argv.slice(2));
  const manifestPath = join(repoRoot, "agents", "skill-routing.json");
  const parsed = parseManifest(readFileSync(manifestPath, "utf8"), "agents/skill-routing.json");
  const diagnostics = [...parsed.diagnostics];
  if (parsed.manifest) {
    diagnostics.push(...validateManifest(parsed.manifest, repoRoot));
    diagnostics.push(...roleSetDiagnostics(parsed.manifest, repoRoot));
  }
  if (diagnostics.length > 0) {
    process.stderr.write(`${formatDiagnostics(diagnostics)}\n`);
    process.exitCode = 1;
  } else {
    const renderPath = join(dirname(fileURLToPath(import.meta.url)), "render-codex-contract.mjs");
    const projection = spawnSync(
      process.execPath,
      [renderPath, "--check", "--repo-root", repoRoot],
      { cwd: repoRoot, encoding: "utf8" },
    );
    if (projection.status !== 0) {
      process.stderr.write(projection.stderr || projection.stdout || "projection validation failed\n");
      process.exitCode = 1;
    } else {
      process.stdout.write("validated Codex skill contract\n");
    }
  }
} catch (error) {
  process.stderr.write(
    `${formatDiagnostics([{ code: "CONTRACT_VALIDATION_FAILED", path: "agents/skill-routing.json", message: error.message, repair: "repair the contract and rerun validation" }])}\n`,
  );
  process.exitCode = 1;
}
