import { createHash } from "node:crypto";
import {
  existsSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  realpathSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { dirname, isAbsolute, join, relative, resolve, sep } from "node:path";

import {
  GENERATED_MARKERS,
  findRepoRoot,
  formatDiagnostics,
  parseManifest,
  renderBridgeSkill,
  renderCodexAgent,
  renderOpenAiMetadata,
  renderRoleSkillBlock,
  validateManifest,
} from "./codex-contract-lib.mjs";

const LEGACY_ROLE_BLOCK_HASHES = new Map(Object.entries({
  auditor: "9e49096ce5acde8e7c05735d513c8e620b2b52a1b4656b1e45960e6874fea495",
  orchestrator: "5960aba856b5fd40d2de27e09b859c145b37bb00b4acc6e4ec5643c73e281549",
  planner: "3819b3639c49c8eb39aaa2d2263db4d74aab9d7a1b4e1c97ae42ec30648f7f75",
  "rust-builder": "675974b30234c0d8210dbc5b04e9beed0720abfe1da455f9992b478ac6ea63ee",
  sampler: "0ebec6ea650b4e952041ae7264f379ab66e15c692b3ad130c578cbe43174d9f1",
  scout: "9792fc656bae5c8dd88a2099b1e73c3a99449bc9d89f041c05cff2bb0a207468",
  sentinel: "f171d575e8041959b90bdcd801f787cbcef2aad37bfd1c113363a7ebf0503816",
  "spec-writer": "dc3a541ff72e6481a311c21df947a4c6c8cded0edd99a63e7d707213dece0df6",
  surveyor: "a1a12f14421a1cee90ba3246957daeb1d6c5bedb715733da2c66244b4520c7ed",
  "test-designer": "380bd10ce143571e69ef68d0fbf5787ed59d8eecc72ec0b57c9749e69af50482",
  "web-builder": "a2ebcb35fb516ac7e759edd3b896b50cc838b5c455652974a0b3c8f18767cbfb",
}));

function parseArguments(arguments_) {
  let mode = null;
  let repoRoot = null;
  for (let index = 0; index < arguments_.length; index += 1) {
    const argument = arguments_[index];
    if (argument === "--write" || argument === "--check") {
      if (mode) throw new Error("choose exactly one of --write or --check");
      mode = argument.slice(2);
    } else if (argument === "--repo-root" && arguments_[index + 1]) {
      repoRoot = arguments_[index + 1];
      index += 1;
    } else {
      throw new Error(`unknown or incomplete argument: ${argument}`);
    }
  }
  if (!mode) throw new Error("choose exactly one of --write or --check");
  return { mode, repoRoot: resolve(repoRoot ?? findRepoRoot(process.cwd())) };
}

function issue(code, path, message, repair = "run node scripts/agents/render-codex-contract.mjs --write") {
  return { code, path, message, repair };
}

function normalized(text) {
  return String(text).replace(/\r\n/g, "\n");
}

function isOwnedBridgeSkill(text) {
  const lines = normalized(text).split("\n");
  return lines[0] === "---" && lines[1] === GENERATED_MARKERS.comment;
}

function isOwnedCommentFile(text) {
  return normalized(text).split("\n")[0] === GENERATED_MARKERS.comment;
}

function isContained(root, candidate) {
  const child = relative(root, candidate);
  return child === "" || (!child.startsWith(`..${sep}`) && child !== ".." && !isAbsolute(child));
}

function unsafePath(path, message) {
  return issue(
    "PROJECTION_UNSAFE_PATH",
    path,
    message,
    "replace every generated-path symlink, junction, or special file with ordinary in-repository directories and files",
  );
}

function throwContractIssue(contractIssue) {
  const error = new Error(contractIssue.message);
  error.contractIssue = contractIssue;
  throw error;
}

function canonicalRepositoryRoot(path) {
  let stat;
  try {
    stat = lstatSync(path);
  } catch (error) {
    throwContractIssue(unsafePath(path, `cannot inspect repository root ${path}: ${error.message}`));
  }
  if (stat.isSymbolicLink() || !stat.isDirectory()) {
    throwContractIssue(unsafePath(path, `repository root must be an ordinary directory, not a symlink or special file: ${path}`));
  }
  try {
    return realpathSync(path);
  } catch (error) {
    throwContractIssue(unsafePath(path, `cannot canonicalize repository root ${path}: ${error.message}`));
  }
}

function inspectSafePath(repoRoot, path, expectedType = "either") {
  const absolute = resolve(path);
  if (!isContained(repoRoot, absolute)) {
    return unsafePath(path, `generated projection path escapes the real repository root: ${path}`);
  }

  const child = relative(repoRoot, absolute);
  const components = child === "" ? [] : child.split(sep);
  let current = repoRoot;
  for (let index = 0; index < components.length; index += 1) {
    current = join(current, components[index]);
    let stat;
    try {
      stat = lstatSync(current);
    } catch (error) {
      if (error?.code === "ENOENT") return null;
      return unsafePath(current, `cannot inspect generated projection path component ${current}: ${error.message}`);
    }

    if (stat.isSymbolicLink()) {
      return unsafePath(current, `generated projection path component is a symlink or junction: ${current}`);
    }
    const final = index === components.length - 1;
    if (!final && !stat.isDirectory()) {
      return unsafePath(current, `generated projection ancestor is not a directory: ${current}`);
    }
    if (final) {
      if (expectedType === "directory" && !stat.isDirectory()) {
        return unsafePath(current, `generated projection path must be an ordinary directory: ${current}`);
      }
      if (expectedType === "file" && !stat.isFile()) {
        return unsafePath(current, `generated projection path must be an ordinary file: ${current}`);
      }
      if (expectedType === "either" && !stat.isDirectory() && !stat.isFile()) {
        return unsafePath(current, `generated projection path is a special file: ${current}`);
      }
    }

    let canonical;
    try {
      canonical = realpathSync(current);
    } catch (error) {
      return unsafePath(current, `cannot canonicalize generated projection path component ${current}: ${error.message}`);
    }
    if (!isContained(repoRoot, canonical)) {
      return unsafePath(current, `generated projection path resolves outside the real repository root: ${current}`);
    }
  }
  return null;
}

function scanDirectory(repoRoot, directory, diagnostics) {
  const rootIssue = inspectSafePath(repoRoot, directory, "directory");
  if (rootIssue) {
    diagnostics.push(rootIssue);
    return;
  }
  if (!existsSync(directory)) return;

  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const child = join(directory, entry.name);
    const childIssue = inspectSafePath(
      repoRoot,
      child,
      entry.isDirectory() ? "directory" : "file",
    );
    if (childIssue) {
      diagnostics.push(childIssue);
      continue;
    }
    if (entry.isDirectory()) scanDirectory(repoRoot, child, diagnostics);
  }
}

