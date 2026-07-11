import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  existsSync,
  lstatSync,
  readFileSync,
  readdirSync,
  realpathSync,
} from "node:fs";
import {
  dirname,
  isAbsolute,
  join,
  relative,
  resolve,
  sep,
} from "node:path";

const TOP_LEVEL_KEYS = [
  "schema_version",
  "hash_algorithm",
  "codex",
  "dispatch",
  "skills",
  "packs",
  "triggers",
  "roles",
];

const MARKERS = Object.freeze({
  markdown: "<!-- GENERATED: govfolio-codex-skill-contract; DO NOT EDIT -->",
  comment: "# GENERATED: govfolio-codex-skill-contract; DO NOT EDIT",
  roleBegin: "<!-- BEGIN GENERATED GOVFOLIO SKILL CONTRACT -->",
  roleEnd: "<!-- END GENERATED GOVFOLIO SKILL CONTRACT -->",
});

function byteCompare(left, right) {
  return Buffer.compare(Buffer.from(String(left), "utf8"), Buffer.from(String(right), "utf8"));
}

function sortDiagnostics(diagnostics) {
  return [...diagnostics].sort((left, right) => {
    for (const key of ["code", "path", "role", "skill"]) {
      const compared = byteCompare(left[key] ?? "", right[key] ?? "");
      if (compared !== 0) return compared;
    }
    return byteCompare(left.message, right.message);
  });
}

function diagnostic(code, message, values = {}) {
  return {
    code,
    message,
    path: values.path ?? null,
    role: values.role ?? null,
    skill: values.skill ?? null,
    expected: values.expected ?? null,
    actual: values.actual ?? null,
    repair:
      values.repair ??
      "edit agents/skill-routing.json and rerun the contract checks",
  };
}

function stableValue(value) {
  if (Array.isArray(value)) return value.map(stableValue);
  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.keys(value)
        .sort(byteCompare)
        .map((key) => [key, stableValue(value[key])]),
    );
  }
  return value;
}

function stableStringify(value) {
  return JSON.stringify(stableValue(value));
}

function normalizedRelativePath(path) {
  return path.split(sep).join("/");
}

function safeRepositoryPath(repoRoot, repositoryPath) {
  if (
    typeof repositoryPath !== "string" ||
    repositoryPath.length === 0 ||
    isAbsolute(repositoryPath) ||
    repositoryPath.includes("\\")
  ) {
    return null;
  }
  const absolutePath = resolve(repoRoot, repositoryPath);
  const fromRoot = relative(resolve(repoRoot), absolutePath);
  if (fromRoot === "" || fromRoot === ".." || fromRoot.startsWith(`..${sep}`) || isAbsolute(fromRoot)) {
    return null;
  }
  return { absolutePath, repositoryPath: normalizedRelativePath(fromRoot) };
}

function frame(buffer) {
  const length = Buffer.alloc(4);
  length.writeUInt32BE(buffer.length);
  return length;
}

function collectFiles(root, current, files, diagnostics, canonicalDirectory) {
  const entries = readdirSync(current, { withFileTypes: true });
  for (const entry of entries) {
    const absolutePath = join(current, entry.name);
    const repositoryPath = normalizedRelativePath(relative(root, absolutePath));
    const stat = lstatSync(absolutePath);
    if (stat.isSymbolicLink()) {
      diagnostics.push(
        diagnostic(
          "CANONICAL_SYMLINK_FORBIDDEN",
          `canonical skill tree contains a symlink: ${repositoryPath}`,
          {
            path: repositoryPath,
            actual: repositoryPath,
            repair: `replace the symlink under ${canonicalDirectory} with regular tracked files`,
          },
        ),
      );
      continue;
    }
    if (stat.isDirectory()) {
      collectFiles(root, absolutePath, files, diagnostics, canonicalDirectory);
    } else if (stat.isFile()) {
      files.push({ absolutePath, repositoryPath });
    } else {
      diagnostics.push(
        diagnostic(
          "CANONICAL_SPECIAL_FILE_FORBIDDEN",
          `canonical skill tree contains a non-regular file: ${repositoryPath}`,
          { path: repositoryPath, actual: repositoryPath },
        ),
      );
    }
  }
}

function duplicates(items, valueOf) {
  const seen = new Set();
  const repeated = new Set();
  for (const item of items) {
    const value = valueOf(item);
    if (seen.has(value)) repeated.add(value);
    seen.add(value);
  }
  return [...repeated].sort(byteCompare);
}

function requireExactKeys(value, expectedKeys, path, diagnostics) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    diagnostics.push(
      diagnostic("INVALID_MANIFEST_SHAPE", `${path} must be an object`, {
        path,
        expected: "object",
        actual: Array.isArray(value) ? "array" : typeof value,
      }),
    );
    return;
  }
  const actualKeys = Object.keys(value).sort(byteCompare);
  const expected = [...expectedKeys].sort(byteCompare);
  if (stableStringify(actualKeys) !== stableStringify(expected)) {
    diagnostics.push(
      diagnostic("INVALID_MANIFEST_KEYS", `${path} has unexpected or missing keys`, {
        path,
        expected,
        actual: actualKeys,
      }),
    );
  }
}

function asArray(value) {
  return Array.isArray(value) ? value : [];
}

export function findRepoRoot(startDirectory) {
  let current = resolve(startDirectory);
  while (true) {
    if (existsSync(join(current, ".git"))) return current;
    const parent = dirname(current);
    if (parent === current) {
      throw new Error(`no Git repository found above ${startDirectory}`);
    }
    current = parent;
  }
}

