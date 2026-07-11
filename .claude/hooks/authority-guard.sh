#!/usr/bin/env sh
# PreToolUse hook (goal 100, design §4.2). Rust owns path policy; this thin
# adapter validates the hook payload, distinguishes reads from writes, and
# applies the cheap amendment-branch condition. Every ambiguity exits 2.
set -u

deny() {
  echo "authority-guard: $*" >&2
  exit 2
}

command -v node >/dev/null 2>&1 \
  || deny "node is required to parse the hook payload -- failing closed"

if ! parsed="$(node -e '
let data = "";
process.stdin.on("data", chunk => { data += chunk; });
process.stdin.on("end", () => {
  try {
    const payload = JSON.parse(data);
    const tool = payload && payload.tool_name;
    const path = payload && payload.tool_input && payload.tool_input.file_path;
    if (!["Read", "Write", "Edit"].includes(tool)
        || typeof path !== "string"
        || path.length === 0
        || /[\t\r\n]/.test(path)) {
      process.exit(2);
    }
    process.stdout.write(`${tool}\t${path}`);
  } catch (_) {
    process.exit(2);
  }
});
' 2>/dev/null)"; then
  deny "malformed or incomplete Read/Write/Edit hook payload -- failing closed"
fi

tab="$(printf '\t')"
tool_name=${parsed%%"$tab"*}
file_path=${parsed#*"$tab"}

root="${CLAUDE_PROJECT_DIR:-$PWD}"
target_dir="${CARGO_TARGET_DIR:-target}"
case "$target_dir" in
  /* | [A-Za-z]:*) ;;
  *) target_dir="$root/$target_dir" ;;
esac
bin="$target_dir/debug/validate-authority"
[ -x "$bin" ] || bin="$bin.exe"
[ -x "$bin" ] \
  || deny "pre-built validate-authority is unavailable at $bin -- denying $file_path"

out="$(cd "$root" && "$bin" --check-path "$file_path" 2>&1)"
code=$?
if [ "$code" -eq 0 ]; then
  exit 0
fi
if [ "$code" -ne 2 ]; then
  deny "validate-authority --check-path exited $code unexpectedly: $out -- failing closed"
fi

case "$out" in
  DENY\ untrusted-goal\ *)
    deny "$out -- unlisted goal files are untrusted for every tool"
    ;;
  DENY\ protected\ *)
    if [ "$tool_name" = "Read" ]; then
      exit 0
    fi
    command -v git >/dev/null 2>&1 \
      || deny "git is required to verify the amendment branch -- failing closed"
    if ! branch="$(git -C "$root" symbolic-ref --quiet --short HEAD 2>/dev/null)"; then
      deny "protected write denied on detached or unreadable HEAD: $file_path"
    fi
    case "$branch" in
      authority/*) exit 0 ;;
      *)
        deny "$out -- protected writes require an authority/* branch; the commit must update agents/AUTHORITY.lock.json and reference an INDEX-listed goal"
        ;;
    esac
    ;;
  *)
    deny "unexpected validator deny verdict: $out -- failing closed"
    ;;
esac
