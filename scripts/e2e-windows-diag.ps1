# Diagnostics for the Windows e2e smoke job (report only). Prints what it
# finds as GitHub annotations, because the job logs are not always at hand:
#   A. app.exe standalone: alive after 15 s, top-level window, WebView2
#      browser process and its command line, app output.
#   C. msedgedriver alone (verbose log) with the capabilities tauri-driver
#      sends: session result, processes during the launch, driver log lines.
# -Cases picks which ones run (default A,C).
# Findings that shaped the e2e build (CI runs of 9968bac and 1350d62):
#   - WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS (how msedgedriver passes
#     --remote-debugging-port=0) does not reach msedgewebview2.exe, because
#     wry sets AdditionalBrowserArguments; WEBVIEW2_USER_DATA_FOLDER does.
#   - The policy value HKCU\Software\Policies\Microsoft\Edge\WebView2\
#     AdditionalBrowserArguments\app.exe does not reach it either.
#   - additionalBrowserArgs in the build's window config does.
# Moves the app's config and WebView2 data dirs into RUNNER_TEMP after each
# case, so the smoke test that follows starts from a first run.
param(
    [Parameter(Mandatory = $true)][string]$AppPath,
    [Parameter(Mandatory = $true)][string]$EdgeDriver,
    [string[]]$Cases = @('A', 'C')
)
$ErrorActionPreference = 'Continue'
$ProgressPreference = 'SilentlyContinue'
$app = (Resolve-Path $AppPath).Path
$tmp = Join-Path $env:RUNNER_TEMP 'e2e-diag'
New-Item -ItemType Directory -Force -Path $tmp | Out-Null
$script:resetCount = 0

function Note([string]$level, [string]$title, [string]$msg) {
    $m = ($msg -replace '%', '%25' -replace "`r", '' -replace "`n", '%0A')
    if ($m.Length -gt 7000) { $m = $m.Substring(0, 7000) + '...' }
    $t = ($title -replace '%', '%25' -replace ':', '%3A' -replace ',', '%2C')
    Write-Output "::$level title=$t::$m"
}

function Tail([string]$file, [int]$n = 25) {
    if (Test-Path $file) { return ((Get-Content $file -Tail $n) -join "`n") }
    return '(no file)'
}

function WebViewProcs {
    $all = Get-CimInstance Win32_Process -Filter "Name='msedgewebview2.exe'"
    $browser = @($all | Where-Object { $_.CommandLine -notmatch '--type=' })
    $lines = @("msedgewebview2.exe processes: $(@($all).Count), browser processes: $($browser.Count)")
    foreach ($b in $browser) {
        $cl = [string]$b.CommandLine
        if ($cl.Length -gt 900) { $cl = $cl.Substring(0, 900) + '...' }
        $lines += "pid $($b.ProcessId) parent $($b.ParentProcessId): $cl"
    }
    return ($lines -join "`n")
}

function AppState([System.Diagnostics.Process]$p) {
    $p.Refresh()
    if ($p.HasExited) { return "app EXITED with code $($p.ExitCode)" }
    return "app alive (pid $($p.Id)), MainWindowHandle=$($p.MainWindowHandle), title='$($p.MainWindowTitle)'"
}

function StopApp([System.Diagnostics.Process]$p) {
    if ($p -and -not $p.HasExited) { & taskkill.exe /PID $p.Id /T /F 2>&1 | Out-Null }
    Get-Process app -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 2
    Reset-AppData $tmp
}

# Moves the app's config and WebView2 data dirs into $dest (no deletes), so
# the next start is a first run.
function Reset-AppData([string]$dest) {
    $script:resetCount++
    foreach ($d in @("$env:APPDATA\ro.valetapp.desktop", "$env:LOCALAPPDATA\ro.valetapp.desktop")) {
        if (Test-Path $d) {
            $leaf = Split-Path -Leaf (Split-Path -Parent $d)
            Move-Item -Path $d -Destination (Join-Path $dest "appdata-$($script:resetCount)-$leaf") -ErrorAction SilentlyContinue
        }
    }
}

