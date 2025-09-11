# Valet

A smart file organization tool that automatically sorts and manages your files based on customizable rules.

## Windows Setup

### Prerequisites

Install the following tools via PowerShell (as Administrator):

```powershell
# Install Git
winget install Git.Git

# Install Node.js 20 LTS
winget install OpenJS.NodeJS.LTS

# Install Rust
winget install Rustlang.Rustup

# Install Visual Studio Build Tools 2022
winget install Microsoft.VisualStudio.2022.BuildTools
```

### Post-Installation Setup

1. **Configure Rust toolchain**:
   ```powershell
   rustup default stable-x86_64-pc-windows-msvc
   ```

2. **Install Visual Studio Build Tools components**:
   - Open "Visual Studio Installer"
   - Select "Desktop development with C++" workload
   - Ensure "Windows 10/11 SDK" is included
   - Click "Install"

3. **Enable Corepack and pnpm**:
   ```powershell
   corepack enable
   ```

4. **Verify installation**:
   ```powershell
   rustc --version  # Should show stable msvc
   cargo --version
   node -v          # Should be 20.x
   pnpm -v
   cl.exe /?        # Should find MSVC compiler
   ```

### Development

1. **Clone the repository**:
   ```powershell
   git clone <your-repo-url>
   cd valet
   ```

2. **Install dependencies and start development**:
   ```powershell
   cd apps/desktop
   pnpm install
   pnpm tauri:dev
   ```

### Configuration

- **Config file**: `%AppData%\com.example.Valet\config.json`
- **Database**: `%AppData%\com.example.Valet\valet.sqlite3`

The app will create these automatically on first run. Edit `config.json` to set your watched folders:

```json
{
  "inbox_paths": [
    "C:\\Users\\<username>\\Downloads",
    "C:\\Users\\<username>\\Desktop"
  ],
  "pause_watchers": false
}
```

## Windows Release Build

To create a production build:

```powershell
cd apps/desktop
pnpm tauri build
```

The built executable and installer will be in `src-tauri/target/release/bundle/`. 

**Note**: Artifacts are currently unsigned. Windows Defender SmartScreen may show warnings until code signing is implemented.
