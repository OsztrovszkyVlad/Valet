#!/usr/bin/env bash
# Runs the UI smoke test (tests/e2e/smoke.mjs) on Linux under Xvfb.
#
#   scripts/e2e-linux.sh <app binary> [screenshots dir]
#
# Builds nothing. Prerequisites: the app built with embedded assets
#   (pnpm build in apps/desktop, then
#    cargo build -p app --features tauri/custom-protocol),
# tauri-driver (cargo install tauri-driver --locked --version 2.0.6),
# WebKitWebDriver (apt: webkit2gtk-driver), Xvfb (apt: xvfb), Node 22.
# If DISPLAY is unset, starts Xvfb on :99 and stops it on exit.
set -euo pipefail

app=${1:?usage: scripts/e2e-linux.sh <app binary> [screenshots dir]}
shots=${2:-}
repo=$(cd "$(dirname "$0")/.." && pwd)

for tool in tauri-driver WebKitWebDriver node; do
  command -v "$tool" >/dev/null || { echo "::error title=e2e setup::$tool not found on PATH"; exit 1; }
done

xvfb_pid=
if [ -z "${DISPLAY:-}" ]; then
  command -v Xvfb >/dev/null || { echo "::error title=e2e setup::Xvfb not found and DISPLAY unset"; exit 1; }
  Xvfb :99 -screen 0 1280x1024x24 -nolisten tcp >/dev/null 2>&1 &
  xvfb_pid=$!
  export DISPLAY=:99
  trap 'kill "$xvfb_pid" 2>/dev/null || true' EXIT
  for _ in $(seq 50); do
    [ -e /tmp/.X11-unix/X99 ] && break
    sleep 0.1
  done
fi

args=(--app "$app")
[ -n "$shots" ] && args+=(--screenshots "$shots")
node "$repo/tests/e2e/smoke.mjs" "${args[@]}"
