#!/usr/bin/env bash
# Runs `cargo test <args>` and, on failure, turns failing test names and panic
# messages into GitHub Actions error annotations (readable through the checks
# API when the raw log cannot be downloaded).
set -o pipefail
log=$(mktemp)
cargo test --no-fail-fast "$@" 2>&1 | tee "$log"
code=${PIPESTATUS[0]}
if [ "$code" -ne 0 ]; then
  grep -E "^test .* FAILED$|panicked at|^---- .* stdout ----$|^error(\[|:)" "$log" | head -40 |
    while IFS= read -r line; do printf '::error::%s\n' "${line//%/%25}"; done
  awk '/^---- .* stdout ----$/{p=1;n=0} p&&n<12{print "::error::" $0; n++}' "$log" | head -80
fi
exit "$code"
