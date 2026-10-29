<#
.SYNOPSIS
Installs a signed copy of the Valet MSIX, starts it through its AUMID, checks
it runs and creates its database, then removes it. For CI runners and test
VMs only. Exits 1 if any check fails.

.DESCRIPTION
The package from scripts/msix-pack.ps1 is unsigned (the Store signs it). To
install it here:
1. A throwaway code-signing certificate is created in Cert:\CurrentUser\My
   (New-SelfSignedCertificate, subject = the manifest Publisher, valid one
   day). Its public part is added to CurrentUser\TrustedPeople; if
   Add-AppxPackage still rejects the signature, to LocalMachine\TrustedPeople
   (needs admin; CI runners are). The certificate is never exported and is
   removed from every store at the end. It is not Vlad's signing key and
   signs nothing that leaves the run.
2. A COPY of the package (in a separate folder) is signed with signtool. The
   unsigned package is checked to be unchanged (SHA-256).
3. Add-AppxPackage (if sideloading is off, AllowAllTrustedApps is set and the
   install retried; note says so). The installed package must have the
   manifest version and the startup task ValetStartup (Enabled=false).
4. `shell:AppsFolder\<PackageFamilyName>!App`: a Valet.exe from the package's
   install folder must run for -WaitSeconds; valet.sqlite3 must appear. The
   script looks in the package's redirected AppData
   (%LOCALAPPDATA%\Packages\<PFN>\LocalCache\Roaming\ro.valetapp.desktop),
   anywhere under %LOCALAPPDATA%\Packages\<PFN>, and in the real
   %APPDATA%\ro.valetapp.desktop, and reports where it was found. Same for
   logs\valet.log. Then the process tree is stopped.
5. Remove-AppxPackage; the package must be gone.
Under GitHub Actions emits one ::notice title=msix <phase>:: per phase and
::error for each failed check.

.EXAMPLE
pwsh scripts/msix-smoke.ps1 -Package $env:RUNNER_TEMP\msix\Valet_0.1.0_x64.msix -Manifest $env:RUNNER_TEMP\msix\layout\AppxManifest.xml
#>
param(
    [Parameter(Mandatory = $true)][string]$Package,
    [Parameter(Mandatory = $true)][string]$Manifest,
    [string]$SignTool = '',
    [int]$WaitSeconds = 10,
    [int]$TimeoutSeconds = 60
)

$ErrorActionPreference = 'Stop'
$InActions = ($env:GITHUB_ACTIONS -eq 'true')
$script:failures = New-Object System.Collections.Generic.List[string]
$script:phase = 'setup'
$script:notes = [ordered]@{}
$script:thumb = $null
$script:machineTrust = $false
$script:installed = $null
$script:app = $null

$BundleId = 'ro.valetapp.desktop'
$RealDb = Join-Path $env:APPDATA "$BundleId\valet.sqlite3"
$RealLog = Join-Path $env:LOCALAPPDATA "$BundleId\logs\valet.log"

function Set-Phase([string]$Name) { $script:phase = $Name }

function Add-Line([string]$Text) {
    if (-not $script:notes.Contains($script:phase)) {
        $script:notes[$script:phase] = New-Object System.Collections.Generic.List[string]
    }
    $script:notes[$script:phase].Add($Text)
}

function Write-Note([string]$Text) {
    Write-Host "msix-smoke: [$($script:phase)] $Text"
    Add-Line $Text
}

function Write-Fail([string]$Text) {
    $script:failures.Add("[$($script:phase)] $Text")
    Write-Host "msix-smoke: [$($script:phase)] FAIL: $Text"
    Add-Line "FAIL: $Text"
    if ($InActions) { Write-Host "::error title=msix $($script:phase)::$($Text -replace '%', '%25')" }
}

function Check([bool]$Condition, [string]$Text) {
    if ($Condition) {
        Write-Host "msix-smoke: [$($script:phase)] ok: $Text"
        Add-Line "ok: $Text"
    }
    else { Write-Fail $Text }
    return $Condition
}

