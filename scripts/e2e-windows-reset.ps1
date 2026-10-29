# Windows e2e: stop the app and move its config, WebView2 data and the
# Downloads contents into RUNNER_TEMP (no deletes), so the next smoke run is
# a first run on an empty Downloads folder.
param([Parameter(Mandatory = $true)][string]$Tag)
$ErrorActionPreference = 'Continue'
Get-Process app -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep -Seconds 2
$dest = Join-Path $env:RUNNER_TEMP "e2e-reset-$Tag"
New-Item -ItemType Directory -Force -Path $dest | Out-Null
foreach ($d in @("$env:APPDATA\ro.valetapp.desktop", "$env:LOCALAPPDATA\ro.valetapp.desktop")) {
    if (Test-Path $d) {
        Move-Item -Path $d -Destination (Join-Path $dest ("appdata-" + (Split-Path -Leaf (Split-Path -Parent $d))))
    }
}
$downloads = Join-Path $env:USERPROFILE 'Downloads'
if (Test-Path $downloads) {
    $to = Join-Path $dest 'Downloads'
    New-Item -ItemType Directory -Force -Path $to | Out-Null
    Get-ChildItem -Force -Path $downloads | Move-Item -Destination $to
}
