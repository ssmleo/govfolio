#!/usr/bin/env bash
# Codex compatibility launcher. This script performs only trusted contract and
# linked-worktree validation; the Rust supervisor exclusively owns providers,
# process trees, fencing, retries, and failover (goals 108-110).
set -Eeuo pipefail
cd "$(dirname "$0")/.."

ROOT="$(pwd)"
LANES="${GOVFOLIO_LANES:-1}"
LANES_DIR="${GOVFOLIO_LANES_DIR:-$ROOT/../govfolio-codex-lanes}"
PREFLIGHT_ONLY="${GOVFOLIO_CODEX_PREFLIGHT_ONLY:-0}"

TRUSTED_POLICY_PATHS=(
  .gitattributes
  .gitignore
  AGENTS.md
  CLAUDE.md
  agents/EPOCHS.md
  agents/GOVERNANCE.md
  agents/LOOP.md
  agents/PROMPT.md
  agents/PROMPT-FACTORY-LANE.md
  agents/run-loop.sh
  agents/run-loop-codex.sh
  agents/skill-routing.json
  agents/skill-routing.schema.json
  scripts/agents/codex-contract-lib.mjs
  scripts/agents/render-codex-contract.mjs
  scripts/agents/validate-codex-contract.mjs
)

case "$LANES" in
  ''|*[!0-9]*|0) echo "ERROR: GOVFOLIO_LANES must be a positive integer" >&2; exit 2 ;;
esac
case "$PREFLIGHT_ONLY" in
  0|1) ;;
  *) echo "ERROR: GOVFOLIO_CODEX_PREFLIGHT_ONLY must be 0 or 1" >&2; exit 2 ;;
esac

command -v git >/dev/null 2>&1 || { echo "ERROR: git not found" >&2; exit 1; }
command -v node >/dev/null 2>&1 || { echo "ERROR: node not found" >&2; exit 1; }
git rev-parse --is-inside-work-tree >/dev/null 2>&1 || {
  echo "ERROR: not inside a Git worktree" >&2
  exit 1
}

while IFS= read -r -d '' policy_path; do
  TRUSTED_POLICY_PATHS+=("$policy_path")
done < <(git -C "$ROOT" ls-files -z -- \
  'agents/roles/*.md' \
  'agents/archetypes/*.md' \
  'agents/workflows/*.md')

require_contract_assets() {
  local worktree="$1"
  local path
  for path in "${TRUSTED_POLICY_PATHS[@]}"; do
    [ -f "$worktree/$path" ] || {
      echo "ERROR: tracked Codex contract asset missing in $worktree: $path" >&2
      return 1
    }
  done
}

forbid_tracked_machine_config() {
  local worktree="$1"
  if git -C "$worktree" ls-files --error-unmatch -- .codex/config.toml >/dev/null 2>&1; then
    echo "ERROR: .codex/config.toml is machine-specific and must never be tracked" >&2
    return 1
  fi
}

policy_hash() {
  local worktree="$1"
  local path="$2"
  git -C "$worktree" hash-object --filters --path="$path" "$worktree/$path"
}

validate_trusted_policy() {
  local worktree="$1"
  local path root_hash root_head_hash worktree_hash worktree_head_hash
  require_contract_assets "$worktree"
  forbid_tracked_machine_config "$worktree"
  for path in "${TRUSTED_POLICY_PATHS[@]}"; do
    git -C "$ROOT" ls-files --error-unmatch -- "$path" >/dev/null 2>&1 || {
      echo "ERROR: trusted root policy is untracked: $path" >&2
      return 1
    }
    git -C "$worktree" ls-files --error-unmatch -- "$path" >/dev/null 2>&1 || {
      echo "ERROR: worktree policy is untracked in $worktree: $path" >&2
      return 1
    }
    [ ! -L "$ROOT/$path" ] && [ ! -L "$worktree/$path" ] || {
      echo "ERROR: policy path must not be a symlink: $path" >&2
      return 1
    }
    root_hash="$(policy_hash "$ROOT" "$path")"
    root_head_hash="$(git -C "$ROOT" rev-parse "HEAD:$path")"
    [ "$root_hash" = "$root_head_hash" ] || {
      echo "ERROR: trusted root policy differs from HEAD: $path" >&2
      return 1
    }
    worktree_hash="$(policy_hash "$worktree" "$path")"
    worktree_head_hash="$(git -C "$worktree" rev-parse "HEAD:$path")"
    [ "$worktree_hash" = "$worktree_head_hash" ] || {
      echo "ERROR: worktree policy differs from HEAD in $worktree: $path" >&2
      return 1
    }
    if [ "$worktree" != "$ROOT" ]; then
      [ "$root_hash" = "$worktree_hash" ] || {
        echo "ERROR: lane policy differs from the trusted root: $path" >&2
        return 1
      }
    fi
  done
}