function scanProjectionSafety(manifest, repoRoot) {
  const diagnostics = [];
  const bridgeRoot = join(repoRoot, ...manifest.codex.bridge_root.split("/"));
  const agentRoot = join(repoRoot, ...manifest.codex.agent_root.split("/"));
  const bridgeRootIssue = inspectSafePath(repoRoot, bridgeRoot, "directory");
  if (bridgeRootIssue) diagnostics.push(bridgeRootIssue);
  else if (existsSync(bridgeRoot)) {
    for (const entry of readdirSync(bridgeRoot, { withFileTypes: true })) {
      if (!entry.name.startsWith("govfolio-")) continue;
      scanDirectory(repoRoot, join(bridgeRoot, entry.name), diagnostics);
    }
  }
  const agentRootIssue = inspectSafePath(repoRoot, agentRoot, "directory");
  if (agentRootIssue) diagnostics.push(agentRootIssue);
  else if (existsSync(agentRoot)) {
    for (const entry of readdirSync(agentRoot, { withFileTypes: true })) {
      if (!entry.name.endsWith(".toml")) continue;
      const path = join(agentRoot, entry.name);
      const pathIssue = inspectSafePath(repoRoot, path, "file");
      if (pathIssue) diagnostics.push(pathIssue);
    }
  }

  for (const skill of manifest.skills.filter(({ status }) => status === "available")) {
    const bridgeDirectory = join(bridgeRoot, skill.codex_name);
    for (const path of [
      join(bridgeDirectory, "SKILL.md"),
      join(bridgeDirectory, "agents", "openai.yaml"),
    ]) {
      const pathIssue = inspectSafePath(repoRoot, path, "file");
      if (pathIssue) diagnostics.push(pathIssue);
    }
  }
  for (const role of manifest.roles) {
    for (const path of [
      join(agentRoot, `${role.id}.toml`),
      join(repoRoot, ...role.role_path.split("/")),
    ]) {
      const pathIssue = inspectSafePath(repoRoot, path, "file");
      if (pathIssue) diagnostics.push(pathIssue);
    }
  }
  return diagnostics;
}

