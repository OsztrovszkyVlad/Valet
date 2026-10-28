#!/usr/bin/env bash
# Refuses to stop a phase run if tests were not executed in this session. qa or PM must run `cargo test` and record it.
source "$(dirname "$0")/lib.sh"

[ "$(json_get stop_hook_active)" = "true" ] && exit 0   # avoid loops
marker=.claude/state/tests-ran.txt
[ -f "$marker" ] || exit 0                              # not in a phase run (marker is created by the phase skill)
if ! grep -qE 'cargo test.*exit=0' "$marker"; then
  block "Do not stop yet: cargo test has not passed in this phase run. Run the full test suite and record the result in .claude/state/tests-ran.txt (line: 'cargo test exit=0')."
fi
exit 0