export function parseManifest(text, sourcePath) {
  try {
    const manifest = JSON.parse(text);
    if (!manifest || typeof manifest !== "object" || Array.isArray(manifest)) {
      return {
        manifest: null,
        diagnostics: [
          diagnostic("MANIFEST_NOT_OBJECT", "skill routing manifest must be a JSON object", {
            path: sourcePath,
            expected: "object",
            actual: Array.isArray(manifest) ? "array" : typeof manifest,
          }),
        ],
      };
    }
    return { manifest, diagnostics: [] };
  } catch (error) {
    return {
      manifest: null,
      diagnostics: [
        diagnostic("MANIFEST_JSON_INVALID", `cannot parse ${sourcePath}: ${error.message}`, {
          path: sourcePath,
          actual: error.message,
          repair: `repair JSON syntax in ${sourcePath}`,
        }),
      ],
    };
  }
}

export function validateManifest(manifest, repoRoot) {
  const diagnostics = [];
  requireExactKeys(manifest, TOP_LEVEL_KEYS, "agents/skill-routing.json", diagnostics);
  if (!manifest || typeof manifest !== "object" || Array.isArray(manifest)) {
    return sortDiagnostics(diagnostics);
  }

  const constants = [
    ["schema_version", manifest.schema_version, 1],
    ["hash_algorithm", manifest.hash_algorithm, "sha256-git-blobs-v1"],
    ["codex.bridge_root", manifest.codex?.bridge_root, ".agents/skills"],
    ["codex.agent_root", manifest.codex?.agent_root, ".codex/agents"],
    ["codex.max_depth", manifest.codex?.max_depth, 2],
    ["codex.max_threads", manifest.codex?.max_threads, 6],
    ["codex.effort", manifest.codex?.effort, "xhigh"],
    ["dispatch.envelope_version", manifest.dispatch?.envelope_version, 1],
    ["dispatch.required_skills_field", manifest.dispatch?.required_skills_field, "Required skills"],
    ["dispatch.failure_prefix", manifest.dispatch?.failure_prefix, "BLOCKED(skill-contract)"],
  ];
  for (const [path, actual, expected] of constants) {
    if (actual !== expected) {
      diagnostics.push(
        diagnostic("INVALID_CONTRACT_CONSTANT", `${path} must equal ${JSON.stringify(expected)}`, {
          path,
          expected,
          actual,
        }),
      );
    }
  }
  requireExactKeys(
    manifest.codex,
    ["bridge_root", "agent_root", "max_depth", "max_threads", "effort"],
    "codex",
    diagnostics,
  );
  requireExactKeys(
    manifest.dispatch,
    ["envelope_version", "required_skills_field", "failure_prefix"],
    "dispatch",
    diagnostics,
  );

  for (const key of ["skills", "packs", "triggers", "roles"]) {
    if (!Array.isArray(manifest[key])) {
      diagnostics.push(
        diagnostic("INVALID_MANIFEST_SHAPE", `${key} must be an array`, {
          path: key,
          expected: "array",
          actual: typeof manifest[key],
        }),
      );
    }
  }

  const skills = asArray(manifest.skills);
  const packs = asArray(manifest.packs);
  const triggers = asArray(manifest.triggers);
  const roles = asArray(manifest.roles);
  const skillById = new Map(skills.map((entry) => [entry?.id, entry]));
  const packById = new Map(packs.map((entry) => [entry?.id, entry]));
  const triggerById = new Map(triggers.map((entry) => [entry?.id, entry]));

  for (const id of duplicates(skills, (entry) => entry?.id)) {
    diagnostics.push(
      diagnostic("DUPLICATE_SKILL_ID", `duplicate governed skill id: ${id}`, {
        path: "agents/skill-routing.json",
        skill: id,
        actual: id,
      }),
    );
  }
  for (const name of duplicates(skills, (entry) => String(entry?.codex_name).toLowerCase())) {
    diagnostics.push(
      diagnostic("DUPLICATE_CODEX_NAME", `duplicate Codex skill name: ${name}`, {
        path: "agents/skill-routing.json",
        actual: name,
      }),
    );
  }
  for (const [kind, entries, code] of [
    ["pack", packs, "DUPLICATE_PACK_ID"],
    ["trigger", triggers, "DUPLICATE_TRIGGER_ID"],
    ["role", roles, "DUPLICATE_ROLE_ID"],
  ]) {
    for (const id of duplicates(entries, (entry) => entry?.id)) {
      diagnostics.push(
        diagnostic(code, `duplicate ${kind} id: ${id}`, {
          path: "agents/skill-routing.json",
          actual: id,
        }),
      );
    }
  }
  for (const rolePath of duplicates(roles, (entry) => entry?.role_path)) {
    diagnostics.push(
      diagnostic("DUPLICATE_ROLE_PATH", `multiple roles use the same role path: ${rolePath}`, {
        path: rolePath,
        actual: rolePath,
      }),
    );
  }

  for (const entry of skills) {
    if (!entry || typeof entry !== "object") continue;
    const allowed = [
      "id",
      "codex_name",
      "description",
      "status",
      "source",
      "dependencies",
      "restrictions",
      "unavailable_reason",
    ];
    const unknown = Object.keys(entry).filter((key) => !allowed.includes(key));
    if (unknown.length > 0) {
      diagnostics.push(
        diagnostic("INVALID_SKILL_KEYS", `${entry.id ?? "skill"} has unknown keys`, {
          path: "agents/skill-routing.json",
          skill: entry.id,
          expected: allowed,
          actual: unknown.sort(byteCompare),
        }),
      );
    }
    if (!/^skill:[a-z0-9][a-z0-9-]*$/.test(entry.id ?? "")) {
      diagnostics.push(
        diagnostic("INVALID_SKILL_ID", `invalid governed skill id: ${entry.id}`, {
          path: "agents/skill-routing.json",
          skill: entry.id,
          actual: entry.id,
        }),
      );
    }
    if (!/^govfolio-[a-z0-9][a-z0-9-]*$/.test(entry.codex_name ?? "")) {
      diagnostics.push(
        diagnostic("INVALID_CODEX_NAME", `invalid Codex skill name: ${entry.codex_name}`, {
          path: "agents/skill-routing.json",
          skill: entry.id,
          actual: entry.codex_name,
        }),
      );
    }
    if (!Array.isArray(entry.dependencies) || !Array.isArray(entry.restrictions)) {
      diagnostics.push(
        diagnostic("INVALID_SKILL_LIST", `${entry.id} dependencies and restrictions must be arrays`, {
          path: "agents/skill-routing.json",
          skill: entry.id,
        }),
      );
    }
    if (entry.status === "planned") {
      if (entry.source !== null || typeof entry.unavailable_reason !== "string") {
        diagnostics.push(
          diagnostic("INVALID_PLANNED_SKILL", `${entry.id} must have null source and an unavailable reason`, {
            path: "agents/skill-routing.json",
            skill: entry.id,
            expected: "source=null and unavailable_reason",
            actual: entry.source,
          }),
        );
      }
      continue;
    }
    if (entry.status !== "available") {
      diagnostics.push(
        diagnostic("INVALID_SKILL_STATUS", `${entry.id} has invalid status`, {
          path: "agents/skill-routing.json",
          skill: entry.id,
          expected: ["available", "planned"],
          actual: entry.status,
        }),
      );
      continue;
    }
    if (!entry.source || typeof entry.source !== "object") {
      diagnostics.push(
        diagnostic("SKILL_SOURCE_MISSING", `${entry.id} has no canonical source`, {
          path: "agents/skill-routing.json",
          skill: entry.id,
        }),
      );
      continue;
    }
    const sourceKeys = Object.keys(entry.source).sort(byteCompare);
    const expectedSourceKeys = ["canonical_path", "file_count", "tree_sha256"].sort(byteCompare);
    if (stableStringify(sourceKeys) !== stableStringify(expectedSourceKeys)) {
      diagnostics.push(
        diagnostic("INVALID_SOURCE_KEYS", `${entry.id} source has unexpected or missing keys`, {
          path: "agents/skill-routing.json",
          skill: entry.id,
          expected: expectedSourceKeys,
          actual: sourceKeys,
        }),
      );
    }
    if (
      typeof entry.source.canonical_path !== "string" ||
      !/^[0-9a-f]{64}$/.test(entry.source.tree_sha256 ?? "") ||
      !Number.isInteger(entry.source.file_count) ||
      entry.source.file_count < 1
    ) {
      diagnostics.push(
        diagnostic("INVALID_SKILL_SOURCE", `${entry.id} source lock is malformed`, {
          path: "agents/skill-routing.json",
          skill: entry.id,
          expected: "canonical_path, 64-char lowercase SHA-256, and positive file_count",
          actual: entry.source,
        }),
      );
    }
    const hashed = hashSkillTree(repoRoot, entry.source.canonical_path);
    diagnostics.push(
      ...hashed.diagnostics.map((item) => ({ ...item, skill: entry.id })),
    );
    if (hashed.diagnostics.length === 0) {
      if (entry.source.tree_sha256 !== hashed.tree_sha256) {
        diagnostics.push(
          diagnostic("SKILL_TREE_HASH_MISMATCH", `${entry.id} canonical tree hash is stale`, {
            path: entry.source.canonical_path,
            skill: entry.id,
            expected: entry.source.tree_sha256,
            actual: hashed.tree_sha256,
            repair: "run node scripts/agents/refresh-codex-skill-lock.mjs",
          }),
        );
      }
      if (entry.source.file_count !== hashed.file_count) {
        diagnostics.push(
          diagnostic("SKILL_FILE_COUNT_MISMATCH", `${entry.id} canonical file count is stale`, {
            path: entry.source.canonical_path,
            skill: entry.id,
            expected: entry.source.file_count,
            actual: hashed.file_count,
            repair: "run node scripts/agents/refresh-codex-skill-lock.mjs",
          }),
        );
      }
    }
    for (const dependency of asArray(entry.dependencies)) {
      if (!skillById.has(dependency) && !packById.has(dependency)) {
        diagnostics.push(
          diagnostic("UNKNOWN_DEPENDENCY_ID", `${entry.id} references unknown dependency ${dependency}`, {
            path: "agents/skill-routing.json",
            skill: entry.id,
            actual: dependency,
          }),
        );
      }
    }
  }

  for (const pack of packs) {
    if (!pack || typeof pack !== "object") continue;
    const packKeys = Object.keys(pack).sort(byteCompare);
    const expectedPackKeys = ["id", "members", "planned_members", "slot_cost"].sort(byteCompare);
    if (stableStringify(packKeys) !== stableStringify(expectedPackKeys)) {
      diagnostics.push(
        diagnostic("INVALID_PACK_KEYS", `${pack.id ?? "pack"} has unexpected or missing keys`, {
          path: "agents/skill-routing.json",
          expected: expectedPackKeys,
          actual: packKeys,
        }),
      );
    }
    if (!/^pack:[a-z0-9][a-z0-9-]*$/.test(pack.id ?? "")) {
      diagnostics.push(
        diagnostic("INVALID_PACK_ID", `invalid governed pack id: ${pack.id}`, {
          path: "agents/skill-routing.json",
          actual: pack.id,
        }),
      );
    }
    if (pack.slot_cost !== 1) {
      diagnostics.push(
        diagnostic("INVALID_PACK_SLOT_COST", `${pack.id} must cost exactly one slot`, {
          path: "agents/skill-routing.json",
          expected: 1,
          actual: pack.slot_cost,
        }),
      );
    }
    if (!Array.isArray(pack.members) || !Array.isArray(pack.planned_members)) {
      diagnostics.push(
        diagnostic("INVALID_PACK_LIST", `${pack.id} members and planned_members must be arrays`, {
          path: "agents/skill-routing.json",
        }),
      );
    }
    const allMembers = [...asArray(pack.members), ...asArray(pack.planned_members)];
    if (allMembers.length > 3) {
      diagnostics.push(
        diagnostic("INVALID_PACK_SIZE", `${pack.id} may contain at most three total members`, {
          path: "agents/skill-routing.json",
          expected: 3,
          actual: allMembers.length,
        }),
      );
    }
    for (const member of duplicates(allMembers, (value) => value)) {
      diagnostics.push(
        diagnostic("DUPLICATE_PACK_MEMBER", `${pack.id} repeats member ${member}`, {
          path: "agents/skill-routing.json",
          skill: member,
          actual: member,
        }),
      );
    }
    for (const member of asArray(pack.members)) {
      if (!skillById.has(member)) {
        diagnostics.push(
          diagnostic("UNKNOWN_PACK_MEMBER", `${pack.id} references unknown member ${member}`, {
            path: "agents/skill-routing.json",
            skill: member,
            actual: member,
          }),
        );
      } else if (skillById.get(member).status !== "available") {
        diagnostics.push(
          diagnostic("PACK_MEMBER_UNAVAILABLE", `${pack.id} dispatchable member is unavailable: ${member}`, {
            path: "agents/skill-routing.json",
            skill: member,
            expected: "available",
            actual: skillById.get(member).status,
          }),
        );
      }
    }
    for (const member of asArray(pack.planned_members)) {
      const planned = skillById.get(member);
      if (!planned || planned.status !== "planned") {
        diagnostics.push(
          diagnostic("INVALID_PLANNED_PACK_MEMBER", `${pack.id} planned member is not a planned skill: ${member}`, {
            path: "agents/skill-routing.json",
            skill: member,
            actual: member,
          }),
        );
      }
    }
  }

  for (const trigger of triggers) {
    if (!trigger || typeof trigger !== "object") continue;
    const triggerKeys = Object.keys(trigger).sort(byteCompare);
    const expectedTriggerKeys = ["description", "id"];
    if (stableStringify(triggerKeys) !== stableStringify(expectedTriggerKeys)) {
      diagnostics.push(
        diagnostic("INVALID_TRIGGER_KEYS", `${trigger.id ?? "trigger"} has unexpected or missing keys`, {
          path: "agents/skill-routing.json",
          expected: expectedTriggerKeys,
          actual: triggerKeys,
        }),
      );
    }
    if (!/^trigger:[a-z0-9][a-z0-9-]*$/.test(trigger.id ?? "")) {
      diagnostics.push(
        diagnostic("INVALID_TRIGGER_ID", `invalid governed trigger id: ${trigger.id}`, {
          path: "agents/skill-routing.json",
          actual: trigger.id,
        }),
      );
    }
    if (typeof trigger.description !== "string" || trigger.description.trim() === "") {
      diagnostics.push(
        diagnostic("INVALID_TRIGGER_DESCRIPTION", `${trigger.id} must have a non-empty description`, {
          path: "agents/skill-routing.json",
          actual: trigger.description,
        }),
      );
    }
  }

  const knownRequirements = new Set([...skillById.keys(), ...packById.keys()]);
  for (const role of roles) {
    if (!role || typeof role !== "object") continue;
    const roleKeys = Object.keys(role).sort(byteCompare);
    const expectedRoleKeys = ["active", "description", "id", "role_path", "situational"];
    if (stableStringify(roleKeys) !== stableStringify(expectedRoleKeys)) {
      diagnostics.push(
        diagnostic("INVALID_ROLE_KEYS", `${role.id ?? "role"} has unexpected or missing keys`, {
          path: "agents/skill-routing.json",
          role: role.id,
          expected: expectedRoleKeys,
          actual: roleKeys,
        }),
      );
    }
    if (!/^[a-z0-9][a-z0-9-]*$/.test(role.id ?? "")) {
      diagnostics.push(
        diagnostic("INVALID_ROLE_ID", `invalid governed role id: ${role.id}`, {
          path: "agents/skill-routing.json",
          role: role.id,
          actual: role.id,
        }),
      );
    }
    if (
      typeof role.role_path !== "string" ||
      !/^agents\/roles\/[a-z0-9][a-z0-9-]*\.md$/.test(role.role_path)
    ) {
      diagnostics.push(
        diagnostic("INVALID_ROLE_PATH", `${role.id} has invalid role_path`, {
          path: "agents/skill-routing.json",
          role: role.id,
          actual: role.role_path,
        }),
      );
    }
    if (!Array.isArray(role.active) || !Array.isArray(role.situational)) {
      diagnostics.push(
        diagnostic("INVALID_ROLE_LIST", `${role.id} active and situational fields must be arrays`, {
          path: "agents/skill-routing.json",
          role: role.id,
        }),
      );
    }
    let slotCost = 0;
    for (const requirement of asArray(role.active)) {
      slotCost += packById.get(requirement)?.slot_cost ?? 1;
    }
    if (slotCost > 6) {
      diagnostics.push(
        diagnostic("ROLE_SLOT_LIMIT_EXCEEDED", `${role.id} uses more than six ACTIVE slots`, {
          path: "agents/skill-routing.json",
          role: role.id,
          expected: 6,
          actual: slotCost,
        }),
      );
    }
    for (const requirement of duplicates(asArray(role.active), (value) => value)) {
      diagnostics.push(
        diagnostic("DUPLICATE_ACTIVE_REQUIREMENT", `${role.id} repeats ACTIVE requirement ${requirement}`, {
          path: "agents/skill-routing.json",
          role: role.id,
          skill: requirement,
          actual: requirement,
        }),
      );
    }
    for (const requirement of asArray(role.active)) {
      if (!knownRequirements.has(requirement)) {
        diagnostics.push(
          diagnostic("UNKNOWN_SKILL_ID", `role ${role.id} references unknown ${requirement}`, {
            path: "agents/skill-routing.json",
            role: role.id,
            skill: requirement,
            actual: requirement,
          }),
        );
      } else if (skillById.get(requirement)?.status === "planned") {
        diagnostics.push(
          diagnostic("PLANNED_ACTIVE_SKILL", `role ${role.id} has planned ACTIVE skill ${requirement}`, {
            path: "agents/skill-routing.json",
            role: role.id,
            skill: requirement,
            actual: requirement,
          }),
        );
      }
    }
    for (const situation of asArray(role.situational)) {
      const situationKeys = situation && typeof situation === "object"
        ? Object.keys(situation).sort(byteCompare)
        : [];
      if (stableStringify(situationKeys) !== stableStringify(["requirements", "trigger"])) {
        diagnostics.push(
          diagnostic("INVALID_SITUATION_KEYS", `${role.id} has malformed situational allocation`, {
            path: "agents/skill-routing.json",
            role: role.id,
            expected: ["requirements", "trigger"],
            actual: situationKeys,
          }),
        );
      }
      if (!Array.isArray(situation?.requirements) || situation.requirements.length === 0) {
        diagnostics.push(
          diagnostic("INVALID_SITUATION_REQUIREMENTS", `${role.id} situational requirements must be a non-empty array`, {
            path: "agents/skill-routing.json",
            role: role.id,
            actual: situation?.requirements,
          }),
        );
      }
      if (!triggerById.has(situation?.trigger)) {
        diagnostics.push(
          diagnostic("UNKNOWN_TRIGGER_ID", `role ${role.id} references unknown trigger ${situation?.trigger}`, {
            path: "agents/skill-routing.json",
            role: role.id,
            actual: situation?.trigger,
          }),
        );
      }
      for (const requirement of asArray(situation?.requirements)) {
        if (!knownRequirements.has(requirement)) {
          diagnostics.push(
            diagnostic("UNKNOWN_SKILL_ID", `role ${role.id} references unknown ${requirement}`, {
              path: "agents/skill-routing.json",
              role: role.id,
              skill: requirement,
              actual: requirement,
            }),
          );
        }
      }
    }
  }

  return sortDiagnostics(diagnostics);
}

