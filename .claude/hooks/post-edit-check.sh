#!/usr/bin/env bash
# After editing a Rust file, run cargo check on the crate that owns it.
# On failure the output goes to Claude (exit 2 on PostToolUse shows stderr; the edit is kept).
source "$(dirname "$0")/lib.sh"

path=$(json_get tool_input.file_path)
case "$path" in *.rs) ;; *) exit 0 ;; esac

crate=
if printf '%s' "$path" | grep -qP 'crates[\\/]valet-core[\\/]'; then crate=valet-core
elif printf '%s' "$path" | grep -qP 'crates[\\/]valet-platform[\\/]'; then crate=valet-platform
elif printf '%s' "$path" | grep -qP 'src-tauri[\\/]'; then crate=app
fi
[ -z "$crate" ] && exit 0

out=$(cargo check -p "$crate" --message-format short 2>&1)
code=$?
mkdir -p .claude/state
printf '%s %s exit=%s\n' "$(date +%Y-%m-%dT%H:%M:%S)" "$crate" "$code" > .claude/state/last-cargo-check.txt
if [ "$code" -ne 0 ]; then
  { echo "cargo check -p $crate FAILED. Fix before continuing:"; printf '%s\n' "$out" | tail -n 40; } >&2
  exit 2
fi
exit 0
