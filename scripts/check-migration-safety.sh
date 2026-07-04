#!/usr/bin/env sh
# Fail closed on destructive DDL in pending migrations. Additive = auto-apply.
set -eu
DIR="${1:-crates/core/migrations}"
if grep -rniE '\b(drop|truncate)\b|alter[[:space:]]+table[^;]*\bdrop\b' "$DIR" 2>/dev/null; then
  echo "BLOCKED: destructive DDL found -> convert to a reviewed work item (auto-halt)."
  exit 1
fi
echo "migrations expand-only: safe to auto-apply"
