<#
.SYNOPSIS
Measures Valet startup time and idle CPU / memory (docs/PLAN.md 2.6:
startup < 2 s, idle RAM < 150 MB, idle CPU < 1%). Report only: exits 0.
The RAM budget applies to the private bytes of the app process tree;
the working set is reported separately, without a budget.

.DESCRIPTION
1. Starts the built app with APPDATA pointing to a new temp folder.
2. Startup: time from process start until the process has a visible
   top-level window (Process.MainWindowHandle), and until its first
   msedgewebview2.exe child exists (from process creation times).
3. Waits until the app process has been up -SettleSeconds.
4. Every second for -Seconds: CPU% and memory of the process tree
   (app.exe and all descendants, i.e. the WebView2 processes).
   Memory is the sum over the tree of private bytes
   (Process.PrivateMemorySize64, compared with the 150 MB budget) and of
   the working set (Process.WorkingSet64, reported only).
   CPU% is of the whole machine (all logical processors), the unit of
   the 1% budget; "% of one core" is printed as well.
5. Prints averages and maxima, emits `::notice::` lines under GitHub
   Actions, then stops the process tree.

Notes:
- Tauri resolves %APPDATA% with SHGetKnownFolderPath, which ignores the
  APPDATA environment variable. The script reports whether the temp folder
  was used; if not, the app read the real roaming folder (on a CI runner
  that folder is fresh anyway).
- A Valet instance that is already running makes the new process exit at
  once (single-instance plugin). The script reports that and exits 0.
- Nothing is deleted. The temp folder is left in place and printed.

.EXAMPLE
pwsh scripts/measure-idle.ps1 -AppPath target/release/app.exe
#>
param(
    [Parameter(Mandatory = $true)][string]$AppPath,
    [int]$Seconds = 60,
    [double]$SettleSeconds = 2,
    [int]$WindowTimeoutSeconds = 30
)

$ErrorActionPreference = 'Continue'
$InActions = ($env:GITHUB_ACTIONS -eq 'true')
$proc = $null

trap {
    Write-Host "measure-idle: error: $_"
    if ($InActions) { Write-Host "::warning title=measure-idle::error: $_" }
    try { if ($null -ne $script:proc) { Stop-Tree $script:proc } } catch { }
    exit 0
}

function Write-Result([string]$Text) {
    Write-Host "measure-idle: $Text"
    if ($InActions) { Write-Host "::notice title=measure-idle::$Text" }
}

function Format-Budget([double]$Value, [double]$Budget) {
    if ($Value -lt $Budget) { return "under budget $Budget" }
    return "OVER budget $Budget"
}

# PIDs of $Root and all its descendants. Children created before the root
# (a reused parent PID) are ignored.
function Get-TreeIds([System.Diagnostics.Process]$Root) {
    $rootStart = $Root.StartTime
    $all = @(Get-CimInstance -ClassName Win32_Process -Property ProcessId, ParentProcessId, CreationDate -ErrorAction SilentlyContinue)
    $children = @{}
    foreach ($p in $all) {
        if ($null -ne $p.CreationDate -and $p.CreationDate -lt $rootStart.AddSeconds(-1)) { continue }
        $parent = [int]$p.ParentProcessId
        if (-not $children.ContainsKey($parent)) {
            $children[$parent] = New-Object System.Collections.Generic.List[int]
        }
        $children[$parent].Add([int]$p.ProcessId)
    }
    $ids = New-Object System.Collections.Generic.List[int]
    $seen = @{}
    $queue = New-Object System.Collections.Generic.Queue[int]
    $queue.Enqueue($Root.Id)
    while ($queue.Count -gt 0) {
        $id = $queue.Dequeue()
        if ($seen.ContainsKey($id)) { continue }
        $seen[$id] = $true
        $ids.Add($id)
        if ($children.ContainsKey($id)) {
            foreach ($c in $children[$id]) { $queue.Enqueue($c) }
        }
    }
    return , $ids
}

function Stop-Tree([System.Diagnostics.Process]$Root) {
    $ids = @()
    if (-not $Root.HasExited) { $ids = Get-TreeIds $Root }
    foreach ($id in $ids) {
        Stop-Process -Id $id -Force -ErrorAction SilentlyContinue
    }
}

