# Shared helpers for the Valet hooks. Sourced, not executed.
# Hooks read one JSON object on stdin. JSON is parsed with jq when present, otherwise node
# (node is required by the project for pnpm, so one of them is always available).

cd "${CLAUDE_PROJECT_DIR:-.}" || exit 0

HOOK_INPUT=$(cat)

# json_get <dotted.path>: prints the string at that path, or nothing.
json_get() {
  if command -v jq >/dev/null 2>&1; then
    printf '%s' "$HOOK_INPUT" | jq -r ".$1 // empty" 2>/dev/null
  elif command -v node >/dev/null 2>&1; then
    printf '%s' "$HOOK_INPUT" | node -e '
      let s = ""; process.stdin.on("data", d => s += d).on("end", () => {
        try {
          let v = JSON.parse(s);
          for (const k of process.argv[1].split(".")) v = v == null ? undefined : v[k];
          if (v !== undefined && v !== null) process.stdout.write(typeof v === "string" ? v : JSON.stringify(v));
        } catch (e) {}
      });' "$1"
  fi
}

# block <message>: report to Claude on stderr and block the tool call (exit 2).
block() {
  printf '%s\n' "$1" >&2
  exit 2
}