function StartApp([string]$tag, [hashtable]$envVars) {
    $saved = @{}
    foreach ($k in $envVars.Keys) { $saved[$k] = [Environment]::GetEnvironmentVariable($k); [Environment]::SetEnvironmentVariable($k, $envVars[$k]) }
    $p = Start-Process -FilePath $app -PassThru -RedirectStandardOutput "$tmp\$tag.out.txt" -RedirectStandardError "$tmp\$tag.err.txt"
    foreach ($k in $envVars.Keys) { [Environment]::SetEnvironmentVariable($k, $saved[$k]) }
    return $p
}

Note notice 'diag versions' ("app: $app ($((Get-Item $app).Length) bytes)`nmsedgedriver: $((& $EdgeDriver --version | Out-String).Trim())`nuser: $env:USERNAME, session: $([System.Diagnostics.Process]::GetCurrentProcess().SessionId)`nAPPDATA=$env:APPDATA`nLOCALAPPDATA=$env:LOCALAPPDATA`nWEBVIEW2_* env: $((Get-ChildItem env: | Where-Object Name -like 'WEBVIEW2*' | ForEach-Object { "$($_.Name)=$($_.Value)" }) -join '; ')")

# ---- A: standalone ----
if ($Cases -contains 'A') {
    StopApp $null
    $p = StartApp 'A' @{}
    Start-Sleep -Seconds 15
    $a = "$(AppState $p)`n$(WebViewProcs)`n--- app stdout tail ---`n$(Tail "$tmp\A.out.txt")`n--- app stderr tail ---`n$(Tail "$tmp\A.err.txt")"
    Note notice 'diag A app standalone' $a
    StopApp $p
}

# ---- C: msedgedriver launches the app like tauri-driver does ----
if ($Cases -contains 'C') {
    $clog = "$tmp\msedgedriver-C.log"
    $env:TAURI_WEBVIEW_AUTOMATION = 'true'
    $env:TAURI_AUTOMATION = 'true'
    $drv = Start-Process -FilePath $EdgeDriver -ArgumentList '--port=9515', '--verbose', "--log-path=$clog" -PassThru -WindowStyle Hidden
    Remove-Item env:TAURI_WEBVIEW_AUTOMATION, env:TAURI_AUTOMATION
    Start-Sleep -Seconds 2
    $caps = @{ capabilities = @{ alwaysMatch = @{ 'ms:edgeChromium' = $true; browserName = 'webview2'; 'ms:edgeOptions' = @{ binary = $app; args = @() } } } }
    $job = Start-Job -ScriptBlock {
        param($b)
        try {
            $r = Invoke-WebRequest -Uri 'http://127.0.0.1:9515/session' -Method Post -ContentType 'application/json' -Body $b -TimeoutSec 90 -SkipHttpErrorCheck
            "HTTP $($r.StatusCode): $($r.Content)"
        } catch { "request failed: $($_.Exception.Message)" }
    } -ArgumentList ($caps | ConvertTo-Json -Depth 10 -Compress)
    Start-Sleep -Seconds 12
    $apps = Get-CimInstance Win32_Process -Filter "Name='app.exe'" | ForEach-Object { "app.exe pid $($_.ProcessId) parent $($_.ParentProcessId): $($_.CommandLine)" }
    $during = "app processes at 12 s:`n$($apps -join "`n")`n$(WebViewProcs)"
    foreach ($ap in (Get-Process app -ErrorAction SilentlyContinue)) { $during += "`nwindow: handle=$($ap.MainWindowHandle) title='$($ap.MainWindowTitle)'" }
    $res = [string](Receive-Job -Job $job -Wait -AutoRemoveJob)
    Note notice 'diag C msedgedriver launch (during)' $during
    Note notice 'diag C msedgedriver launch (result)' "session: $($res.Substring(0, [Math]::Min(1500, [string]$res.Length)))"
    if (Test-Path $clog) {
        $interesting = Select-String -Path $clog -Pattern 'Launching|command line|DevToolsActivePort|WEBVIEW2|user-data-dir|remote-debugging|ERROR|SEVERE|exited|crash|webview' | Select-Object -First 40 | ForEach-Object { $_.Line.Substring(0, [Math]::Min(600, $_.Line.Length)) }
        Note notice 'diag C msedgedriver log (matches)' ($interesting -join "`n")
        Note notice 'diag C msedgedriver log (tail)' (Tail $clog 20)
}
Stop-Process -Id $drv.Id -Force -ErrorAction SilentlyContinue
StopApp $null
}

exit 0
