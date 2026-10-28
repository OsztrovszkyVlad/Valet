#!/usr/bin/env bash
# Blocks edits to protected files and forbidden filesystem calls in Rust sources. Exit 2 = block.
source "$(dirname "$0")/lib.sh"

path=$(json_get tool_input.file_path)
[ -z "$path" ] && exit 0

matches() { printf '%s' "$1" | grep -qiP -- "$2"; }

for p in '\.env' '\.claude[\\/]settings\.json$' '\.claude[\\/]hooks[\\/]' \
         '\.github[\\/]workflows[\\/]release\.yml$' '[\\/]keys?[\\/]' '\.key$' 'minisign'; do
  if matches "$path" "$p"; then
    block "Blocked: $path is protected. Ask Vlad to change it."
  fi
done

new=$(json_get tool_input.new_string)
[ -z "$new" ] && new=$(json_get tool_input.content)

# tauri.conf.json: allow edits, but not to updater/signing sections.
if matches "$path" 'tauri\.conf\.json$' && [ -n "$new" ] &&
   matches "$new" '"updater"|"pubkey"|"certificateThumbprint"|"signCommand"'; then
  block "Blocked: updater/signing config in tauri.conf.json is changed by Vlad only."
fi

# Forbidden destructive fs calls in non-test Rust code.
if matches "$path" '\.rs$' && ! matches "$path" '[\\/]tests?[\\/]' && ! matches "$path" '_test\.rs$' &&
   [ -n "$new" ] && matches "$new" 'fs::remove_file|fs::remove_dir|remove_dir_all|std::fs::remove'; then
  block "Blocked: permanent deletion is forbidden in product code (CLAUDE.md invariant 1). Use the trash crate."
fi
exit 0
