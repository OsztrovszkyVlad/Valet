# Contributing to Valet

Thank you for your interest in contributing to Valet! This document provides guidelines for development and testing.

## Windows Development Setup

Please follow the setup instructions in [README.md](README.md#windows-setup) to install the required tools:
- Git via winget
- Node.js 20 LTS via winget  
- Rust via rustup (stable msvc toolchain)
- Visual Studio Build Tools 2022 with Desktop C++ workload and Windows 10/11 SDK
- Enable Corepack and pnpm

Alternatively, you can run `scripts/windows_dev_setup.ps1` as Administrator to automate most of the installation.

## Windows Test Checklist

Before submitting any changes, please verify the following functionality on Windows:

### 🔧 Basic Functionality
- [ ] **App launches**: `pnpm tauri:dev` starts without errors
- [ ] **Tray icon**: System tray shows Valet icon with menu
- [ ] **Database creation**: `%AppData%\com.example.Valet\valet.sqlite3` is created on first run
- [ ] **Config loading**: `%AppData%\com.example.Valet\config.json` is read and logged

### 📁 File Watching
- [ ] **Watcher starts**: Debug logs show successful directory watching
- [ ] **File detection**: Create/rename files in watched directories and verify events are logged
- [ ] **Pause/Resume**: Toggle "Pause watchers" in tray menu
  - [ ] When paused: No file events should be detected
  - [ ] When resumed: File events should be detected again

### 🔍 Dry Run Testing
1. **Create test PDF**: Place a file named `test-invoice-2025.pdf` in Downloads folder
2. **Run dry-run**: Click "Index Downloads (dry run)" in tray menu
3. **Verify output**: Check terminal logs for:
   - [ ] `Tray: dry-run clicked`
   - [ ] `🔍 Dry run results for X action(s):`
   - [ ] Rule match: `[Invoices → Finance] ... -> MoveTo { path: "~/Documents/Finance" }`
   - [ ] Tag action: `... -> Tag { tags: ["finance", "invoice"] }`

### 🏗️ Build Testing
- [ ] **Development build**: `pnpm tauri:dev` compiles and runs
- [ ] **Production build**: `pnpm tauri build` completes successfully (smoke test)
- [ ] **Build artifacts**: Check `src-tauri/target/release/bundle/` for .exe and installer

### 🧪 Code Quality
- [ ] **Clippy clean**: `cargo clippy --all-targets --all-features -- -D warnings` passes
- [ ] **No compiler warnings**: Development build shows minimal warnings

## File Locations

| Item | Windows Path |
|------|-------------|
| Config file | `%AppData%\com.example.Valet\config.json` |
| Database | `%AppData%\com.example.Valet\valet.sqlite3` |
| Logs | Terminal output (development) |
| Built artifacts | `apps/desktop/src-tauri/target/release/bundle/` |

## Sample Config

For testing, create or edit `%AppData%\com.example.Valet\config.json`:

```json
{
  "inbox_paths": [
    "C:\\Users\\<username>\\Downloads",
    "C:\\Users\\<username>\\Desktop"
  ],
  "pause_watchers": false
}
```

## Submitting Changes

1. Ensure all tests in the checklist above pass
2. Run `cargo clippy --all-targets --all-features -- -D warnings` 
3. Test both development and production builds
4. Include relevant test results in your pull request description

## Troubleshooting

### Common Issues

**"msvc toolchain not found"**
- Ensure Desktop C++ workload + Windows SDK are installed
- Set default toolchain: `rustup default stable-x86_64-pc-windows-msvc`

**"pnpm not found"**
- Re-open PowerShell after running `corepack enable`

**"Permission denied moving files"**
- Use user-writable folders (Downloads, Desktop) for testing
- Ensure antivirus isn't blocking file operations

**Build failures**
- Clear target directory: `cargo clean`
- Update dependencies: `pnpm install`
- Check Node.js version: `node -v` (should be 20.x)
