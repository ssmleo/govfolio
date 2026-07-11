import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import {
  chmodSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

import {
  GENERATED_MARKERS,
  expandRequirements,
  formatDiagnostics,
  formatEnvelope,
  hashSkillTree,
  parseManifest,
  parseRequiredSkills,
  renderBridgeSkill,
  renderCodexAgent,
  renderOpenAiMetadata,
  renderRoleSkillBlock,
  resolveDispatch,
  validateManifest,
} from "./codex-contract-lib.mjs";

const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const repositoryRoot = join(scriptDirectory, "..", "..");

function repositoryFile(relativePath) {
  return readFileSync(join(repositoryRoot, ...relativePath.split("/")), "utf8");
}

function runCli(name, args, options = {}) {
  return execFileSync(process.execPath, [join(scriptDirectory, name), ...args], {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
    ...options,
  });
}

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
  execFileSync("git", ["config", "core.autocrlf", "false"], { cwd: root });
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

function lockedSkill(root, id, overrides = {}) {
  const { canonicalPath = `agents/skills/${id.replace(/^skill:/, "")}`, ...skillOverrides } = overrides;
  write(root, `${canonicalPath}/SKILL.md`, `${id}\n`);
  const hashed = hashSkillTree(root, canonicalPath);
  assert.deepEqual(hashed.diagnostics, []);
  return skill(id, {
    ...skillOverrides,
    source: {
      canonical_path: canonicalPath,
      tree_sha256: hashed.tree_sha256,
      file_count: hashed.file_count,
    },
  });
}

function writeManifest(root, value) {
  write(root, "agents/skill-routing.json", `${JSON.stringify(value, null, 2)}\n`);
}

function fixtureRole(id = "builder") {
  return {
    id,
    description: `Perform governed work as ${id}.`,
    role_path: `agents/roles/${id}.md`,
    active: ["skill:alpha"],
    situational: [],
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

test("Required skills inherits plan preamble requirements", () => {
  const markdown = [
    "# Plan",
    "",
    "**Required skills:** skill:executing-plans",
    "",
    "## Task 1: Build",
    "",
    "**Required skills:** skill:rust-tdd",
  ].join("\n");

  const parsed = parseRequiredSkills(markdown, "plan.md", "Task 1: Build");
  assert.deepEqual(parsed.diagnostics, []);
  assert.deepEqual(parsed.requirements, [
    "skill:executing-plans",
    "skill:rust-tdd",
  ]);
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

test("refresh-lock changes only lock fields", (t) => {
  const root = fixtureRepository(t);
  write(root, "agents/skills/alpha/SKILL.md", "alpha\n");
  const input = manifest({
    skills: [
      skill("skill:alpha", {
        source: {
          canonical_path: "agents/skills/alpha",
          tree_sha256: "0".repeat(64),
          file_count: 99,
        },
      }),
    ],
  });
  writeManifest(root, input);

  runCli("refresh-codex-skill-lock.mjs", ["--repo-root", root]);

  const actual = JSON.parse(readFileSync(join(root, "agents", "skill-routing.json"), "utf8"));
  assert.notEqual(actual.skills[0].source.tree_sha256, input.skills[0].source.tree_sha256);
  assert.equal(actual.skills[0].source.file_count, 1);
  actual.skills[0].source.tree_sha256 = input.skills[0].source.tree_sha256;
  actual.skills[0].source.file_count = input.skills[0].source.file_count;
  assert.deepEqual(actual, input);
});

test("render check mode performs no writes", (t) => {
  const root = fixtureRepository(t);
  const input = manifest({ skills: [lockedSkill(root, "skill:alpha")], roles: [fixtureRole()] });
  writeManifest(root, input);
  const roleText = "# role: builder\n5 Skills/Tools (legacy)\n6 Output format: report.\n";
  write(root, "agents/roles/builder.md", roleText);

  assert.throws(
    () => runCli("render-codex-contract.mjs", ["--check", "--repo-root", root]),
    (error) => `${error.stdout}\n${error.stderr}`.includes("GENERATED_FILE_MISSING"),
  );
  assert.equal(readFileSync(join(root, "agents", "roles", "builder.md"), "utf8"), roleText);
  assert.equal(existsSync(join(root, ".agents")), false);
  assert.equal(existsSync(join(root, ".codex")), false);
});

test("render write mode repairs marked generated drift", (t) => {
  const root = fixtureRepository(t);
  const alpha = lockedSkill(root, "skill:alpha");
  const role = fixtureRole();
  const input = manifest({ skills: [alpha], roles: [role] });
  writeManifest(root, input);
  write(root, role.role_path, "# role: builder\n6 Output format: report.\n");

  runCli("render-codex-contract.mjs", ["--write", "--repo-root", root]);
  const bridgePath = join(root, ".agents", "skills", alpha.codex_name, "SKILL.md");
  writeFileSync(bridgePath, `---\n${GENERATED_MARKERS.comment}\ndrift\n`);
  runCli("render-codex-contract.mjs", ["--write", "--repo-root", root]);

  assert.equal(readFileSync(bridgePath, "utf8"), renderBridgeSkill(input, alpha));
  assert.equal(
    readFileSync(join(root, ".agents", "skills", alpha.codex_name, "agents", "openai.yaml"), "utf8"),
    renderOpenAiMetadata(alpha),
  );
  assert.equal(
    readFileSync(join(root, ".codex", "agents", `${role.id}.toml`), "utf8"),
    renderCodexAgent(input, role),
  );
});

test("render never deletes an unmarked custom agent or skill", (t) => {
  const root = fixtureRepository(t);
  const input = manifest({ skills: [lockedSkill(root, "skill:alpha")], roles: [fixtureRole()] });
  writeManifest(root, input);
  write(root, "agents/roles/builder.md", "# role: builder\n6 Output format: report.\n");
  write(root, ".agents/skills/custom/SKILL.md", "---\nname: custom\ndescription: custom\n---\n");
  write(root, ".codex/agents/custom.toml", "name = \"custom\"\n");

  runCli("render-codex-contract.mjs", ["--write", "--repo-root", root]);

  assert.equal(existsSync(join(root, ".agents", "skills", "custom", "SKILL.md")), true);
  assert.equal(existsSync(join(root, ".codex", "agents", "custom.toml")), true);
});

test("stale marked bridge and shim are rejected", (t) => {
  const root = fixtureRepository(t);
  const input = manifest({ skills: [lockedSkill(root, "skill:alpha")], roles: [fixtureRole()] });
  writeManifest(root, input);
  write(root, "agents/roles/builder.md", "# role: builder\n6 Output format: report.\n");
  write(root, ".agents/skills/govfolio-stale/SKILL.md", `---\n${GENERATED_MARKERS.comment}\nstale\n`);
  write(root, ".codex/agents/stale.toml", `${GENERATED_MARKERS.comment}\nstale\n`);

  assert.throws(
    () => runCli("render-codex-contract.mjs", ["--check", "--repo-root", root]),
    (error) => {
      const output = `${error.stdout}\n${error.stderr}`;
      return output.includes("STALE_MARKED_OUTPUT") &&
        output.includes("govfolio-stale") && output.includes("stale.toml");
    },
  );
});

test("role generated blocks match manifest slots and trigger IDs", (t) => {
  const root = fixtureRepository(t);
  const alpha = lockedSkill(root, "skill:alpha");
  const beta = lockedSkill(root, "skill:beta");
  const role = {
    ...fixtureRole(),
    situational: [
      {
        trigger: "trigger:completion-review",
        requirements: ["skill:beta"],
      },
    ],
  };
  const input = manifest({
    skills: [alpha, beta],
    triggers: [{ id: "trigger:completion-review", description: "Review completed work." }],
    roles: [role],
  });
  writeManifest(root, input);
  write(root, role.role_path, "# role: builder\n6 Output format: report.\n");

  runCli("render-codex-contract.mjs", ["--write", "--repo-root", root]);

  const renderedRole = readFileSync(join(root, ...role.role_path.split("/")), "utf8");
  assert.ok(renderedRole.includes(renderRoleSkillBlock(input, role)));
  assert.match(renderedRole, /trigger:completion-review/);
});

test("resolver expands role, trigger, step, pack, and dependencies", (t) => {
  const root = fixtureRepository(t);
  const input = manifest({
    skills: [
      lockedSkill(root, "skill:alpha"),
      lockedSkill(root, "skill:beta"),
      lockedSkill(root, "skill:gamma", { dependencies: ["skill:delta"] }),
      lockedSkill(root, "skill:delta"),
    ],
    packs: [{ id: "pack:craft", slot_cost: 1, members: ["skill:gamma"], planned_members: [] }],
    triggers: [{ id: "trigger:completion-review", description: "Review completed work." }],
    roles: [
      {
        ...fixtureRole(),
        situational: [
          { trigger: "trigger:completion-review", requirements: ["skill:beta"] },
        ],
      },
    ],
  });
  writeManifest(root, input);
  write(
    root,
    "plan.md",
    "### Task 1: Build\n\n**Required skills:** pack:craft\n",
  );

  const output = runCli(
    "resolve-codex-dispatch.mjs",
    [
      "--repo-root", root,
      "--role", "builder",
      "--trigger", "trigger:completion-review",
      "--section-file", "plan.md",
      "--section-heading", "Task 1: Build",
      "--source-context", "docs/context.md",
    ],
    { cwd: root },
  );
  const envelope = JSON.parse(output.split(/\r?\n/)[1]);
  assert.deepEqual(envelope.skills.map(({ id }) => id), [
    "skill:alpha",
    "skill:beta",
    "skill:delta",
    "skill:gamma",
  ]);
});

test("resolver blocks a triggered planned skill", (t) => {
  const root = fixtureRepository(t);
  const input = manifest({
    skills: [
      lockedSkill(root, "skill:alpha"),
      skill("skill:future", {
        status: "planned",
        source: null,
        unavailable_reason: "not imported",
      }),
    ],
    triggers: [{ id: "trigger:future", description: "Require unavailable future work." }],
    roles: [
      {
        ...fixtureRole(),
        situational: [{ trigger: "trigger:future", requirements: ["skill:future"] }],
      },
    ],
  });
  writeManifest(root, input);

  assert.throws(
    () => runCli(
      "resolve-codex-dispatch.mjs",
      ["--repo-root", root, "--role", "builder", "--trigger", "trigger:future"],
      { cwd: root },
    ),
    (error) => `${error.stdout}\n${error.stderr}`.includes("PLANNED_SKILL_REQUIRED"),
  );
});

test("impeccable bridge preserves docs-only script restrictions", () => {
  const repositoryRoot = join(scriptDirectory, "..", "..");
  const input = JSON.parse(
    readFileSync(join(repositoryRoot, "agents", "skill-routing.json"), "utf8"),
  );
  const impeccable = input.skills.find(({ id }) => id === "skill:impeccable");
  assert.ok(impeccable);
  assert.ok(
    impeccable.restrictions.some((restriction) =>
      /never.*(?:execute|import|copy|adapt|vendor).*scripts/i.test(restriction),
    ),
  );
  const rendered = renderBridgeSkill(input, impeccable);
  for (const restriction of impeccable.restrictions) assert.ok(rendered.includes(restriction));
});

test("nested manifest constraints fail closed", (t) => {
  const root = fixtureRepository(t);
  const alpha = lockedSkill(root, "skill:alpha");
  alpha.source.unexpected = true;
  const future = skill("skill:future", {
    status: "planned",
    source: null,
    unavailable_reason: "not imported",
  });
  const repeatedActive = Array.from({ length: 7 }, () => "skill:alpha");
  const input = manifest({
    skills: [alpha, future],
    packs: [
      {
        id: "pack:broken",
        slot_cost: 2,
        members: ["skill:alpha", "skill:future", "skill:alpha", "skill:alpha"],
        planned_members: ["skill:alpha"],
      },
    ],
    triggers: [{ id: "not-namespaced", description: "" }],
    roles: [
      {
        ...fixtureRole("builder-one"),
        role_path: "agents/roles/shared.md",
        active: repeatedActive,
      },
      {
        ...fixtureRole("builder-two"),
        role_path: "agents/roles/shared.md",
      },
    ],
  });

  const codes = new Set(validateManifest(input, root).map(({ code }) => code));
  for (const expected of [
    "DUPLICATE_PACK_MEMBER",
    "DUPLICATE_ROLE_PATH",
    "INVALID_PACK_SIZE",
    "INVALID_PACK_SLOT_COST",
    "INVALID_SOURCE_KEYS",
    "INVALID_TRIGGER_DESCRIPTION",
    "INVALID_TRIGGER_ID",
    "PACK_MEMBER_UNAVAILABLE",
    "ROLE_SLOT_LIMIT_EXCEEDED",
  ]) {
    assert.ok(codes.has(expected), `missing diagnostic ${expected}`);
  }
});

test("validator rejects governed role-set drift", (t) => {
  const root = fixtureRepository(t);
  const input = manifest({
    skills: [lockedSkill(root, "skill:alpha")],
    roles: [fixtureRole()],
  });
  writeManifest(root, input);
  write(root, "agents/roles/builder.md", "# role: builder\n6 Output format: report.\n");
  runCli("render-codex-contract.mjs", ["--write", "--repo-root", root]);
  write(root, ".claude/agents/other.md", "---\nname: other\n---\n");

  assert.throws(
    () => runCli("validate-codex-contract.mjs", ["--repo-root", root]),
    (error) => `${error.stdout}\n${error.stderr}`.includes("ROLE_SET_MISMATCH"),
  );
});

test("expected generated bridge rejects unowned extra files", (t) => {
  const root = fixtureRepository(t);
  const input = manifest({
    skills: [lockedSkill(root, "skill:alpha")],
    roles: [fixtureRole()],
  });
  writeManifest(root, input);
  write(root, "agents/roles/builder.md", "# role: builder\n6 Output format: report.\n");
  runCli("render-codex-contract.mjs", ["--write", "--repo-root", root]);
  write(root, ".agents/skills/govfolio-alpha/notes.md", "unowned procedure text\n");

  assert.throws(
    () => runCli("render-codex-contract.mjs", ["--check", "--repo-root", root]),
    (error) => `${error.stdout}\n${error.stderr}`.includes("UNOWNED_BRIDGE_EXTRA"),
  );
  assert.equal(
    readFileSync(join(root, ".agents", "skills", "govfolio-alpha", "notes.md"), "utf8"),
    "unowned procedure text\n",
  );
});

test("formatted diagnostics always use the stable contract shape", () => {
  const output = formatDiagnostics([
    {
      code: "EXAMPLE_FAILURE",
      message: "example",
      path: "example.md",
      repair: "repair example",
    },
  ]);
  const parsed = JSON.parse(output.replace(/^BLOCKED\(skill-contract\): /, ""));
  assert.deepEqual(Object.keys(parsed), [
    "code",
    "message",
    "path",
    "role",
    "skill",
    "expected",
    "actual",
    "repair",
  ]);
  assert.equal(parsed.role, null);
  assert.equal(parsed.skill, null);
  assert.equal(parsed.expected, null);
  assert.equal(parsed.actual, null);
});

test("root and factory prompts require resolver output and native Codex roles", () => {
  for (const path of ["agents/PROMPT.md", "agents/PROMPT-FACTORY-LANE.md"]) {
    const contents = repositoryFile(path);
    assert.match(contents, /resolve-codex-dispatch\.mjs/);
    assert.match(contents, /unmodified.*GOVFOLIO_DISPATCH_V1/i);
    assert.match(contents, /\.codex\/agents\/<role>\.toml/);
    assert.match(contents, /\.claude\/agents\/<role>/);
  }
});

test("orchestration rejects missing envelope and receipt", () => {
  const orchestration = repositoryFile("agents/workflows/orchestration.md");
  const contract = repositoryFile("agents/workflows/skill-dispatch-contract.md");
  for (const contents of [orchestration, contract]) {
    assert.match(contents, /SKILLS_LOADED/);
    assert.match(contents, /BLOCKED\(skill-contract\)/);
    assert.match(contents, /missing.*(?:envelope|receipt).*(?:hard failure|do no task work)/i);
  }
});

test("chassis propagates the contract to nested dispatch", () => {
  const contents = repositoryFile("agents/archetypes/_CHASSIS.md");
  assert.match(contents, /pre-slot skill-contract precondition/i);
  assert.match(contents, /every nested dispatch/i);
  assert.match(contents, /SKILLS_LOADED/);
  assert.match(contents, /BLOCKED\(skill-contract\)/);
});

test("governance declares manifest authority and generated projections", () => {
  const contents = repositoryFile("agents/GOVERNANCE.md");
  assert.match(contents, /agents\/skill-routing\.json.*authoritative/i);
  assert.match(contents, /\.agents\/skills.*generated projection/i);
  assert.match(contents, /\.codex\/agents.*generated projection/i);
  assert.match(contents, /never.*hand-edit/i);
});

test("known plan and goal sub-skills use Required skills fields", () => {
  const expectations = new Map([
    ["docs/plans/2026-07-04-govfolio-implementation.md", "**Required skills:** skill:executing-plans"],
    ["docs/plans/2026-07-07-consensus-extraction.md", "**Required skills:** skill:subagent-driven-development"],
    ["docs/plans/2026-07-07-consensus-hardening.md", "**Required skills:** skill:subagent-driven-development"],
    ["docs/plans/2026-07-09-filing-document-viewer-implementation.md", "**Required skills:** skill:subagent-driven-development"],
    ["agents/goals/021-llm-extraction.md", "**Required skills:** skill:plan-decomposition, skill:writing-plans"],
  ]);
  for (const [path, marker] of expectations) {
    assert.ok(repositoryFile(path).includes(marker), `${path} is missing ${marker}`);
  }
});

test("real plan task headings inherit their governed workflow skill", () => {
  const expectations = [
    [
      "docs/plans/2026-07-04-govfolio-implementation.md",
      "Task 1: Cargo workspace + lint regime + smoke test",
      "skill:executing-plans",
    ],
    [
      "docs/plans/2026-07-07-consensus-extraction.md",
      "Task 1: Consensus DTOs + content-key derivation",
      "skill:subagent-driven-development",
    ],
    [
      "docs/plans/2026-07-07-consensus-hardening.md",
      "Task H27: Frontier re-check + amendment reclassification",
      "skill:subagent-driven-development",
    ],
    [
      "docs/plans/2026-07-09-filing-document-viewer-implementation.md",
      "Task 1: Fix `mime_type` — sniff real content instead of hardcoding `application/pdf`",
      "skill:subagent-driven-development",
    ],
  ];
  for (const [path, heading, required] of expectations) {
    const parsed = parseRequiredSkills(repositoryFile(path), path, heading);
    assert.deepEqual(parsed.diagnostics, [], path);
    assert.ok(parsed.requirements.includes(required), `${path} did not inherit ${required}`);
  }
});

test("tracked AGENTS requires CLAUDE and the dispatch contract", () => {
  const contents = repositoryFile("AGENTS.md");
  assert.match(contents, /read.*CLAUDE\.md.*completely/i);
  assert.match(contents, /agents\/workflows\/skill-dispatch-contract\.md/);
  assert.match(contents, /resolve-codex-dispatch\.mjs/);
  assert.match(contents, /only permitted pre-receipt actions/i);
  assert.match(contents, /every nested dispatch/i);
});

test("every pre-receipt boundary permits the mandatory CLAUDE read", () => {
  const paths = [
    "AGENTS.md",
    "agents/workflows/skill-dispatch-contract.md",
    ".codex/agents/rust-builder.toml",
  ];
  for (const path of paths) {
    assert.match(
      repositoryFile(path),
      /pre-receipt[\s\S]{0,500}CLAUDE\.md/i,
      `${path} omits CLAUDE.md from its pre-receipt boundary`,
    );
  }
});

function gitBash() {
  const candidates = process.platform === "win32"
    ? [
        "C:\\Program Files\\Git\\bin\\bash.exe",
        "C:\\Program Files\\Git\\usr\\bin\\bash.exe",
      ]
    : ["bash"];
  const candidate = candidates.find((path) => path === "bash" || existsSync(path));
  if (!candidate) throw new Error("Git Bash is required for runner contract tests");
  return candidate;
}

function bashPath(path) {
  if (process.platform !== "win32") return path;
  return path.replace(/^([A-Za-z]):[\\/]/, (_, drive) => `/${drive.toLowerCase()}/`).replaceAll("\\", "/");
}

function cleanVerifierFixture(t, { trackMachineConfig = false } = {}) {
  const root = fixtureRepository(t);
  const verifier = repositoryFile("scripts/agents/check-codex-contract-clean-worktree.mjs");
  write(root, "scripts/agents/check-codex-contract-clean-worktree.mjs", verifier);
  write(root, "scripts/agents/render-codex-contract.mjs", "process.exit(0);\n");
  write(root, "scripts/agents/validate-codex-contract.mjs", "process.exit(0);\n");
  write(root, "AGENTS.md", "# Fixture agent contract\n");
  write(root, "agents/run-loop-codex.sh", "#!/usr/bin/env bash\nexit 0\n");
  chmodSync(join(root, "agents", "run-loop-codex.sh"), 0o755);
  write(root, ".agents/skills/govfolio-alpha/SKILL.md", "---\nname: govfolio-alpha\ndescription: fixture\n---\n");
  write(root, ".agents/skills/govfolio-alpha/agents/openai.yaml", "interface: {}\n");
  write(root, ".codex/agents/builder.toml", "name = \"builder\"\n");
  writeManifest(root, manifest({
    skills: [skill("skill:alpha")],
    roles: [fixtureRole()],
  }));
  if (trackMachineConfig) write(root, ".codex/config.toml", "model = \"local-only\"\n");
  execFileSync("git", ["add", "."], { cwd: root });
  execFileSync("git", ["update-index", "--chmod=+x", "agents/run-loop-codex.sh"], {
    cwd: root,
  });
  execFileSync("git", ["commit", "--quiet", "-m", "fixture"], { cwd: root });
  return root;
}

test("Codex compatibility runner never invokes the provider directly", () => {
  const contents = repositoryFile("agents/run-loop-codex.sh");
  assert.doesNotMatch(contents, /(?:^|\|)\s*codex\s+/m);
  assert.match(contents, /exec "\$ROOT\/agents\/run-loop\.sh" "\$@"/);
  assert.doesNotMatch(contents, /\r\n/);
});

test("Codex compatibility runner validates before supervisor delegation", () => {
  const contents = repositoryFile("agents/run-loop-codex.sh");
  const render = contents.indexOf("render-codex-contract.mjs");
  const validate = contents.indexOf("validate-codex-contract.mjs");
  const delegate = contents.indexOf('exec "$ROOT/agents/run-loop.sh"');
  assert.ok(render >= 0 && validate > render && delegate > validate);
});

test("Rust Codex adapter enforces depth two and thread bound six", () => {
  const contents = repositoryFile("crates/supervisor/src/provider/codex.rs");
  assert.match(contents, /"agents\.max_depth=2"/);
  assert.match(contents, /"agents\.max_threads=6"/);
});

test("provider looping is owned by the fenced Rust supervisor", () => {
  const contents = repositoryFile("agents/run-loop-codex.sh");
  assert.doesNotMatch(contents, /while\s+:/);
  const supervisor = repositoryFile("crates/supervisor/src/supervisor.rs");
  assert.match(supervisor, /async fn run\(once: bool\)/);
  assert.match(supervisor, /acquire_supervisor/);
});

test("runner preflight-only mode never invokes a stub Codex binary", (t) => {
  const stub = mkdtempSync(join(tmpdir(), "govfolio-codex-stub-"));
  t.after(() => rmSync(stub, { recursive: true, force: true }));
  const marker = join(stub, "invoked");
  const stubPath = write(stub, "codex", "#!/usr/bin/env bash\nprintf invoked >\"$CODEX_STUB_MARKER\"\nexit 97\n");
  chmodSync(stubPath, 0o755);
  const output = execFileSync(
    gitBash(),
    [
      "-c",
      'PATH="$1:$PATH" GOVFOLIO_CODEX_PREFLIGHT_ONLY=1 GOVFOLIO_LANES=1 CODEX_STUB_MARKER="$2" bash agents/run-loop-codex.sh',
      "contract-test",
      bashPath(stub),
      bashPath(marker),
    ],
    { cwd: repositoryRoot, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] },
  );
  assert.match(output, /Codex contract preflight passed/i);
  assert.equal(existsSync(marker), false);
});

test("tracked machine Codex config is forbidden", (t) => {
  const root = cleanVerifierFixture(t, { trackMachineConfig: true });
  assert.throws(
    () => runCli("check-codex-contract-clean-worktree.mjs", ["--repo-root", root]),
    (error) => `${error.stdout}\n${error.stderr}`.includes(".codex/config.toml"),
  );
});

test("clean detached worktree validates without mutation", (t) => {
  const root = cleanVerifierFixture(t);
  const before = execFileSync("git", ["worktree", "list", "--porcelain"], {
    cwd: root,
    encoding: "utf8",
  });
  const output = runCli("check-codex-contract-clean-worktree.mjs", ["--repo-root", root]);
  const after = execFileSync("git", ["worktree", "list", "--porcelain"], {
    cwd: root,
    encoding: "utf8",
  });
  assert.match(output, /clean detached worktree passed/i);
  assert.equal(after, before);
  assert.equal(
    execFileSync("git", ["status", "--porcelain", "--untracked-files=all"], {
      cwd: root,
      encoding: "utf8",
    }),
    "",
  );
});
