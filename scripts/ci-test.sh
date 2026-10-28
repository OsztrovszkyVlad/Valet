#!/usr/bin/env bash
# Runs `cargo test <args>` and, on failure, emits one GitHub Actions error
# annotation per failing test with the first lines of its output (annotations
# are readable through the checks API when the raw log cannot be downloaded;
# GitHub keeps at most 10 per step).
set -o pipefail
log=$(mktemp)
cargo test --no-fail-fast "$@" 2>&1 | tee "$log"
code=${PIPESTATUS[0]}
if [ "$code" -ne 0 ]; then
  awk '
    function flush() { if (msg != "") { print "::error::" msg; msg = "" } }
    /^---- .* stdout ----$/ { flush(); msg = $0; n = 0; grab = 1; next }
    /^failures:$/ || /^test result:/ { flush(); grab = 0 }
    grab && n < 25 { gsub(/%/, "%25"); msg = msg "%0A" $0; n++ }
    END { flush() }
  ' "$log" | head -10
fi
exit "$code"
