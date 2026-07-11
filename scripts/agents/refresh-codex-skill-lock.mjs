import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

import {
  findRepoRoot,
  formatDiagnostics,
  hashSkillTree,
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

function fail(diagnostics) {
  process.stderr.write(`${formatDiagnostics(diagnostics)}\n`);
  process.exitCode = 1;
}

try {
  const repoRoot = parseArguments(process.argv.slice(2));
  const manifestPath = resolve(repoRoot, "agents", "skill-routing.json");
  const originalText = readFileSync(manifestPath, "utf8");
  const parsed = parseManifest(originalText, "agents/skill-routing.json");
  if (!parsed.manifest) {
    fail(parsed.diagnostics);
  } else {
    const tolerated = new Set(["SKILL_TREE_HASH_MISMATCH", "SKILL_FILE_COUNT_MISMATCH"]);
    const structuralDiagnostics = validateManifest(parsed.manifest, repoRoot).filter(
      ({ code }) => !tolerated.has(code),
    );
    if (structuralDiagnostics.length > 0) {
      fail(structuralDiagnostics);
    } else {
      const computed = [];
      const hashDiagnostics = [];
      for (const skill of parsed.manifest.skills) {
        if (skill.status !== "available") continue;
        const result = hashSkillTree(repoRoot, skill.source.canonical_path);
        hashDiagnostics.push(...result.diagnostics.map((item) => ({ ...item, skill: skill.id })));
        computed.push({ skill, result });
      }
      if (hashDiagnostics.length > 0) {
        fail(hashDiagnostics);
      } else {
        for (const { skill, result } of computed) {
          skill.source.tree_sha256 = result.tree_sha256;
          skill.source.file_count = result.file_count;
        }
        const refreshedText = `${JSON.stringify(parsed.manifest, null, 2)}\n`;
        if (refreshedText !== originalText) writeFileSync(manifestPath, refreshedText);
        process.stdout.write(`refreshed ${computed.length} governed skill locks\n`);
      }
    }
  }
} catch (error) {
  fail([
    {
      code: "LOCK_REFRESH_FAILED",
      message: error.message,
      path: "agents/skill-routing.json",
      repair: "fix the reported manifest or repository path and rerun lock refresh",
    },
  ]);
}
