# Valet Development Environment Setup for Windows
# Run this script as Administrator in PowerShell

Write-Host "Setting up Valet development environment..." -ForegroundColor Green

# Check if running as Administrator
if (-NOT ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole] "Administrator"))
{
    Write-Host "This script requires Administrator privileges. Please run PowerShell as Administrator." -ForegroundColor Red
    exit 1
}

# Install Git
Write-Host "Installing Git..." -ForegroundColor Yellow
try {
    winget install Git.Git --accept-package-agreements --accept-source-agreements
    Write-Host "✓ Git installed successfully" -ForegroundColor Green
} catch {
    Write-Host "⚠ Git installation may have failed: $_" -ForegroundColor Yellow
}

# Install Node.js 20 LTS
Write-Host "Installing Node.js 20 LTS..." -ForegroundColor Yellow
try {
    winget install OpenJS.NodeJS.LTS --accept-package-agreements --accept-source-agreements
    Write-Host "✓ Node.js installed successfully" -ForegroundColor Green
} catch {
    Write-Host "⚠ Node.js installation may have failed: $_" -ForegroundColor Yellow
}

# Install Rust
Write-Host "Installing Rust..." -ForegroundColor Yellow
try {
    winget install Rustlang.Rustup --accept-package-agreements --accept-source-agreements
    Write-Host "✓ Rust installed successfully" -ForegroundColor Green
} catch {
    Write-Host "⚠ Rust installation may have failed: $_" -ForegroundColor Yellow
}

# Install Visual Studio Build Tools 2022
Write-Host "Installing Visual Studio Build Tools 2022..." -ForegroundColor Yellow
try {
    winget install Microsoft.VisualStudio.2022.BuildTools --accept-package-agreements --accept-source-agreements
    Write-Host "✓ Visual Studio Build Tools installed successfully" -ForegroundColor Green
    Write-Host "⚠ You still need to configure the C++ workload manually!" -ForegroundColor Yellow
} catch {
    Write-Host "⚠ Visual Studio Build Tools installation may have failed: $_" -ForegroundColor Yellow
}

Write-Host "`nRefreshing environment variables..." -ForegroundColor Yellow
$env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path","User")

Write-Host "`nSetup completed! Please follow these manual steps:" -ForegroundColor Green
Write-Host "1. Close and reopen PowerShell (as regular user)" -ForegroundColor Cyan
Write-Host "2. Run: rustup default stable-x86_64-pc-windows-msvc" -ForegroundColor Cyan
Write-Host "3. Open Visual Studio Installer and add 'Desktop development with C++' workload" -ForegroundColor Cyan
Write-Host "4. Ensure 'Windows 10/11 SDK' is selected in the installer" -ForegroundColor Cyan
Write-Host "5. Run: corepack enable" -ForegroundColor Cyan
Write-Host "6. Verify installation with these commands:" -ForegroundColor Cyan
Write-Host "   - rustc --version" -ForegroundColor White
Write-Host "   - cargo --version" -ForegroundColor White
Write-Host "   - node -v" -ForegroundColor White
Write-Host "   - pnpm -v" -ForegroundColor White
Write-Host "   - cl.exe /?" -ForegroundColor White

Write-Host "`n7. Then you can clone the repo and run:" -ForegroundColor Cyan
Write-Host "   cd valet/apps/desktop" -ForegroundColor White
Write-Host "   pnpm install" -ForegroundColor White
Write-Host "   pnpm tauri:dev" -ForegroundColor White

Write-Host "`nPress any key to exit..." -ForegroundColor Yellow
$null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown")