export function hashSkillTree(repoRoot, canonicalDirectory) {
  const diagnostics = [];
  const safe = safeRepositoryPath(repoRoot, canonicalDirectory);
  if (!safe) {
    return {
      tree_sha256: null,
      file_count: 0,
      diagnostics: [
        diagnostic(
          "CANONICAL_PATH_OUTSIDE_REPO",
          `canonical skill path must stay inside the repository: ${canonicalDirectory}`,
          {
            path: canonicalDirectory,
            actual: canonicalDirectory,
            repair: "use a forward-slash repository-relative canonical path",
          },
        ),
      ],
    };
  }
  if (!existsSync(safe.absolutePath)) {
    return {
      tree_sha256: null,
      file_count: 0,
      diagnostics: [
        diagnostic("CANONICAL_PATH_MISSING", `canonical skill path does not exist: ${canonicalDirectory}`, {
          path: canonicalDirectory,
          actual: canonicalDirectory,
          repair: `restore ${canonicalDirectory} or mark the skill planned`,
        }),
      ],
    };
  }
  const rootStat = lstatSync(safe.absolutePath);
  if (rootStat.isSymbolicLink()) {
    return {
      tree_sha256: null,
      file_count: 0,
      diagnostics: [
        diagnostic("CANONICAL_SYMLINK_FORBIDDEN", `canonical skill path is a symlink: ${canonicalDirectory}`, {
          path: canonicalDirectory,
          actual: canonicalDirectory,
          repair: `replace ${canonicalDirectory} with regular tracked files`,
        }),
      ],
    };
  }
  if (!rootStat.isDirectory()) {
    return {
      tree_sha256: null,
      file_count: 0,
      diagnostics: [
        diagnostic("CANONICAL_PATH_NOT_DIRECTORY", `canonical skill path is not a directory: ${canonicalDirectory}`, {
          path: canonicalDirectory,
          actual: canonicalDirectory,
        }),
      ],
    };
  }
  const rootReal = realpathSync(resolve(repoRoot));
  const skillReal = realpathSync(safe.absolutePath);
  const realRelative = relative(rootReal, skillReal);
  if (realRelative === ".." || realRelative.startsWith(`..${sep}`) || isAbsolute(realRelative)) {
    return {
      tree_sha256: null,
      file_count: 0,
      diagnostics: [
        diagnostic("CANONICAL_PATH_OUTSIDE_REPO", `canonical skill resolves outside the repository: ${canonicalDirectory}`, {
          path: canonicalDirectory,
          actual: skillReal,
        }),
      ],
    };
  }

  const files = [];
  collectFiles(resolve(repoRoot), safe.absolutePath, files, diagnostics, canonicalDirectory);
  const lowerCasePaths = new Map();
  for (const file of files) {
    const folded = file.repositoryPath.toLowerCase();
    const previous = lowerCasePaths.get(folded);
    if (previous && previous !== file.repositoryPath) {
      diagnostics.push(
        diagnostic("CANONICAL_CASE_COLLISION", `canonical paths collide by case: ${previous} and ${file.repositoryPath}`, {
          path: canonicalDirectory,
          expected: previous,
          actual: file.repositoryPath,
        }),
      );
    }
    lowerCasePaths.set(folded, file.repositoryPath);
  }
  if (diagnostics.length > 0) {
    return { tree_sha256: null, file_count: 0, diagnostics: sortDiagnostics(diagnostics) };
  }

  files.sort((left, right) => byteCompare(left.repositoryPath, right.repositoryPath));
  const hash = createHash("sha256");
  try {
    for (const file of files) {
      const blobId = execFileSync(
        "git",
        [
          "hash-object",
          "--filters",
          `--path=${file.repositoryPath}`,
          file.absolutePath,
        ],
        { cwd: repoRoot, encoding: "utf8" },
      ).trim();
      if (!/^[0-9a-f]{40}$/.test(blobId)) {
        throw new Error(`git returned invalid blob id ${JSON.stringify(blobId)}`);
      }
      const pathBytes = Buffer.from(file.repositoryPath, "utf8");
      const blobBytes = Buffer.from(blobId, "ascii");
      hash.update(frame(pathBytes));
      hash.update(pathBytes);
      hash.update(frame(blobBytes));
      hash.update(blobBytes);
    }
  } catch (error) {
    return {
      tree_sha256: null,
      file_count: 0,
      diagnostics: [
        diagnostic("GIT_FILTER_HASH_FAILED", `cannot hash ${canonicalDirectory} with Git clean filters: ${error.message}`, {
          path: canonicalDirectory,
          actual: error.message,
          repair: "ensure Git is available and the repository attributes are valid",
        }),
      ],
    };
  }
  return { tree_sha256: hash.digest("hex"), file_count: files.length, diagnostics: [] };
}