function bridgeDirectoryOwnership(directory) {
  const files = bridgeDirectoryFiles(directory);
  if (files.length === 0) return false;
  return files.every((file) => {
    if (file.symlink) return false;
    const text = readFileSync(file.path, "utf8");
    if (file.relative === "SKILL.md") return isOwnedBridgeSkill(text);
    if (file.relative === "agents/openai.yaml") return isOwnedCommentFile(text);
    return false;
  });
}

function bridgeDirectoryFiles(directory) {
  const files = [];
  function visit(current, relative = "") {
    for (const entry of readdirSync(current, { withFileTypes: true })) {
      const child = join(current, entry.name);
      const childRelative = relative ? `${relative}/${entry.name}` : entry.name;
      if (entry.isDirectory()) visit(child, childRelative);
      else files.push({ path: child, relative: childRelative, symlink: entry.isSymbolicLink() });
    }
  }
  visit(directory);
  return files;
}

function roleProjection(sourceText, manifest, role) {
  const source = normalized(sourceText);
  const expectedBlock = renderRoleSkillBlock(manifest, role);
  const markedPattern = new RegExp(
    `${GENERATED_MARKERS.roleBegin.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}[\\s\\S]*?${GENERATED_MARKERS.roleEnd.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}`,
  );
  if (markedPattern.test(source)) return { text: source.replace(markedPattern, expectedBlock), diagnostics: [] };

  const legacyPattern = /^5 Skills\/Tools[^\n]*(?:\n[ \t]+ACTIVE:[^\n]*)?(?:\n[ \t]+SITUATIONAL:[^\n]*)?/m;
  const legacy = source.match(legacyPattern);
  if (legacy) {
    const actualHash = createHash("sha256").update(legacy[0]).digest("hex");
    if (actualHash !== LEGACY_ROLE_BLOCK_HASHES.get(role.id)) {
      return {
        text: null,
        diagnostics: [
          issue(
            "UNOWNED_ROLE_BLOCK_CONFLICT",
            role.role_path,
            `refusing to overwrite an unmarked Slot-5 block for ${role.id}`,
            "restore the reviewed legacy block or add the generated ownership markers deliberately",
          ),
        ],
      };
    }
    return { text: source.replace(legacyPattern, expectedBlock), diagnostics: [] };
  }

  const outputAnchor = /^6 Output format:/m;
  if (outputAnchor.test(source)) {
    return { text: source.replace(outputAnchor, `${expectedBlock}\n6 Output format:`), diagnostics: [] };
  }
  const suffix = source.endsWith("\n") ? "" : "\n";
  return { text: `${source}${suffix}${expectedBlock}\n`, diagnostics: [] };
}

function compareOrPlan(path, expected, mode, ownership, diagnostics, operations) {
  if (existsSync(path)) {
    const actual = readFileSync(path, "utf8");
    if (actual === expected) return;
    if (!ownership(actual)) {
      diagnostics.push(
        issue(
          "UNOWNED_OUTPUT_CONFLICT",
          path,
          `refusing to overwrite unmarked generated-path content at ${path}`,
          "move the custom file or explicitly add the correct generated marker",
        ),
      );
      return;
    }
    if (mode === "write") operations.push({ kind: "write", path, contents: expected });
    else diagnostics.push(issue("GENERATED_OUTPUT_DRIFT", path, `generated output differs: ${path}`));
    return;
  }
  if (mode === "write") {
    operations.push({ kind: "write", path, contents: expected });
  } else {
    diagnostics.push(issue("GENERATED_FILE_MISSING", path, `generated output is missing: ${path}`));
  }
}