validate_contract() {
  local worktree="$1"
  validate_trusted_policy "$worktree"
  node "$ROOT/scripts/agents/render-codex-contract.mjs" --check --repo-root "$worktree"
  node "$ROOT/scripts/agents/validate-codex-contract.mjs" --repo-root "$worktree"
}

canonical_path() {
  node -e 'process.stdout.write(require("node:fs").realpathSync(process.argv[1]).replaceAll("\\\\", "/"))' "$1"
}

absolute_path() {
  node -e 'process.stdout.write(require("node:path").resolve(process.argv[1]).replaceAll("\\\\", "/"))' "$1"
}

reject_symlink_path() {
  node -e '
    const fs = require("node:fs");
    const path = require("node:path");
    let cursor = path.resolve(process.argv[1]);
    const root = path.parse(cursor).root;
    while (cursor !== root) {
      if (fs.lstatSync(cursor).isSymbolicLink()) {
        process.stderr.write(`ERROR: lane worktree path contains a symlink: ${cursor}\n`);
        process.exit(1);
      }
      cursor = path.dirname(cursor);
    }
  ' "$1"
}

verify_lane_worktree() {
  local n="$1"
  local worktree="$2"
  local expected_branch="codex/lane/$n"
  local worktree_absolute worktree_real top_level branch root_common lane_common registered line candidate
  [ -d "$worktree" ] || {
    echo "ERROR: configured Codex lane worktree missing: $worktree" >&2
    return 1
  }
  worktree_absolute="$(absolute_path "$worktree")"
  reject_symlink_path "$worktree_absolute"
  worktree="$worktree_absolute"
  worktree_real="$(canonical_path "$worktree")"
  top_level="$(canonical_path "$(git -C "$worktree" rev-parse --show-toplevel)")"
  [ "$top_level" = "$worktree_real" ] || {
    echo "ERROR: lane top-level mismatch: expected $worktree_real, got $top_level" >&2
    return 1
  }
  branch="$(git -C "$worktree" symbolic-ref --short HEAD 2>/dev/null || true)"
  [ "$branch" = "$expected_branch" ] || {
    echo "ERROR: lane branch mismatch: expected $expected_branch, got ${branch:-detached}" >&2
    return 1
  }
  root_common="$(canonical_path "$(git -C "$ROOT" rev-parse --path-format=absolute --git-common-dir)")"
  lane_common="$(canonical_path "$(git -C "$worktree" rev-parse --path-format=absolute --git-common-dir)")"
  [ "$root_common" = "$lane_common" ] || {
    echo "ERROR: lane does not share the trusted root's common Git directory" >&2
    return 1
  }
  registered=0
  while IFS= read -r line; do
    case "$line" in
      "worktree "*)
        candidate="${line#worktree }"
        if [ -d "$candidate" ] && [ "$(canonical_path "$candidate")" = "$worktree_real" ]; then
          registered=1
        fi
        ;;
    esac
  done < <(git -C "$ROOT" worktree list --porcelain)
  [ "$registered" = "1" ] || {
    echo "ERROR: lane is not a registered worktree of the trusted root: $worktree" >&2
    return 1
  }
}

validate_contract "$ROOT"
n=1
while [ "$n" -lt "$LANES" ]; do
  worktree="$LANES_DIR/lane-$n"
  verify_lane_worktree "$n" "$worktree"
  validate_contract "$worktree"
  n=$((n + 1))
done

if [ "$PREFLIGHT_ONLY" = "1" ]; then
  echo "Codex contract preflight passed for the primary checkout and configured lane worktrees."
  exit 0
fi

export GOVFOLIO_LOOP_PROVIDER=codex
exec "$ROOT/agents/run-loop.sh" "$@"
