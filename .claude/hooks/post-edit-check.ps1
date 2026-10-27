# After editing a Rust file, run cargo check on the crate that owns it. Non-blocking (exit 0), but the output reaches Claude.
$input_json = [Console]::In.ReadToEnd()
try { $data = $input_json | ConvertFrom-Json } catch { exit 0 }
$path = $data.tool_input.file_path
if (-not $path -or $path -notmatch '\.rs$') { exit 0 }

$crate = $null
if ($path -match 'crates[\\/]valet-core[\\/]')     { $crate = 'valet-core' }
elseif ($path -match 'crates[\\/]valet-platform[\\/]') { $crate = 'valet-platform' }
elseif ($path -match 'src-tauri[\\/]')             { $crate = 'valet-desktop' }
if (-not $crate) { exit 0 }

$out = & cargo check -p $crate --message-format short 2>&1
$code = $LASTEXITCODE
New-Item -ItemType Directory -Force -Path .claude/state | Out-Null
Set-Content -Path .claude/state/last-cargo-check.txt -Value ("{0} {1} exit={2}" -f (Get-Date -Format s), $crate, $code)
if ($code -ne 0) {
  Write-Output "cargo check -p $crate FAILED. Fix before continuing:"
  Write-Output ($out | Select-Object -Last 40)
} else {
  Write-Output "cargo check -p $crate ok"
}
exit 0
