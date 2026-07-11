#!/usr/bin/env node

import { execFile } from "node:child_process";
import { mkdtemp, readFile, realpath, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, isAbsolute, join, relative, resolve } from "node:path";
import { promisify } from "node:util";
import { fileURLToPath } from "node:url";

const execFileAsync = promisify(execFile);
const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const defaultRepositoryRoot = resolve(scriptDirectory, "..", "..");

function parseArguments(argv) {
  let repositoryRoot = defaultRepositoryRoot;
  for (let index = 0; index < argv.length; index += 1) {
    if (argv[index] === "--repo-root" && argv[index + 1]) {
      repositoryRoot = resolve(argv[index + 1]);
      index += 1;
      continue;
    }
    throw new Error(`usage: ${process.argv[1]} [--repo-root PATH]`);
  }
  return repositoryRoot;
}

async function run(file, args, cwd, { allowFailure = false } = {}) {
  try {
    return await execFileAsync(file, args, {
      cwd,
      encoding: "utf8",
      maxBuffer: 16 * 1024 * 1024,
      windowsHide: true,
    });
  } catch (error) {
    if (allowFailure) return error;
    const detail = [error.stdout, error.stderr].filter(Boolean).join("\n").trim();
    throw new Error(`${file} ${args.join(" ")} failed${detail ? `:\n${detail}` : ""}`);
  }
}

async function git(args, cwd, options) {
  return run("git", args, cwd, options);
}

async function requireClean(repositoryRoot, label) {
  const { stdout } = await git(
    ["status", "--porcelain", "--untracked-files=all"],
    repositoryRoot,
  );
  if (stdout !== "") {
    throw new Error(`${label} must be clean before contract verification:\n${stdout}`);
  }
}

async function requireTracked(repositoryRoot, paths) {
  for (const path of paths) {
    await git(["ls-files", "--error-unmatch", "--", path], repositoryRoot);
  }
}

async function requireExecutableRunner(repositoryRoot) {
  const { stdout } = await git(
    ["ls-files", "--stage", "--", "agents/run-loop-codex.sh"],
    repositoryRoot,
  );
  if (!stdout.startsWith("100755 ")) {
    throw new Error(
      "agents/run-loop-codex.sh must be tracked executable (stage with git add --chmod=+x)",
    );
  }
}

async function forbidTrackedMachineConfig(repositoryRoot) {
  const result = await git(
    ["ls-files", "--error-unmatch", "--", ".codex/config.toml"],
    repositoryRoot,
    { allowFailure: true },
  );
  if (result.exitCode === undefined && result.code === undefined) {
    throw new Error(".codex/config.toml is machine-specific and must never be tracked");
  }
}

async function verifiedCleanup(sourceRoot, container, worktree, worktreeAdded) {
  const containerReal = await realpath(container);
  const relativeWorktree = relative(containerReal, worktree);
  if (
    !relativeWorktree ||
    relativeWorktree.startsWith("..") ||
    isAbsolute(relativeWorktree)
  ) {
    throw new Error(`refusing unverified temporary cleanup target: ${worktree}`);
  }

  if (worktreeAdded) {
    const { stdout: listed } = await git(["worktree", "list", "--porcelain"], sourceRoot);
    const normalizedWorktree = worktree.replaceAll("\\", "/");
    if (!listed.replaceAll("\\", "/").includes(`worktree ${normalizedWorktree}\n`)) {
      throw new Error(`temporary worktree is not registered for cleanup: ${worktree}`);
    }
    const { stdout: topLevel } = await git(["rev-parse", "--show-toplevel"], worktree);
    if (resolve(topLevel.trim()) !== resolve(worktree)) {
      throw new Error(`temporary worktree identity mismatch: ${topLevel.trim()}`);
    }
    await git(["worktree", "remove", "--force", "--", worktree], sourceRoot);
  }
  await rm(containerReal, { recursive: true, force: true });
}

async function main() {
  const requestedRoot = parseArguments(process.argv.slice(2));
  const repositoryRoot = await realpath(requestedRoot);
  const { stdout: topLevel } = await git(["rev-parse", "--show-toplevel"], repositoryRoot);
  if (resolve(topLevel.trim()) !== resolve(repositoryRoot)) {
    throw new Error(`--repo-root must name a Git worktree root: ${requestedRoot}`);
  }

  await requireClean(repositoryRoot, "source worktree");
  await forbidTrackedMachineConfig(repositoryRoot);

  const container = await mkdtemp(join(tmpdir(), "govfolio-codex-contract-"));
  const worktree = join(container, "worktree");
  let worktreeAdded = false;
  try {
    await git(["worktree", "add", "--detach", "--", worktree, "HEAD"], repositoryRoot);
    worktreeAdded = true;

    await run(
      process.execPath,
      [join(worktree, "scripts/agents/render-codex-contract.mjs"), "--check", "--repo-root", worktree],
      worktree,
    );
    await run(
      process.execPath,
      [join(worktree, "scripts/agents/validate-codex-contract.mjs"), "--repo-root", worktree],
      worktree,
    );
    await requireClean(worktree, "detached verification worktree");

    const manifest = JSON.parse(
      await readFile(join(worktree, "agents/skill-routing.json"), "utf8"),
    );
    const tracked = [
      "AGENTS.md",
      "agents/run-loop-codex.sh",
      "agents/skill-routing.json",
    ];
    for (const skill of manifest.skills ?? []) {
      if (skill.status !== "available") continue;
      const root = `${manifest.codex.bridge_root}/${skill.codex_name}`;
      tracked.push(`${root}/SKILL.md`, `${root}/agents/openai.yaml`);
    }
    for (const role of manifest.roles ?? []) {
      tracked.push(`${manifest.codex.agent_root}/${role.id}.toml`);
    }
    await requireTracked(worktree, tracked);
    await requireExecutableRunner(worktree);
    await forbidTrackedMachineConfig(worktree);
    await requireClean(worktree, "detached verification worktree");
  } finally {
    await verifiedCleanup(repositoryRoot, container, worktree, worktreeAdded);
  }

  process.stdout.write("Codex contract clean detached worktree passed.\n");
}

main().catch((error) => {
  process.stderr.write(`BLOCKED(skill-contract): ${error.message}\n`);
  process.exitCode = 1;
});
