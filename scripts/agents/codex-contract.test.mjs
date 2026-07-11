import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import test from "node:test";

import {
  expandRequirements,
  formatEnvelope,
  hashSkillTree,
  parseManifest,
  parseRequiredSkills,
  renderBridgeSkill,
  resolveDispatch,
  validateManifest,
} from "./codex-contract-lib.mjs";

function write(root, relativePath, contents) {
  const path = join(root, ...relativePath.split("/"));
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, contents);
  return path;
}

function fixtureRepository(t) {
  const root = mkdtempSync(join(tmpdir(), "govfolio-contract-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  execFileSync("git", ["init", "--quiet"], { cwd: root });
  execFileSync("git", ["config", "user.email", "contract@example.invalid"], {
    cwd: root,
  });
  execFileSync("git", ["config", "user.name", "Contract Test"], { cwd: root });
  return root;
}

function skill(id, overrides = {}) {
  const slug = id.replace(/^skill:/, "");
  return {
    id,
    codex_name: `govfolio-${slug}`,
    description: `Use the governed ${slug} workflow.`,
    status: "available",
    source: {
      canonical_path: `agents/skills/${slug}`,
      tree_sha256: "a".repeat(64),
      file_count: 1,
    },
    dependencies: [],
    restrictions: [],
    ...overrides,
  };
}

function manifest(overrides = {}) {
  return {
    schema_version: 1,
    hash_algorithm: "sha256-git-blobs-v1",
    codex: {
      bridge_root: ".agents/skills",
      agent_root: ".codex/agents",
      max_depth: 2,
      max_threads: 6,
      effort: "xhigh",
    },
    dispatch: {
      envelope_version: 1,
      required_skills_field: "Required skills",
      failure_prefix: "BLOCKED(skill-contract)",
    },
    skills: [],
    packs: [],
    triggers: [],
    roles: [],
    ...overrides,
  };
}

test("valid manifest preserves slots and expands packs", (t) => {
  const root = fixtureRepository(t);
  write(root, "agents/skills/alpha/SKILL.md", "alpha\n");
  write(root, "agents/skills/beta/SKILL.md", "beta\n");
  const alphaHash = hashSkillTree(root, "agents/skills/alpha");
  const betaHash = hashSkillTree(root, "agents/skills/beta");
  assert.deepEqual(alphaHash.diagnostics, []);
  assert.deepEqual(betaHash.diagnostics, []);

  const input = manifest({
    skills: [
      skill("skill:alpha", {
        source: {
          canonical_path: "agents/skills/alpha",
          tree_sha256: alphaHash.tree_sha256,
          file_count: alphaHash.file_count,
        },
      }),
      skill("skill:beta", {
        source: {
          canonical_path: "agents/skills/beta",
          tree_sha256: betaHash.tree_sha256,
          file_count: betaHash.file_count,
        },
      }),
      skill("skill:future", {
        status: "planned",
        source: null,
        unavailable_reason: "not imported",
      }),
    ],
    packs: [
      {
        id: "pack:craft",
        slot_cost: 1,
        members: ["skill:beta", "skill:alpha"],
        planned_members: ["skill:future"],
      },
    ],
    roles: [
      {
        id: "builder",
        description: "Build one fixture.",
        role_path: "agents/roles/builder.md",
        active: ["skill:alpha", "pack:craft"],
        situational: [],
      },
    ],
  });

  const parsed = parseManifest(JSON.stringify(input), "agents/skill-routing.json");
  assert.deepEqual(parsed.diagnostics, []);
  assert.deepEqual(parsed.manifest.roles[0].active, ["skill:alpha", "pack:craft"]);
  assert.deepEqual(validateManifest(parsed.manifest, root), []);

  const expanded = expandRequirements(parsed.manifest, ["pack:craft"]);
  assert.deepEqual(expanded.diagnostics, []);
  assert.deepEqual(
    expanded.skills.map(({ id }) => id),
    ["skill:alpha", "skill:beta"],
  );
});

test("duplicate skill and Codex names fail", () => {
  const input = manifest({
    skills: [
      skill("skill:alpha"),
      skill("skill:alpha", { codex_name: "govfolio-other" }),
      skill("skill:beta", { codex_name: "govfolio-alpha" }),
    ],
  });

  const codes = validateManifest(input, process.cwd()).map(({ code }) => code);
  assert.ok(codes.includes("DUPLICATE_SKILL_ID"));
  assert.ok(codes.includes("DUPLICATE_CODEX_NAME"));
});

test("unknown and planned requirements fail closed", () => {
  const input = manifest({
    skills: [
      skill("skill:alpha"),
      skill("skill:future", {
        status: "planned",
        source: null,
        unavailable_reason: "license missing",
      }),
    ],
  });

  const unknown = expandRequirements(input, ["skill:missing"]);
  assert.deepEqual(unknown.skills, []);
  assert.deepEqual(unknown.diagnostics.map(({ code }) => code), [
    "UNKNOWN_REQUIREMENT_ID",
  ]);

  const planned = expandRequirements(input, ["skill:future"]);
  assert.deepEqual(planned.skills, []);
  assert.deepEqual(planned.diagnostics.map(({ code }) => code), [
    "PLANNED_SKILL_REQUIRED",
  ]);
});

test("path traversal and symlinked canonical skills fail", (t) => {
  const root = fixtureRepository(t);
  const outside = mkdtempSync(join(tmpdir(), "govfolio-contract-outside-"));
  t.after(() => rmSync(outside, { recursive: true, force: true }));
  write(outside, "SKILL.md", "outside\n");

  const traversal = hashSkillTree(root, "../outside");
  assert.equal(traversal.tree_sha256, null);
  assert.equal(traversal.file_count, 0);
  assert.deepEqual(traversal.diagnostics.map(({ code }) => code), [
    "CANONICAL_PATH_OUTSIDE_REPO",
  ]);

  mkdirSync(join(root, "agents", "skills"), { recursive: true });
  symlinkSync(outside, join(root, "agents", "skills", "linked"), "junction");
  const linked = hashSkillTree(root, "agents/skills/linked");
  assert.equal(linked.tree_sha256, null);
  assert.deepEqual(linked.diagnostics.map(({ code }) => code), [
    "CANONICAL_SYMLINK_FORBIDDEN",
  ]);
});

test("Git-filtered tree hashes are stable across CRLF checkout bytes", (t) => {
  const root = fixtureRepository(t);
  write(root, ".gitattributes", "*.md text eol=lf\n");
  const path = write(root, "agents/skills/alpha/SKILL.md", "one\r\ntwo\r\n");

  const crlf = hashSkillTree(root, "agents/skills/alpha");
  writeFileSync(path, "one\ntwo\n");
  const lf = hashSkillTree(root, "agents/skills/alpha");

  assert.deepEqual(crlf.diagnostics, []);
  assert.deepEqual(lf.diagnostics, []);
  assert.equal(crlf.tree_sha256, lf.tree_sha256);
  assert.equal(crlf.file_count, 1);
});

test("dependency closure is sorted and cycle-safe", () => {
  const input = manifest({
    skills: [
      skill("skill:zeta", { dependencies: ["skill:alpha"] }),
      skill("skill:alpha", { dependencies: ["skill:zeta"] }),
    ],
  });

  const expanded = expandRequirements(input, ["skill:zeta"]);
  assert.deepEqual(expanded.diagnostics, []);
  assert.deepEqual(
    expanded.skills.map(({ id }) => id),
    ["skill:alpha", "skill:zeta"],
  );
});

test("Required skills ignores fenced examples and selects the exact heading", () => {
  const markdown = [
    "# Plan",
    "",
    "```markdown",
    "### Task 2: Target",
    "**Required skills:** skill:wrong",
    "```",
    "",
    "### Task 1: Other",
    "",
    "**Required skills:** skill:other",
    "",
    "### Task 2: Target",
    "",
    "**Required skills:** skill:beta, pack:craft",
    "",
    "#### Notes",
    "",
    "**Required skills:** skill:nested",
    "",
    "### Task 3: Later",
    "",
    "**Required skills:** skill:later",
  ].join("\n");

  const parsed = parseRequiredSkills(markdown, "plan.md", "Task 2: Target");
  assert.deepEqual(parsed.diagnostics, []);
  assert.deepEqual(parsed.requirements, ["skill:beta", "pack:craft"]);
});

test("unknown triggers are rejected instead of inferred", () => {
  const input = manifest({
    skills: [skill("skill:alpha")],
    roles: [
      {
        id: "builder",
        description: "Build.",
        role_path: "agents/roles/builder.md",
        active: ["skill:alpha"],
        situational: [],
      },
    ],
  });

  const resolution = resolveDispatch(input, {
    role: "builder",
    triggers: ["trigger:invented"],
    sectionFile: null,
    sectionHeading: null,
    sourceContext: null,
  });
  assert.equal(resolution.envelope, null);
  assert.deepEqual(resolution.diagnostics.map(({ code }) => code), [
    "UNKNOWN_TRIGGER_ID",
  ]);
});

test("dispatch formatting is deterministic JSON", () => {
  const input = manifest({
    skills: [skill("skill:alpha")],
    roles: [
      {
        id: "builder",
        description: "Build.",
        role_path: "agents/roles/builder.md",
        active: ["skill:alpha"],
        situational: [],
      },
    ],
  });

  const resolution = resolveDispatch(input, {
    role: "builder",
    triggers: [],
    sectionFile: null,
    sectionHeading: null,
    sourceContext: "docs/context.md",
  });
  assert.deepEqual(resolution.diagnostics, []);
  assert.ok(resolution.envelope);

  const first = formatEnvelope(resolution.envelope);
  const second = formatEnvelope(resolution.envelope);
  assert.equal(first, second);
  const lines = first.split("\n");
  assert.equal(lines[0], "--- GOVFOLIO_DISPATCH_V1 ---");
  assert.equal(lines.at(-1), "--- END GOVFOLIO_DISPATCH_V1 ---");
  assert.deepEqual(JSON.parse(lines[1]), resolution.envelope);
  assert.equal(
    readFileSync(new URL("./codex-contract.test.mjs", import.meta.url), "utf8").includes(
      "GOVFOLIO_DISPATCH_V1",
    ),
    true,
  );
});

test("generated bridge starts with valid YAML frontmatter", () => {
  const rendered = renderBridgeSkill(manifest(), skill("skill:alpha"));
  const lines = rendered.split("\n");
  assert.equal(lines[0], "---");
  assert.match(lines[1], /^# GENERATED: govfolio-codex-skill-contract/);
  assert.equal(lines[4], "---");
});
