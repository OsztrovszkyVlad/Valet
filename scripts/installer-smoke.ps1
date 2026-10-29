<#
.SYNOPSIS
Installs, starts, uninstalls and reinstalls the Valet NSIS installer silently
and checks each step. Exits 1 if any check fails.

.DESCRIPTION
Per-user install (tauri.conf.json: nsis.installMode = currentUser):
1. `<setup.exe> /S`. The install dir comes from the uninstall key
   HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\* with
   DisplayName "Valet" (InstallLocation); it must be %LOCALAPPDATA%\Valet,
   hold Valet.exe and uninstall.exe, and no Valet key may exist under HKLM.
   The Start Menu shortcut (current user) must point to Valet.exe.
2. `Valet.exe --minimized`, wait -WaitSeconds: the process must still run,
   %APPDATA%\ro.valetapp.desktop\valet.sqlite3 and
   %LOCALAPPDATA%\ro.valetapp.desktop\logs\valet.log must exist. Then the
   process tree is stopped.
3. Writes the auto-start value HKCU\...\Run "Valet" the way the app does
   (`"<exe>" --minimized`), so the uninstaller has an entry to remove.
4. `uninstall.exe /S` (path from UninstallString). The NSIS uninstaller
   copies itself to %TEMP% and returns at once, so the script polls until
   Valet.exe, uninstall.exe and the uninstall key are gone. Then: Start Menu
   and desktop shortcuts gone, no "Valet" value under HKCU Run, user data
   %APPDATA%\ro.valetapp.desktop (with valet.sqlite3) kept.
5. Reinstall (`/S`), start again: the database is the same file (creation
   time and NTFS file ID unchanged) and the log has a second start line.
6. Installs once more over the installed app (`/S`): one uninstall key,
   Valet.exe present, database unchanged.
Leaves Valet installed. Under GitHub Actions writes `exe=<Valet.exe>` and
`dir=<install dir>` to $GITHUB_OUTPUT and emits ::notice:: / ::error::
annotations (install dir, installer size, timings, each failed check).
Nothing outside the install is deleted; the app data folders are left in
place.

.EXAMPLE
pwsh scripts/installer-smoke.ps1 target/release/bundle/nsis/Valet_0.1.0_x64-setup.exe
#>
param(
    [Parameter(Mandatory = $true, Position = 0)][string]$Installer,
    [int]$WaitSeconds = 10,
    [int]$TimeoutSeconds = 120
)

$ErrorActionPreference = 'Stop'
$InActions = ($env:GITHUB_ACTIONS -eq 'true')
$script:failures = New-Object System.Collections.Generic.List[string]
$script:app = $null

$ProductName = 'Valet'
$BundleId = 'ro.valetapp.desktop'
$UninstallSubkey = 'Software\Microsoft\Windows\CurrentVersion\Uninstall'
$UninstallSubkeyWow = 'Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall'
$RunKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
$DataDir = Join-Path $env:APPDATA $BundleId
$LocalDataDir = Join-Path $env:LOCALAPPDATA $BundleId
$DbPath = Join-Path $DataDir 'valet.sqlite3'
$LogPath = Join-Path $LocalDataDir 'logs\valet.log'
$ExpectedInstallDir = Join-Path $env:LOCALAPPDATA $ProductName
$StartMenuShortcut = Join-Path ([Environment]::GetFolderPath('Programs')) "$ProductName\$ProductName.lnk"
$CommonStartMenu = [Environment]::GetFolderPath('CommonPrograms')
$DesktopShortcut = Join-Path ([Environment]::GetFolderPath('DesktopDirectory')) "$ProductName.lnk"

function Write-Note([string]$Text) {
    Write-Host "installer-smoke: $Text"
    if ($InActions) { Write-Host "::notice title=installer-smoke::$Text" }
}

function Write-Fail([string]$Text) {
    $script:failures.Add($Text)
    Write-Host "installer-smoke: FAIL: $Text"
    if ($InActions) { Write-Host "::error title=installer-smoke::$Text" }
}

function Check([bool]$Condition, [string]$Text) {
    if ($Condition) { Write-Host "installer-smoke: ok: $Text" } else { Write-Fail $Text }
    return $Condition
}