$exe = (Resolve-Path -LiteralPath $AppPath -ErrorAction Stop).Path
$cpus = [Environment]::ProcessorCount
$tempAppData = Join-Path ([IO.Path]::GetTempPath()) ("valet-idle-" + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $tempAppData -Force | Out-Null
Write-Host "measure-idle: app $exe, $cpus logical processors, APPDATA=$tempAppData"

$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = $exe
$psi.UseShellExecute = $false
$psi.WorkingDirectory = Split-Path -Parent $exe
$psi.EnvironmentVariables['APPDATA'] = $tempAppData

# --- Startup -----------------------------------------------------------------
$sw = [System.Diagnostics.Stopwatch]::StartNew()
$proc = [System.Diagnostics.Process]::Start($psi)
$windowMs = $null
$webviewMs = $null
while ($sw.Elapsed.TotalSeconds -lt $WindowTimeoutSeconds) {
    if ($proc.HasExited) { break }
    $proc.Refresh()
    if ($null -eq $windowMs -and $proc.MainWindowHandle -ne [IntPtr]::Zero) {
        $windowMs = $sw.ElapsedMilliseconds
    }
    if ($null -eq $webviewMs) {
        $appStart = $proc.StartTime
        $candidates = @(Get-Process -Name msedgewebview2 -ErrorAction SilentlyContinue |
                Where-Object { try { $_.StartTime -ge $appStart } catch { $false } })
        if ($candidates.Count -gt 0) {
            $tree = Get-TreeIds $proc
            $mine = @($candidates | Where-Object { $tree.Contains($_.Id) } | Sort-Object StartTime)
            if ($mine.Count -gt 0) {
                $webviewMs = [int](($mine[0].StartTime - $appStart).TotalMilliseconds)
            }
        }
    }
    if ($null -ne $windowMs -and $null -ne $webviewMs) { break }
    Start-Sleep -Milliseconds 20
}

if ($proc.HasExited) {
    Write-Result ("app exited during startup with code {0} after {1:N1} s (another Valet instance running?); no numbers" -f $proc.ExitCode, $sw.Elapsed.TotalSeconds)
    exit 0
}

if ($null -ne $windowMs) {
    Write-Result ("startup: first visible window after {0} ms ({1})" -f $windowMs, (Format-Budget $windowMs 2000))
}
else {
    Write-Result "startup: no visible window within $WindowTimeoutSeconds s (no interactive desktop?)"
}
if ($null -ne $webviewMs) {
    Write-Result ("startup: first WebView2 child process {0} ms after app start" -f $webviewMs)
}
else {
    Write-Result "startup: no WebView2 child process within $WindowTimeoutSeconds s"
}

# --- Settle ------------------------------------------------------------------
while (((Get-Date) - $proc.StartTime).TotalSeconds -lt $SettleSeconds -and -not $proc.HasExited) {
    Start-Sleep -Milliseconds 50
}

# --- Idle sampling -------------------------------------------------------------
$prevCpu = @{}
$prevTick = $null
$prevWall = $null
$cpuSamples = New-Object System.Collections.Generic.List[double]
$wsSamples = New-Object System.Collections.Generic.List[double]
$privSamples = New-Object System.Collections.Generic.List[double]
$maxProcs = 0
$clock = [System.Diagnostics.Stopwatch]::StartNew()

# One baseline sample plus $Seconds measured samples, one second apart.
for ($i = 0; $i -le $Seconds; $i++) {
    $target = $i * 1000
    $wait = $target - $clock.ElapsedMilliseconds
    if ($wait -gt 0) { Start-Sleep -Milliseconds $wait }
    if ($proc.HasExited) {
        Write-Result ("app exited during sampling with code {0} after {1} samples" -f $proc.ExitCode, $cpuSamples.Count)
        break
    }

    $ids = Get-TreeIds $proc
    $tick = $clock.Elapsed.TotalMilliseconds
    $wall = Get-Date
    $ws = 0.0
    $priv = 0.0
    $cpuDeltaMs = 0.0
    $curCpu = @{}
    $count = 0
    foreach ($id in $ids) {
        $p = Get-Process -Id $id -ErrorAction SilentlyContinue
        if ($null -eq $p) { continue }
        $count++
        $ws += $p.WorkingSet64
        $priv += $p.PrivateMemorySize64
        $t = $null
        try { $t = $p.TotalProcessorTime.TotalMilliseconds } catch { $t = $null }
        if ($null -eq $t) { continue }
        $curCpu[$id] = $t
        if ($prevCpu.ContainsKey($id)) {
            $cpuDeltaMs += [Math]::Max(0.0, $t - $prevCpu[$id])
        }
        elseif ($null -ne $prevWall) {
            # Started since the previous sample: all its CPU time is new.
            $started = $null
            try { $started = $p.StartTime } catch { $started = $null }
            if ($null -ne $started -and $started -ge $prevWall) { $cpuDeltaMs += $t }
        }
    }
    if ($count -gt $maxProcs) { $maxProcs = $count }

    if ($null -ne $prevTick) {
        $elapsedMs = $tick - $prevTick
        if ($elapsedMs -gt 0) {
            $cpuSamples.Add(100.0 * $cpuDeltaMs / ($elapsedMs * $cpus))
            $wsSamples.Add($ws / 1MB)
            $privSamples.Add($priv / 1MB)
        }
    }
    $prevCpu = $curCpu
    $prevTick = $tick
    $prevWall = $wall
}

$configUsed = Test-Path -LiteralPath (Join-Path $tempAppData 'ro.valetapp.desktop')
Stop-Tree $proc

if ($cpuSamples.Count -eq 0) {
    Write-Result "idle: no samples"
    exit 0
}

$cpuAvg = ($cpuSamples | Measure-Object -Average).Average
$cpuMax = ($cpuSamples | Measure-Object -Maximum).Maximum
$wsAvg = ($wsSamples | Measure-Object -Average).Average
$wsMax = ($wsSamples | Measure-Object -Maximum).Maximum
$privAvg = ($privSamples | Measure-Object -Average).Average
$privMax = ($privSamples | Measure-Object -Maximum).Maximum

Write-Result ("idle CPU over {0} s (app + {1} child processes): avg {2:N2}%, max {3:N2}% of {4} logical processors ({5}); avg {6:N1}% of one core" -f `
        $cpuSamples.Count, ($maxProcs - 1), $cpuAvg, $cpuMax, $cpus, (Format-Budget $cpuAvg 1), ($cpuAvg * $cpus))
Write-Result ("idle private bytes (app tree): avg {0:N1} MB ({1}), max {2:N1} MB" -f $privAvg, (Format-Budget $privAvg 150), $privMax)
Write-Result ("idle working set (app tree): avg {0:N1} MB (reported, no budget), max {1:N1} MB" -f $wsAvg, $wsMax)
if ($configUsed) {
    Write-Result "config: fresh folder created under the temp APPDATA"
}
else {
    $real = Join-Path ([Environment]::GetFolderPath('ApplicationData')) 'ro.valetapp.desktop'
    Write-Result "config: temp APPDATA not used (known-folder lookup); app used $real"
}
exit 0