function Stop-Smoke([string]$Text) {
    Write-Fail "$Text (stopping here)"
    Finish
}

# Removes what this run added: the app process, the package, the certificate.
function Remove-RunState {
    if ($null -ne $script:app) { try { Stop-AppTree $script:app } catch { } }
    if ($null -ne $script:installed) {
        try { Remove-AppxPackage -Package $script:installed -ErrorAction Stop } catch { Write-Host "msix-smoke: cleanup: Remove-AppxPackage: $($_.Exception.Message)" }
        $script:installed = $null
    }
    if ($null -ne $script:thumb) {
        $stores = @('Cert:\CurrentUser\My', 'Cert:\CurrentUser\TrustedPeople')
        if ($script:machineTrust) { $stores += 'Cert:\LocalMachine\TrustedPeople' }
        foreach ($s in $stores) {
            $p = Join-Path $s $script:thumb
            if (Test-Path -LiteralPath $p) { Remove-Item -LiteralPath $p -ErrorAction SilentlyContinue }
        }
        $left = @($stores | Where-Object { Test-Path -LiteralPath (Join-Path $_ $script:thumb) })
        Write-Host "msix-smoke: cleanup: throwaway certificate removed from $($stores -join ', ')$(if ($left.Count) { " (still in: $($left -join ', '))" })"
        $script:thumb = $null
    }
}

function Finish {
    Remove-RunState
    $result = if ($script:failures.Count -gt 0) { "$($script:failures.Count) check(s) failed" } else { 'all checks passed' }
    Set-Phase 'result'
    Add-Line $result
    if ($InActions) {
        foreach ($k in $script:notes.Keys) {
            $msg = ($script:notes[$k] -join '; ') -replace '%', '%25' -replace "`r", '' -replace "`n", ' '
            Write-Host "::notice title=msix ${k}::$msg"
        }
    }
    Write-Host "msix-smoke: $result"
    if ($script:failures.Count -gt 0) { exit 1 }
    exit 0
}

trap {
    Write-Fail "unexpected error: $($_.Exception.Message -replace '\s+', ' ')"
    Finish
}

# PIDs of $RootId and all its descendants (the WebView2 processes).
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
    $script:app = $null
}

# Valet.exe processes started from the package's install folder.
function Get-PackagedValet([string]$InstallLocation) {
    return @(Get-Process -Name 'Valet' -ErrorAction SilentlyContinue | Where-Object {
            $_.Path -and $_.Path.StartsWith($InstallLocation, [StringComparison]::OrdinalIgnoreCase) })
}

# First existing path among the redirected and real locations of a file.
function Find-AppFile([string]$PackageData, [string]$Redirected, [string]$Leaf, [string]$Real) {
    if (Test-Path -LiteralPath $Redirected -PathType Leaf) { return @{ Path = $Redirected; Kind = 'package AppData (redirected)' } }
    if (Test-Path -LiteralPath $PackageData) {
        $hit = @(Get-ChildItem -LiteralPath $PackageData -Recurse -File -Filter $Leaf -ErrorAction SilentlyContinue | Select-Object -First 1)
        if ($hit.Count -gt 0) { return @{ Path = $hit[0].FullName; Kind = 'package data folder' } }
    }
    if (Test-Path -LiteralPath $Real -PathType Leaf) { return @{ Path = $Real; Kind = 'real user AppData (not redirected)' } }
    return $null
}

function Add-Trust([string]$Location, [System.Security.Cryptography.X509Certificates.X509Certificate2]$Public) {
    $store = [System.Security.Cryptography.X509Certificates.X509Store]::new('TrustedPeople', $Location)
    $store.Open('ReadWrite')
    try { $store.Add($Public) } finally { $store.Close() }
}

