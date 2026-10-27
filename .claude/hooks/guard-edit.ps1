# Blocks edits to protected files and forbidden filesystem calls in Rust sources. Exit 2 = block.
$input_json = [Console]::In.ReadToEnd()
try { $data = $input_json | ConvertFrom-Json } catch { exit 0 }
$path = $data.tool_input.file_path
if (-not $path) { exit 0 }

$protected = @(
  '\.env',
  '\.claude[\\/]settings\.json$',
  '\.claude[\\/]hooks[\\/]',
  '\.github[\\/]workflows[\\/]release\.yml$',
  '[\\/]keys?[\\/]',
  '\.key$',
  'minisign'
)
foreach ($p in $protected) {
  if ($path -match $p) {
    [Console]::Error.WriteLine("Blocked: $path is protected. Ask Vlad to change it.")
    exit 2
  }
}

# tauri.conf.json: allow edits, but not to updater/signing sections, unless the content clearly keeps them.
if ($path -match 'tauri\.conf\.json$') {
  $new = $data.tool_input.new_string
  if (-not $new) { $new = $data.tool_input.content }
  if ($new -and ($new -match '"updater"|"pubkey"|"certificateThumbprint"|"signCommand"')) {
    [Console]::Error.WriteLine("Blocked: updater/signing config in tauri.conf.json is changed by Vlad only.")
    exit 2
  }
}

# Forbidden destructive fs calls in non-test Rust code.
if ($path -match '\.rs$' -and $path -notmatch '[\\/]tests?[\\/]' -and $path -notmatch '_test\.rs$') {
  $new = $data.tool_input.new_string
  if (-not $new) { $new = $data.tool_input.content }
  if ($new -and ($new -match 'fs::remove_file|fs::remove_dir|remove_dir_all|std::fs::remove')) {
    [Console]::Error.WriteLine("Blocked: permanent deletion is forbidden in product code (CLAUDE.md invariant 1). Use the trash crate.")
    exit 2
  }
}
exit 0