function planProjection(manifest, repoRoot, mode) {
  const diagnostics = [];
  const operations = [];
  const expectedBridgeDirectories = new Set();
  const expectedAgentFiles = new Set();

  for (const skill of manifest.skills.filter(({ status }) => status === "available")) {
    const bridgeDirectory = join(repoRoot, ...manifest.codex.bridge_root.split("/"), skill.codex_name);
    expectedBridgeDirectories.add(resolve(bridgeDirectory));
    compareOrPlan(
      join(bridgeDirectory, "SKILL.md"),
      renderBridgeSkill(manifest, skill),
      mode,
      isOwnedBridgeSkill,
      diagnostics,
      operations,
    );
    compareOrPlan(
      join(bridgeDirectory, "agents", "openai.yaml"),
      renderOpenAiMetadata(skill),
      mode,
      isOwnedCommentFile,
      diagnostics,
      operations,
    );
    if (existsSync(bridgeDirectory)) {
      const allowed = new Set(["SKILL.md", "agents/openai.yaml"]);
      for (const file of bridgeDirectoryFiles(bridgeDirectory)) {
        if (allowed.has(file.relative)) continue;
        if (file.symlink) {
          diagnostics.push(
            issue(
              "UNOWNED_BRIDGE_EXTRA",
              file.path,
              `generated bridge contains an unowned symlink: ${file.path}`,
              "remove the unowned bridge entry",
            ),
          );
          continue;
        }
        const text = readFileSync(file.path, "utf8");
        if (isOwnedCommentFile(text) || isOwnedBridgeSkill(text)) {
          if (mode === "write") operations.push({ kind: "remove-file", path: file.path });
          else diagnostics.push(issue("STALE_MARKED_OUTPUT", file.path, `stale marked bridge output: ${file.path}`));
        } else {
          diagnostics.push(
            issue(
              "UNOWNED_BRIDGE_EXTRA",
              file.path,
              `generated bridge contains unowned extra content: ${file.path}`,
              "move the custom content outside the governed bridge directory",
            ),
          );
        }
      }
    }
  }

  for (const role of manifest.roles) {
    const agentPath = join(repoRoot, ...manifest.codex.agent_root.split("/"), `${role.id}.toml`);
    expectedAgentFiles.add(resolve(agentPath));
    compareOrPlan(
      agentPath,
      renderCodexAgent(manifest, role),
      mode,
      isOwnedCommentFile,
      diagnostics,
      operations,
    );

    const rolePath = join(repoRoot, ...role.role_path.split("/"));
    if (!existsSync(rolePath)) {
      diagnostics.push(issue("ROLE_FILE_MISSING", role.role_path, `role file is missing: ${role.role_path}`));
      continue;
    }
    const actual = readFileSync(rolePath, "utf8");
    const rendered = roleProjection(actual, manifest, role);
    diagnostics.push(...rendered.diagnostics);
    if (rendered.text === null || rendered.text === actual) continue;
    if (mode === "write") operations.push({ kind: "write", path: rolePath, contents: rendered.text });
    else diagnostics.push(issue("ROLE_BLOCK_DRIFT", role.role_path, `generated Slot-5 block differs for ${role.id}`));
  }

  const bridgeRoot = join(repoRoot, ...manifest.codex.bridge_root.split("/"));
  if (existsSync(bridgeRoot)) {
    for (const entry of readdirSync(bridgeRoot, { withFileTypes: true })) {
      if (!entry.isDirectory() || !entry.name.startsWith("govfolio-")) continue;
      const directory = resolve(bridgeRoot, entry.name);
      if (expectedBridgeDirectories.has(directory)) continue;
      if (!bridgeDirectoryOwnership(directory)) {
        diagnostics.push(
          issue(
            "UNOWNED_GENERATED_DIRECTORY",
            directory,
            `stale generated-name directory contains unmarked files: ${directory}`,
            "remove or relocate custom files before deleting the stale generated directory",
          ),
        );
      } else if (mode === "write") {
        operations.push({ kind: "remove-directory", path: directory });
      } else {
        diagnostics.push(issue("STALE_MARKED_OUTPUT", directory, `stale marked bridge: ${directory}`));
      }
    }
  }

  const agentRoot = join(repoRoot, ...manifest.codex.agent_root.split("/"));
  if (existsSync(agentRoot)) {
    for (const entry of readdirSync(agentRoot, { withFileTypes: true })) {
      if (!entry.isFile() || !entry.name.endsWith(".toml")) continue;
      const path = resolve(agentRoot, entry.name);
      if (expectedAgentFiles.has(path)) continue;
      const text = readFileSync(path, "utf8");
      if (!isOwnedCommentFile(text)) continue;
      if (mode === "write") operations.push({ kind: "remove-file", path });
      else diagnostics.push(issue("STALE_MARKED_OUTPUT", path, `stale marked Codex shim: ${path}`));
    }
  }
  return { diagnostics, operations };
}