# --- Setup ---------------------------------------------------------------------------
Set-Phase 'setup'
if (-not (Test-Path -LiteralPath $Package -PathType Leaf)) { Stop-Smoke "no package at $Package" }
$Package = (Resolve-Path -LiteralPath $Package).Path
[xml]$xml = Get-Content -LiteralPath $Manifest -Raw
$identity = $xml.DocumentElement.SelectSingleNode("*[local-name()='Identity']")
$Name = $identity.GetAttribute('Name')
$Publisher = $identity.GetAttribute('Publisher')
$Version = $identity.GetAttribute('Version')
Write-Note "package $(Split-Path -Leaf $Package), identity $Name, publisher $Publisher, version $Version"
if (@(Get-AppxPackage -Name $Name).Count -gt 0) { Stop-Smoke "a package named $Name is already installed" }
if (@(Get-Process -Name 'Valet' -ErrorAction SilentlyContinue).Count -gt 0) { Stop-Smoke 'a Valet process is already running' }
$realDbBefore = Test-Path -LiteralPath $RealDb
Write-Note "real $RealDb before start: $(if ($realDbBefore) { 'exists' } else { 'absent' })"
$shaBefore = (Get-FileHash -LiteralPath $Package -Algorithm SHA256).Hash

# --- Sign a copy ---------------------------------------------------------------------
Set-Phase 'sign'
if (-not $SignTool -or -not (Test-Path -LiteralPath $SignTool -PathType Leaf)) {
    $cmd = Get-Command 'signtool.exe' -ErrorAction SilentlyContinue
    if ($null -eq $cmd) { Stop-Smoke "signtool.exe not found (got '$SignTool', none on PATH)" }
    $SignTool = $cmd.Source
}
$cert = New-SelfSignedCertificate -Type Custom -Subject $Publisher -KeyUsage DigitalSignature `
    -FriendlyName 'Valet MSIX CI smoke (throwaway)' -CertStoreLocation 'Cert:\CurrentUser\My' `
    -TextExtension @('2.5.29.37={text}1.3.6.1.5.5.7.3.3', '2.5.29.19={text}') -NotAfter (Get-Date).AddDays(1)
$script:thumb = $cert.Thumbprint
# Public part only (no private key) for the TrustedPeople stores.
$public = [System.Security.Cryptography.X509Certificates.X509Certificate2]::new($cert.RawData)
Add-Trust 'CurrentUser' $public
Write-Note "throwaway certificate $($cert.Subject) created in CurrentUser\My, trusted in CurrentUser\TrustedPeople"

$signedDir = Join-Path (Split-Path -Parent (Split-Path -Parent $Package)) 'msix-signed-test'
if (Test-Path -LiteralPath $signedDir) { Stop-Smoke "$signedDir already exists" }
$null = New-Item -ItemType Directory -Path $signedDir
$signed = Join-Path $signedDir (Split-Path -Leaf $Package)
Copy-Item -LiteralPath $Package -Destination $signed
$out = @(& $SignTool sign /fd SHA256 /sha1 $script:thumb /s My $signed 2>&1 | ForEach-Object { "$_" })
$code = $LASTEXITCODE
$out | ForEach-Object { Write-Host $_ }
if (-not (Check ($code -eq 0) "signtool sign exit code $code ($SignTool)")) {
    Stop-Smoke "signing failed: $(($out | Select-Object -Last 3) -join ' | ')"
}
$null = Check ((Get-FileHash -LiteralPath $Package -Algorithm SHA256).Hash -eq $shaBefore) "unsigned package unchanged (sha256)"