function Stop-Smoke([string]$Text) {
    Write-Fail "$Text (stopping here)"
    Finish
}

function Finish {
    if ($null -ne $script:app) { try { Stop-AppTree $script:app } catch { } }
    if ($script:failures.Count -gt 0) {
        Write-Host "installer-smoke: $($script:failures.Count) check(s) failed"
        exit 1
    }
    Write-Note 'all checks passed'
    exit 0
}

trap {
    Write-Fail "unexpected error: $($_.Exception.Message -replace '\s+', ' ')"
    Finish
}

function Unquote([string]$s) {
    if ($null -eq $s) { return $null }
    return $s.Trim().Trim('"').TrimEnd('\')
}

function Same-Path([string]$a, [string]$b) {
    return [string]::Equals((Unquote $a), (Unquote $b), [StringComparison]::OrdinalIgnoreCase)
}

# Uninstall entries with DisplayName "Valet" under one Uninstall key.
function Get-ValetEntries([string]$Path) {
    if (-not (Test-Path -LiteralPath $Path)) { return @() }
    return @(Get-ChildItem -LiteralPath $Path -ErrorAction SilentlyContinue |
            ForEach-Object { Get-ItemProperty -LiteralPath $_.PSPath -ErrorAction SilentlyContinue } |
            Where-Object { $_.DisplayName -eq $ProductName })
}

function Get-HkcuEntries { return @(Get-ValetEntries "HKCU:\$UninstallSubkey") }

function Get-HklmEntries {
    return @(@(Get-ValetEntries "HKLM:\$UninstallSubkey") + @(Get-ValetEntries "HKLM:\$UninstallSubkeyWow"))
}

function Get-ShortcutTarget([string]$Path) {
    $shell = New-Object -ComObject WScript.Shell
    return $shell.CreateShortcut($Path).TargetPath
}

function Get-FileId([string]$Path) {
    try {
        $out = (& fsutil file queryFileID $Path 2>&1 | Out-String).Trim()
        if ($out -match '0x[0-9a-fA-F]+') { return $Matches[0] }
        return "unknown ($out)"
    }
    catch { return 'unknown' }
}

function Get-StartLines {
    if (-not (Test-Path -LiteralPath $LogPath)) { return 0 }
    return @(Select-String -LiteralPath $LogPath -SimpleMatch -Pattern "$ProductName " |
            Where-Object { $_.Line -match 'Valet \S+ starting' }).Count
}

# PIDs of $Root and all its descendants (the WebView2 processes).
function Get-TreeIds([int]$RootId) {
    $all = @(Get-CimInstance -ClassName Win32_Process -Property ProcessId, ParentProcessId -ErrorAction SilentlyContinue)
    $ids = New-Object System.Collections.Generic.List[int]
    $queue = New-Object System.Collections.Generic.Queue[int]
    $queue.Enqueue($RootId)
    while ($queue.Count -gt 0) {
        $id = $queue.Dequeue()
        if ($ids.Contains($id)) { continue }
        $ids.Add($id)
        foreach ($p in $all) {
            if ([int]$p.ParentProcessId -eq $id -and [int]$p.ProcessId -ne $id) { $queue.Enqueue([int]$p.ProcessId) }
        }
    }
    return , $ids
}

function Stop-AppTree([System.Diagnostics.Process]$Proc) {
    $ids = Get-TreeIds $Proc.Id
    foreach ($id in $ids) { Stop-Process -Id $id -Force -ErrorAction SilentlyContinue }
    $null = $Proc.WaitForExit(15000)
    $deadline = (Get-Date).AddSeconds(15)
    while ((Get-Date) -lt $deadline) {
        $left = @($ids | Where-Object { Get-Process -Id $_ -ErrorAction SilentlyContinue })
        if ($left.Count -eq 0) { break }
        Start-Sleep -Milliseconds 200
    }
    $script:app = $null
}

# Runs the installer silently. Returns the HKCU uninstall entry.
function Install-Valet([string]$Label) {
    $sw = [Diagnostics.Stopwatch]::StartNew()
    $p = Start-Process -FilePath $Installer -ArgumentList '/S' -PassThru -Wait
    $ms = $sw.ElapsedMilliseconds
    if (-not (Check ($p.ExitCode -eq 0) "$Label`: installer exit code $($p.ExitCode)")) {
        Stop-Smoke "$Label`: installer failed"
    }
    $entries = @(Get-HkcuEntries)
    while ($entries.Count -eq 0 -and $sw.Elapsed.TotalSeconds -lt $TimeoutSeconds) {
        Start-Sleep -Milliseconds 250
        $entries = @(Get-HkcuEntries)
    }
    Write-Note ("{0}: silent install took {1:N1} s" -f $Label, ($ms / 1000))
    if (-not (Check ($entries.Count -eq 1) "$Label`: one HKCU uninstall entry with DisplayName '$ProductName' (found $($entries.Count))")) {
        Stop-Smoke "$Label`: no usable uninstall entry"
    }
    $hklm = @(Get-HklmEntries)
    $null = Check ($hklm.Count -eq 0) "$Label`: no '$ProductName' uninstall entry under HKLM (found $($hklm.Count): $(($hklm | ForEach-Object { $_.PSPath }) -join ', '))"
    return $entries[0]
}

# Starts Valet.exe --minimized, waits, checks it runs and its files exist.
function Start-AndCheck([string]$Label, [string]$Exe) {
    $sw = [Diagnostics.Stopwatch]::StartNew()
    $script:app = Start-Process -FilePath $Exe -ArgumentList '--minimized' -WorkingDirectory (Split-Path -Parent $Exe) -PassThru
    $dbMs = $null
    $logMs = $null
    while ($sw.Elapsed.TotalSeconds -lt $WaitSeconds) {
        if ($null -eq $dbMs -and (Test-Path -LiteralPath $DbPath)) { $dbMs = $sw.ElapsedMilliseconds }
        if ($null -eq $logMs -and (Test-Path -LiteralPath $LogPath)) { $logMs = $sw.ElapsedMilliseconds }
        if ($script:app.HasExited) { break }
        Start-Sleep -Milliseconds 100
    }
    $alive = -not $script:app.HasExited
    $code = if ($alive) { '' } else { " (exited with code $($script:app.ExitCode))" }
    $null = Check $alive ("{0}: Valet.exe --minimized still running after {1:N1} s{2}" -f $Label, $sw.Elapsed.TotalSeconds, $code)
    if ($alive) {
        $path = (Get-Process -Id $script:app.Id).Path
        $null = Check (Same-Path $path $Exe) "$Label`: running process is the installed exe ($path)"
        $tree = Get-TreeIds $script:app.Id
        Write-Note "$Label`: process tree has $($tree.Count) process(es) (Valet.exe and WebView2)"
    }
    $null = Check (Test-Path -LiteralPath $DbPath) "$Label`: database exists ($DbPath)"
    $null = Check (Test-Path -LiteralPath $LogPath) "$Label`: log exists ($LogPath)"
    if ($null -ne $dbMs) { Write-Note "$Label`: database present $dbMs ms after start" }
    if ($null -ne $logMs) { Write-Note "$Label`: log present $logMs ms after start" }
    if (Test-Path -LiteralPath $LogPath) {
        $minimized = @(Select-String -LiteralPath $LogPath -SimpleMatch -Pattern 'Started minimized to the tray').Count
        $errors = @(Select-String -LiteralPath $LogPath -Pattern '\]\[ERROR\]').Count
        Write-Note "$Label`: log has $(Get-StartLines) start line(s), $minimized 'Started minimized' line(s), $errors ERROR line(s)"
    }
    if ($null -ne $script:app) { Stop-AppTree $script:app }
}

# --- Preconditions -----------------------------------------------------------
$Installer = (Resolve-Path -LiteralPath $Installer).Path
$size = (Get-Item -LiteralPath $Installer).Length
Write-Note ("installer {0}: {1:N2} MB ({2} bytes)" -f (Split-Path -Leaf $Installer), ($size / 1MB), $size)
if (@(Get-HkcuEntries).Count -gt 0) { Stop-Smoke 'Valet is already installed for this user' }
if (@(Get-Process -Name $ProductName -ErrorAction SilentlyContinue).Count -gt 0) { Stop-Smoke 'a Valet process is already running' }
$dataBefore = Test-Path -LiteralPath $DataDir
Write-Note "user data folder before install: $(if ($dataBefore) { 'exists' } else { 'absent' }) ($DataDir)"
$total = [Diagnostics.Stopwatch]::StartNew()

# --- 1. Install ----------------------------------------------------------------
$entry = Install-Valet 'install'
$installDir = Unquote $entry.InstallLocation
$exe = Join-Path $installDir "$ProductName.exe"
$uninstaller = Unquote $entry.UninstallString
Write-Note "install dir $installDir (InstallLocation), version $($entry.DisplayVersion), publisher '$($entry.Publisher)', key $($entry.PSChildName)"
$null = Check (Same-Path $installDir $ExpectedInstallDir) "install dir is $ExpectedInstallDir (got $installDir)"
$null = Check ($entry.MainBinaryName -eq "$ProductName.exe") "MainBinaryName is $ProductName.exe (got $($entry.MainBinaryName))"
if (-not (Check (Test-Path -LiteralPath $exe -PathType Leaf) "Valet.exe exists ($exe)")) { Stop-Smoke 'no Valet.exe' }
$null = Check (Same-Path $uninstaller (Join-Path $installDir 'uninstall.exe')) "UninstallString points to the install dir ($uninstaller)"
$null = Check (Test-Path -LiteralPath $uninstaller -PathType Leaf) "uninstall.exe exists ($uninstaller)"
$files = @(Get-ChildItem -LiteralPath $installDir -Recurse -File | ForEach-Object {
        "{0} {1:N2} MB" -f $_.FullName.Substring($installDir.Length + 1), ($_.Length / 1MB) })
Write-Note "installed files: $($files -join ', ')"
if (Check (Test-Path -LiteralPath $StartMenuShortcut) "Start Menu shortcut exists ($StartMenuShortcut)") {
    $target = Get-ShortcutTarget $StartMenuShortcut
    $null = Check (Same-Path $target $exe) "Start Menu shortcut targets Valet.exe (got $target)"
}
$common = @(Get-ChildItem -LiteralPath $CommonStartMenu -Recurse -Filter "$ProductName.lnk" -ErrorAction SilentlyContinue)
$null = Check ($common.Count -eq 0) "no Valet shortcut in the all-users Start Menu (found $($common.Count))"
$desktopAfterInstall = Test-Path -LiteralPath $DesktopShortcut
Write-Note "desktop shortcut after silent install: $(if ($desktopAfterInstall) { 'present' } else { 'absent' }) ($DesktopShortcut)"

# --- 2. First start --------------------------------------------------------------
Start-AndCheck 'first start' $exe
if (-not (Test-Path -LiteralPath $DbPath)) { Stop-Smoke 'no database after the first start' }
$dbCreated = (Get-Item -LiteralPath $DbPath).CreationTimeUtc
$dbId = Get-FileId $DbPath
Write-Note ("database created {0:o}, file ID {1}" -f $dbCreated, $dbId)

# --- 3. Auto-start value as the app writes it -------------------------------------
Set-ItemProperty -LiteralPath $RunKey -Name $ProductName -Value "`"$exe`" --minimized"
Write-Note "HKCU Run '$ProductName' set to `"$exe`" --minimized before uninstalling"

# --- 4. Uninstall ----------------------------------------------------------------
$sw = [Diagnostics.Stopwatch]::StartNew()
$p = Start-Process -FilePath $uninstaller -ArgumentList '/S' -PassThru -Wait
$null = Check ($p.ExitCode -eq 0) "uninstaller exit code $($p.ExitCode)"
$done = $false
while ($sw.Elapsed.TotalSeconds -lt $TimeoutSeconds) {
    if (-not (Test-Path -LiteralPath $exe) -and -not (Test-Path -LiteralPath $uninstaller) -and
        @(Get-HkcuEntries).Count -eq 0) { $done = $true; break }
    Start-Sleep -Milliseconds 250
}
# The copy in %TEMP% removes the Run value and shortcuts after the files;
# give it a moment to finish.
$deadline = (Get-Date).AddSeconds(10)
while ((Get-Date) -lt $deadline -and $null -ne (Get-ItemProperty -LiteralPath $RunKey -Name $ProductName -ErrorAction SilentlyContinue)) {
    Start-Sleep -Milliseconds 250
}
Write-Note ("silent uninstall took {0:N1} s{1}" -f $sw.Elapsed.TotalSeconds, $(if ($done) { '' } else { ' (timed out)' }))
$null = Check (-not (Test-Path -LiteralPath $exe)) "Valet.exe removed"
$null = Check (-not (Test-Path -LiteralPath $uninstaller)) "uninstall.exe removed"
Write-Note "install dir after uninstall: $(if (Test-Path -LiteralPath $installDir) { 'still present: ' + ((Get-ChildItem -LiteralPath $installDir -Recurse -Force | ForEach-Object Name) -join ', ') } else { 'removed' })"
$null = Check (-not (Test-Path -LiteralPath $StartMenuShortcut)) "Start Menu shortcut removed"
if ($desktopAfterInstall) { $null = Check (-not (Test-Path -LiteralPath $DesktopShortcut)) "desktop shortcut removed" }
$null = Check (@(Get-HkcuEntries).Count -eq 0) "HKCU uninstall entry removed"
$run = Get-ItemProperty -LiteralPath $RunKey -Name $ProductName -ErrorAction SilentlyContinue
$null = Check ($null -eq $run) "no '$ProductName' value under HKCU Run"
$null = Check (Test-Path -LiteralPath $DataDir -PathType Container) "user data folder kept ($DataDir)"
$null = Check (Test-Path -LiteralPath $DbPath -PathType Leaf) "database kept ($DbPath)"
Write-Note "local data folder after uninstall: $(if (Test-Path -LiteralPath $LocalDataDir) { 'kept' } else { 'removed' }) ($LocalDataDir)"

# --- 5. Reinstall, data reused ---------------------------------------------------
$startsBefore = Get-StartLines
$entry = Install-Valet 'reinstall'
$null = Check (Same-Path (Unquote $entry.InstallLocation) $installDir) "reinstall: same install dir ($(Unquote $entry.InstallLocation))"
$null = Check (Test-Path -LiteralPath $exe -PathType Leaf) "reinstall: Valet.exe exists"
$null = Check (Test-Path -LiteralPath $StartMenuShortcut) "reinstall: Start Menu shortcut exists"
Start-AndCheck 'start after reinstall' $exe
$created2 = (Get-Item -LiteralPath $DbPath).CreationTimeUtc
$id2 = Get-FileId $DbPath
$null = Check ($created2 -eq $dbCreated) ("reinstall: database creation time unchanged ({0:o}, was {1:o})" -f $created2, $dbCreated)
$null = Check ($id2 -eq $dbId) "reinstall: database file ID unchanged ($id2, was $dbId)"
$startsAfter = Get-StartLines
$null = Check ($startsAfter -gt $startsBefore) "reinstall: log reused and has a new start line ($startsBefore -> $startsAfter)"

# --- 6. Install over the installed app -------------------------------------------
$entry = Install-Valet 'install over existing'
$null = Check (Same-Path (Unquote $entry.InstallLocation) $installDir) "install over existing: same install dir"
$null = Check (Test-Path -LiteralPath $exe -PathType Leaf) "install over existing: Valet.exe exists"
$null = Check ((Get-Item -LiteralPath $DbPath).CreationTimeUtc -eq $dbCreated -and (Get-FileId $DbPath) -eq $dbId) "install over existing: database unchanged"

Write-Note ("total {0:N1} s; Valet left installed at {1}" -f $total.Elapsed.TotalSeconds, $installDir)
if ($env:GITHUB_OUTPUT) {
    "exe=$exe" | Out-File -FilePath $env:GITHUB_OUTPUT -Append -Encoding utf8
    "dir=$installDir" | Out-File -FilePath $env:GITHUB_OUTPUT -Append -Encoding utf8
}
Finish