function operationSafety(manifest, repoRoot, operations) {
  const diagnostics = scanProjectionSafety(manifest, repoRoot);
  for (const operation of operations) {
    const expectedType = operation.kind === "remove-directory" ? "directory" : "file";
    const pathIssue = inspectSafePath(repoRoot, operation.path, expectedType);
    if (pathIssue) diagnostics.push(pathIssue);
  }
  return diagnostics;
}

function applyOperations(operations) {
  for (const operation of operations) {
    if (operation.kind === "write") {
      mkdirSync(dirname(operation.path), { recursive: true });
      writeFileSync(operation.path, operation.contents);
    } else if (operation.kind === "remove-file") {
      rmSync(operation.path);
    } else if (operation.kind === "remove-directory") {
      rmSync(operation.path, { recursive: true, force: true });
    } else {
      throw new Error(`unknown projection operation: ${operation.kind}`);
    }
  }
}

function project(manifest, repoRoot, mode) {
  const safetyDiagnostics = scanProjectionSafety(manifest, repoRoot);
  if (safetyDiagnostics.length > 0) return safetyDiagnostics;

  const planned = planProjection(manifest, repoRoot, mode);
  if (planned.diagnostics.length > 0 || mode === "check") return planned.diagnostics;

  const finalSafetyDiagnostics = operationSafety(manifest, repoRoot, planned.operations);
  if (finalSafetyDiagnostics.length > 0) return finalSafetyDiagnostics;
  applyOperations(planned.operations);
  return [];
}

try {
  const arguments_ = parseArguments(process.argv.slice(2));
  const mode = arguments_.mode;
  const repoRoot = canonicalRepositoryRoot(arguments_.repoRoot);
  const manifestPath = join(repoRoot, "agents", "skill-routing.json");
  const manifestIssue = inspectSafePath(repoRoot, manifestPath, "file");
  if (manifestIssue) throwContractIssue(manifestIssue);
  const parsed = parseManifest(readFileSync(manifestPath, "utf8"), "agents/skill-routing.json");
  const diagnostics = [...parsed.diagnostics];
  if (parsed.manifest) diagnostics.push(...validateManifest(parsed.manifest, repoRoot));
  if (diagnostics.length === 0) diagnostics.push(...project(parsed.manifest, repoRoot, mode));
  if (diagnostics.length > 0) {
    process.stderr.write(`${formatDiagnostics(diagnostics)}\n`);
    process.exitCode = 1;
  } else {
    process.stdout.write(`${mode === "write" ? "rendered" : "verified"} Codex skill projection\n`);
  }
} catch (error) {
  process.stderr.write(
    `${formatDiagnostics([error.contractIssue ?? issue("PROJECTION_RENDER_FAILED", "agents/skill-routing.json", error.message)])}\n`,
  );
  process.exitCode = 1;
}