# --- Install -------------------------------------------------------------------------
Set-Phase 'install'
function Install-SignedCopy {
    try { Add-AppxPackage -Path $signed -ErrorAction Stop; return $null }
    catch { return ($_.Exception.Message -replace '\s+', ' ') }
}
$sw = [Diagnostics.Stopwatch]::StartNew()
$err = Install-SignedCopy
if ($null -ne $err -and $err -match '0x800B0109|0x800B010A|trust|certificate') {
    Write-Note "Add-AppxPackage with CurrentUser\TrustedPeople failed: $err"
    Add-Trust 'LocalMachine' $public
    $script:machineTrust = $true
    Write-Note 'certificate added to LocalMachine\TrustedPeople, retrying'
    $err = Install-SignedCopy
}
if ($null -ne $err -and $err -match '0x80073CFF|sideload|developer license') {
    Write-Note "Add-AppxPackage failed, sideloading off: $err"
    $unlock = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\AppModelUnlock'
    if (-not (Test-Path -LiteralPath $unlock)) { $null = New-Item -Path $unlock }
    Set-ItemProperty -LiteralPath $unlock -Name 'AllowAllTrustedApps' -Value 1 -Type DWord
    Write-Note 'AllowAllTrustedApps set to 1, retrying'
    $err = Install-SignedCopy
}
if (-not (Check ($null -eq $err) "Add-AppxPackage of the signed copy$(if ($null -ne $err) { ": $err" })")) { Stop-Smoke 'install failed' }
Write-Note ("installed in {0:N1} s; trust store used: {1}" -f $sw.Elapsed.TotalSeconds, $(if ($script:machineTrust) { 'LocalMachine\TrustedPeople' } else { 'CurrentUser\TrustedPeople' }))