export function parseRequiredSkills(markdown, sourcePath, heading) {
  if (heading === null || heading === undefined || heading === "") {
    return { requirements: [], diagnostics: [] };
  }
  const lines = String(markdown).split(/\r?\n/);
  let fenced = false;
  const visible = lines.map((line) => {
    if (/^\s{0,3}(```|~~~)/.test(line)) {
      fenced = !fenced;
      return null;
    }
    return fenced ? null : line;
  });
  let start = -1;
  let targetIndex = -1;
  let level = 0;
  let ancestors = [];
  const headingStack = [];
  for (let index = 0; index < visible.length; index += 1) {
    const match = visible[index]?.match(/^(#{1,6})\s+(.+?)\s*#*\s*$/);
    if (!match) continue;
    const currentLevel = match[1].length;
    while (headingStack.at(-1)?.level >= currentLevel) headingStack.pop();
    if (match[2] === heading) {
      start = index + 1;
      targetIndex = index;
      level = currentLevel;
      ancestors = [...headingStack];
      break;
    }
    headingStack.push({ index, level: currentLevel, heading: match[2] });
  }
  if (start < 0) {
    return {
      requirements: [],
      diagnostics: [
        diagnostic("REQUIRED_SKILLS_HEADING_MISSING", `heading not found in ${sourcePath}: ${heading}`, {
          path: sourcePath,
          expected: heading,
          actual: null,
          repair: `use an exact Markdown heading from ${sourcePath}`,
        }),
      ],
    };
  }
  function fieldInRange(rangeStart, rangeEnd, ownerHeading) {
    for (let index = rangeStart; index < rangeEnd; index += 1) {
      const field = visible[index]?.match(/^\s*\*\*Required skills:\*\*\s*(.*?)\s*$/);
      if (!field) continue;
      if (field[1] === "" || /^none$/i.test(field[1])) {
        return { requirements: [], diagnostics: [] };
      }
      const requirements = field[1].split(",").map((value) => value.trim()).filter(Boolean);
      const invalid = requirements.filter((value) => !/^(skill|pack):[a-z0-9][a-z0-9-]*$/.test(value));
      if (invalid.length > 0) {
        return {
          requirements: [],
          diagnostics: invalid.map((value) =>
            diagnostic("INVALID_REQUIRED_SKILL_ID", `invalid Required skills entry: ${value}`, {
              path: sourcePath,
              skill: value,
              actual: value,
              repair: `use comma-separated namespaced skill: or pack: IDs under ${ownerHeading}`,
            }),
          ),
        };
      }
      return { requirements, diagnostics: [] };
    }
    return { requirements: [], diagnostics: [] };
  }

  const requirements = [];
  const diagnostics = [];
  for (const ancestor of ancestors) {
    let end = targetIndex;
    for (let index = ancestor.index + 1; index < targetIndex; index += 1) {
      if (visible[index]?.match(/^(#{1,6})\s+/)) {
        end = index;
        break;
      }
    }
    const parsed = fieldInRange(ancestor.index + 1, end, ancestor.heading);
    requirements.push(...parsed.requirements);
    diagnostics.push(...parsed.diagnostics);
  }
  let sectionEnd = visible.length;
  for (let index = start; index < visible.length; index += 1) {
    const nextHeading = visible[index]?.match(/^(#{1,6})\s+/);
    if (nextHeading && nextHeading[1].length <= level) {
      sectionEnd = index;
      break;
    }
  }
  const selected = fieldInRange(start, sectionEnd, heading);
  requirements.push(...selected.requirements);
  diagnostics.push(...selected.diagnostics);
  if (diagnostics.length > 0) return { requirements: [], diagnostics: sortDiagnostics(diagnostics) };
  return { requirements: [...new Set(requirements)], diagnostics: [] };
}

export function expandRequirements(manifest, requirementIds) {
  const diagnostics = [];
  const skills = new Map(asArray(manifest?.skills).map((entry) => [entry.id, entry]));
  const packs = new Map(asArray(manifest?.packs).map((entry) => [entry.id, entry]));
  const selected = new Map();
  const visited = new Set();

  function visit(requirement, parent = null) {
    if (visited.has(requirement)) return;
    visited.add(requirement);
    const skill = skills.get(requirement);
    if (skill) {
      if (skill.status !== "available") {
        diagnostics.push(
          diagnostic("PLANNED_SKILL_REQUIRED", `required skill is unavailable: ${requirement}`, {
            path: "agents/skill-routing.json",
            skill: requirement,
            expected: "available",
            actual: skill.status,
            repair: `make ${requirement} available with a reviewed canonical source or remove the requirement`,
          }),
        );
        return;
      }
      selected.set(skill.id, skill);
      for (const dependency of asArray(skill.dependencies)) visit(dependency, skill.id);
      return;
    }
    const pack = packs.get(requirement);
    if (pack) {
      for (const member of asArray(pack.members)) visit(member, pack.id);
      return;
    }
    diagnostics.push(
      diagnostic("UNKNOWN_REQUIREMENT_ID", `unknown governed requirement: ${requirement}`, {
        path: "agents/skill-routing.json",
        skill: parent ?? requirement,
        actual: requirement,
      }),
    );
  }

  for (const requirement of asArray(requirementIds)) visit(requirement);
  if (diagnostics.length > 0) {
    return { skills: [], diagnostics: sortDiagnostics(diagnostics) };
  }
  return {
    skills: [...selected.values()].sort((left, right) => byteCompare(left.id, right.id)),
    diagnostics: [],
  };
}

export function resolveDispatch(manifest, options) {
  const diagnostics = [];
  const roles = new Map(asArray(manifest?.roles).map((entry) => [entry.id, entry]));
  const knownTriggers = new Set(asArray(manifest?.triggers).map((entry) => entry.id));
  const role = roles.get(options?.role);
  if (!role) {
    diagnostics.push(
      diagnostic("UNKNOWN_ROLE_ID", `unknown governed role: ${options?.role}`, {
        path: "agents/skill-routing.json",
        role: options?.role,
        actual: options?.role,
      }),
    );
  }
  const requestedTriggers = [...new Set(asArray(options?.triggers))].sort(byteCompare);
  for (const trigger of requestedTriggers) {
    if (!knownTriggers.has(trigger)) {
      diagnostics.push(
        diagnostic("UNKNOWN_TRIGGER_ID", `unknown explicit trigger: ${trigger}`, {
          path: "agents/skill-routing.json",
          role: options?.role,
          actual: trigger,
          repair: "use an exact trigger ID declared in agents/skill-routing.json",
        }),
      );
    }
  }
  if (diagnostics.length > 0) {
    return { envelope: null, diagnostics: sortDiagnostics(diagnostics) };
  }

  const requirements = [...asArray(role.active)];
  for (const trigger of requestedTriggers) {
    const situation = asArray(role.situational).find((entry) => entry.trigger === trigger);
    if (!situation) {
      diagnostics.push(
        diagnostic("TRIGGER_NOT_ASSIGNED_TO_ROLE", `${trigger} is not assigned to role ${role.id}`, {
          path: "agents/skill-routing.json",
          role: role.id,
          actual: trigger,
        }),
      );
      continue;
    }
    requirements.push(...asArray(situation.requirements));
  }

  if (options?.sectionFile || options?.sectionHeading) {
    if (!options.sectionFile || !options.sectionHeading) {
      diagnostics.push(
        diagnostic("INCOMPLETE_SECTION_SELECTOR", "sectionFile and sectionHeading must be supplied together", {
          path: options?.sectionFile,
          role: role.id,
        }),
      );
    } else {
      try {
        const repoRoot = findRepoRoot(process.cwd());
        const safe = safeRepositoryPath(repoRoot, options.sectionFile);
        if (!safe) throw new Error("section path escapes the repository");
        const parsed = parseRequiredSkills(
          readFileSync(safe.absolutePath, "utf8"),
          options.sectionFile,
          options.sectionHeading,
        );
        diagnostics.push(...parsed.diagnostics.map((item) => ({ ...item, role: role.id })));
        requirements.push(...parsed.requirements);
      } catch (error) {
        diagnostics.push(
          diagnostic("SECTION_READ_FAILED", `cannot read dispatch section: ${error.message}`, {
            path: options.sectionFile,
            role: role.id,
            actual: error.message,
          }),
        );
      }
    }
  }

  const expanded = expandRequirements(manifest, requirements);
  diagnostics.push(...expanded.diagnostics.map((item) => ({ ...item, role: role.id })));
  if (diagnostics.length > 0) {
    return { envelope: null, diagnostics: sortDiagnostics(diagnostics) };
  }
  const contractSha256 = createHash("sha256").update(stableStringify(manifest)).digest("hex");
  return {
    envelope: {
      contract_sha256: contractSha256,
      role: role.id,
      source_context: options?.sourceContext ?? null,
      triggers: requestedTriggers,
      skills: expanded.skills.map((entry) => ({
        id: entry.id,
        codex_name: entry.codex_name,
        canonical_path: entry.source.canonical_path,
        tree_sha256: entry.source.tree_sha256,
      })),
    },
    diagnostics: [],
  };
}

function yamlString(value) {
  return JSON.stringify(String(value));
}

function tomlString(value) {
  return JSON.stringify(String(value));
}

export function renderBridgeSkill(_manifest, skill) {
  if (skill.status !== "available" || !skill.source) {
    throw new Error(`cannot render unavailable skill ${skill.id}`);
  }
  const restrictions = asArray(skill.restrictions).map((item) => `- ${item}`);
  return [
    "---",
    MARKERS.comment,
    `name: ${skill.codex_name}`,
    `description: ${yamlString(skill.description)}`,
    "---",
    "",
    `# Governed bridge for ${skill.id}`,
    "",
    `Canonical directory: \`${skill.source.canonical_path}\``,
    `Expected tree SHA-256: \`${skill.source.tree_sha256}\``,
    "",
    "Before any task action, verify this path and hash against `agents/skill-routing.json`, then read the canonical `SKILL.md` completely. Resolve every relative resource from the canonical directory.",
    ...(restrictions.length > 0 ? ["", "Additional enforced restrictions:", "", ...restrictions] : []),
    "",
    "If the source is missing, stale, or unreadable, return `BLOCKED(skill-contract)` without task mutation.",
    "",
  ].join("\n");
}

export function renderOpenAiMetadata(skill) {
  const slug = skill.id.replace(/^skill:/, "");
  return [
    MARKERS.comment,
    "interface:",
    `  display_name: ${yamlString(`Govfolio: ${slug}`)}`,
    `  short_description: ${yamlString("Governed bridge; explicit dispatch only.")}`,
    `  default_prompt: ${yamlString(`Use $${skill.codex_name} to follow the pinned Govfolio workflow for this task.`)}`,
    "policy:",
    "  allow_implicit_invocation: false",
    "",
  ].join("\n");
}

export function renderCodexAgent(manifest, role) {
  const receipt = `SKILLS_LOADED role=${role.id} contract=<contract_sha256> skills=<comma-separated envelope skill IDs in envelope order>`;
  const instructions = [
    "Before task work, require an unmodified GOVFOLIO_DISPATCH_V1 envelope.",
    "The only permitted pre-receipt actions are reading AGENTS.md, tracked CLAUDE.md completely, the exact role file, agents/skill-routing.json, and every listed bridge and canonical SKILL.md, plus running the deterministic contract validator.",
    `Load AGENTS.md and tracked CLAUDE.md completely, then ${role.role_path}.`,
    `Verify every envelope path and hash. Emit exactly: ${receipt}.`,
    `On any mismatch, return ${manifest.dispatch.failure_prefix} and perform no mutation.`,
    "Apply the same envelope and receipt process to every nested dispatch.",
  ].join(" ");
  return [
    MARKERS.comment,
    `name = ${tomlString(role.id)}`,
    `description = ${tomlString(role.description)}`,
    `model_reasoning_effort = ${tomlString(manifest.codex.effort)}`,
    `developer_instructions = ${tomlString(instructions)}`,
    "",
  ].join("\n");
}

export function renderRoleSkillBlock(_manifest, role) {
  const active = asArray(role.active).join(", ") || "none";
  const situational = asArray(role.situational)
    .map((entry) => `${asArray(entry.requirements).join(" + ")} (${entry.trigger})`)
    .join("; ") || "none";
  return [
    MARKERS.roleBegin,
    "5 Skills/Tools (GENERATED from agents/skill-routing.json):",
    `   ACTIVE: ${active}`,
    `   SITUATIONAL: ${situational}`,
    MARKERS.roleEnd,
  ].join("\n");
}

export function formatEnvelope(envelope) {
  if (!envelope || typeof envelope !== "object") {
    throw new TypeError("formatEnvelope requires a non-null envelope");
  }
  return [
    "--- GOVFOLIO_DISPATCH_V1 ---",
    JSON.stringify(envelope),
    "--- END GOVFOLIO_DISPATCH_V1 ---",
  ].join("\n");
}

export function formatDiagnostics(diagnostics) {
  const normalized = asArray(diagnostics).map((item) =>
    diagnostic(item.code, item.message, item));
  return sortDiagnostics(normalized)
    .map((item) => `BLOCKED(skill-contract): ${JSON.stringify(item)}`)
    .join("\n");
}

export const GENERATED_MARKERS = MARKERS;
