# Refuses to stop a phase run if tests were not executed in this session. qa or PM must run `cargo test` and record it.
$input_json = [Console]::In.ReadToEnd()
try { $data = $input_json | ConvertFrom-Json } catch { exit 0 }
if ($data.stop_hook_active -eq $true) { exit 0 }   # avoid loops
$marker = '.claude/state/tests-ran.txt'
if (-not (Test-Path $marker)) { exit 0 }           # not in a phase run (marker is created by the phase skill)
$content = Get-Content $marker -Raw
if ($content -notmatch 'cargo test.*exit=0') {
  [Console]::Error.WriteLine("Do not stop yet: cargo test has not passed in this phase run. Run the full test suite and record the result in .claude/state/tests-ran.txt (line: 'cargo test exit=0').")
  exit 2
}
exit 0