$pkgs = @(Get-AppxPackage -Name $Name)
if (-not (Check ($pkgs.Count -eq 1) "one installed package named $Name (found $($pkgs.Count))")) { Stop-Smoke 'package not registered' }
$pkg = $pkgs[0]
$script:installed = $pkg.PackageFullName
$pfn = $pkg.PackageFamilyName
Write-Note "PackageFullName $($pkg.PackageFullName), PackageFamilyName $pfn, SignatureKind $($pkg.SignatureKind), InstallLocation $($pkg.InstallLocation)"
$null = Check ([string]$pkg.Version -eq $Version) "installed version $($pkg.Version) is the manifest version $Version"
$installedManifest = Get-AppxPackageManifest -Package $pkg.PackageFullName
$task = $installedManifest.SelectSingleNode("//*[local-name()='StartupTask']")
$null = Check ($null -ne $task -and $task.GetAttribute('TaskId') -eq 'ValetStartup' -and $task.GetAttribute('Enabled') -eq 'false') `
    "startup task ValetStartup declared, Enabled=false"
$null = Check ($null -ne $installedManifest.SelectSingleNode("//*[local-name()='Capability' and @Name='runFullTrust']")) "runFullTrust capability declared"

# --- Start through the AUMID ------------------------------------------------------------
Set-Phase 'start'
$aumid = "$pfn!App"
$pkgData = Join-Path $env:LOCALAPPDATA "Packages\$pfn"
$redirectedDb = Join-Path $pkgData "LocalCache\Roaming\$BundleId\valet.sqlite3"
$redirectedLog = Join-Path $pkgData "LocalCache\Local\$BundleId\logs\valet.log"
$sw = [Diagnostics.Stopwatch]::StartNew()
try { Start-Process -FilePath "shell:AppsFolder\$aumid" }
catch {
    Write-Note "Start-Process shell:AppsFolder\$aumid failed ($($_.Exception.Message -replace '\s+', ' ')), using explorer.exe"
    Start-Process -FilePath 'explorer.exe' -ArgumentList "shell:AppsFolder\$aumid"
}
$procs = @()
while ($sw.Elapsed.TotalSeconds -lt $TimeoutSeconds) {
    $procs = @(Get-PackagedValet $pkg.InstallLocation)
    if ($procs.Count -gt 0) { break }
    Start-Sleep -Milliseconds 250
}
if (-not (Check ($procs.Count -gt 0) ("Valet.exe from the package started via shell:AppsFolder\{0} (after {1:N1} s)" -f $aumid, $sw.Elapsed.TotalSeconds))) {
    $any = @(Get-Process -Name 'Valet' -ErrorAction SilentlyContinue | ForEach-Object { $_.Path })
    Write-Note "Valet processes seen: $(if ($any.Count) { $any -join ', ' } else { 'none' })"
    Stop-Smoke 'the app did not start'
}
$script:app = $procs[0]
$null = $script:app.Handle
$appPath = $script:app.Path
$startMs = $sw.ElapsedMilliseconds
$run = [Diagnostics.Stopwatch]::StartNew()
$db = $null
$dbMs = $null
while ($run.Elapsed.TotalSeconds -lt $WaitSeconds -or ($null -eq $db -and $run.Elapsed.TotalSeconds -lt $TimeoutSeconds)) {
    if ($null -eq $db) {
        $db = Find-AppFile $pkgData $redirectedDb 'valet.sqlite3' $RealDb
        if ($null -ne $db -and $db.Kind -like 'real*' -and $realDbBefore) { $db = $null }
        if ($null -ne $db) { $dbMs = $run.ElapsedMilliseconds }
    }
    if ($script:app.HasExited) { break }
    Start-Sleep -Milliseconds 200
}
$alive = -not $script:app.HasExited
$code = ''
if (-not $alive) { try { $code = " (exited with code $($script:app.ExitCode))" } catch { $code = ' (exited)' } }
$null = Check ($alive -and $run.Elapsed.TotalSeconds -ge $WaitSeconds) ("packaged Valet.exe still running after {0:N1} s{1} ({2})" -f $run.Elapsed.TotalSeconds, $code, $appPath)
if ($alive) { Write-Note "process tree has $((Get-TreeIds $script:app.Id).Count) process(es) (Valet.exe and WebView2)" }
if (Check ($null -ne $db) "database created") {
    Write-Note "database at $($db.Path) [$($db.Kind)], $dbMs ms after the process appeared ($startMs ms after the launch)"
}
else {
    $files = @()
    if (Test-Path -LiteralPath $pkgData) {
        $files = @(Get-ChildItem -LiteralPath $pkgData -Recurse -File -ErrorAction SilentlyContinue | Select-Object -First 15 | ForEach-Object { $_.FullName.Substring($pkgData.Length + 1) })
    }
    Write-Note "files under $pkgData`: $(if ($files.Count) { $files -join ', ' } else { 'none' })"
}
$log = Find-AppFile $pkgData $redirectedLog 'valet.log' $RealLog
if ($null -ne $log) {
    $errors = @(Select-String -LiteralPath $log.Path -Pattern '\]\[ERROR\]' -ErrorAction SilentlyContinue).Count
    $starts = @(Select-String -LiteralPath $log.Path -Pattern 'Valet \S+ starting' -ErrorAction SilentlyContinue).Count
    Write-Note "log at $($log.Path) [$($log.Kind)]: $starts start line(s), $errors ERROR line(s)"
    $firstErrors = @(Select-String -LiteralPath $log.Path -Pattern '\]\[ERROR\]' -ErrorAction SilentlyContinue | Select-Object -First 3 | ForEach-Object { $_.Line.Trim() })
    if ($firstErrors.Count -gt 0) { Write-Note "first ERROR lines: $($firstErrors -join ' | ')" }
}
else { Write-Note 'no valet.log found' }
if ($null -ne $script:app) { Stop-AppTree $script:app }

# --- Remove --------------------------------------------------------------------------
Set-Phase 'remove'
$sw = [Diagnostics.Stopwatch]::StartNew()
Remove-AppxPackage -Package $pkg.PackageFullName
$script:installed = $null
$null = Check (@(Get-AppxPackage -Name $Name).Count -eq 0) ("package removed ({0:N1} s)" -f $sw.Elapsed.TotalSeconds)
Write-Note "package data folder after removal: $(if (Test-Path -LiteralPath $pkgData) { 'still present' } else { 'removed by Windows' }) ($pkgData)"

Finish
