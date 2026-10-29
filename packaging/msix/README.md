# Valet in the Microsoft Store (MSIX)

The Store build of Valet is an MSIX package built in GitHub Actions
(`.github/workflows/msix.yml`) and uploaded by Vlad in Partner Center. The
Store signs the package, so it needs no code-signing certificate of ours
(PLAN 3.2, 3.3, 4.7).

The package holds `Valet.exe`, `AppxManifest.xml` (from the template in this
folder) and four logos. It is a full-trust desktop app
(`Windows.FullTrustApplication`, capability `runFullTrust`) for
Windows 10 version 1809 (build 17763) and later, x64.

## 1. One-time setup (Vlad)

1. Partner Center, Apps and games, New product, MSIX or PWA app: reserve the
   name **Valet**. The manifest's display name is `Valet`; if the reserved
   name is different, the `DisplayName` values in `AppxManifest.xml` must be
   changed to match it, or the upload is rejected.
2. Partner Center, the app, Product management, **Product identity**. Copy
   three values into GitHub, repository Settings, Secrets and variables,
   Actions, tab **Variables** (not Secrets; these values are public, they are
   in every package):

   | Partner Center field | GitHub Actions variable |
   |---|---|
   | Package/Identity/Name | `VALET_MSIX_IDENTITY_NAME` |
   | Package/Identity/Publisher (starts with `CN=`) | `VALET_MSIX_PUBLISHER` |
   | Package/Properties/PublisherDisplayName | `VALET_MSIX_PUBLISHER_DISPLAY_NAME` |

Until they are set, the workflow uses fake values (`Valet.Dev`,
`CN=ValetDev`) and shows a warning annotation "NOT FOR THE STORE". A package
with the fake identity is rejected by Partner Center.

## 2. Running the workflow

| Trigger | What it does |
|---|---|
| push to `main` touching `packaging/msix/**`, `scripts/msix-*` or the workflow | dry run |
| Actions, MSIX (Microsoft Store), Run workflow, `dry_run` checked (default) | dry run on the chosen branch or tag |
| Run workflow on a tag `vX.Y.Z`, `dry_run` unchecked | the package for the Store |

With the GitHub CLI: `gh workflow run msix.yml -f dry_run=true`, or for the
Store package `gh workflow run msix.yml --ref v1.0.0 -f dry_run=false`.

Each run:

1. checks that all version files agree (and match the tag on a tag);
2. checks that the build has no updater config (`scripts/msix-manifest.mjs`):
   no `plugins.updater` or `bundle.createUpdaterArtifacts` in
   `tauri.conf.json`, no `tauri.updater.conf.json`, no `TAURI_CONFIG`; a Store
   run also needs the three variables and a stable version;
3. builds `Valet.exe` with `pnpm tauri build --no-bundle`;
4. packs it with `makeappx` (`scripts/msix-pack.ps1`) into
   `Valet_<version>_x64.msix`, unsigned, after checking that `Valet.exe`
   contains no updater public key;
5. install smoke (`scripts/msix-smoke.ps1`): makes a throwaway self-signed
   certificate inside the job (never exported, removed at the end, not a
   project key), signs a copy of the package, installs it, starts it through
   `shell:AppsFolder\<PackageFamilyName>!App`, checks that it runs for 10 s
   and creates its database, prints where the database is, removes the
   package;
6. uploads the unsigned package as the artifact
   `valet-<version>-msix-x64` (Store run, kept 30 days) or
   `valet-<version>-msix-x64-dryrun` (kept 14 days).

The results are in the run's annotations: `msix pack`, `msix sign`,
`msix install`, `msix start`, `msix remove`, `msix result`.

The PLAN says "built from the release output". The installer build in
`release.yml` merges the updater config into `Valet.exe`, so that binary
cannot go into the Store. The MSIX workflow builds the same tag with the
same command, without the updater config.

## 3. Submitting in Partner Center

1. Download the artifact of the Store run and unzip it: it contains
   `Valet_X.Y.Z_x64.msix`.
2. Partner Center, the app, Start your submission (or Update for later
   versions).
3. Packages: drop the `.msix`. Partner Center checks the identity and the
   version. Each submission needs a higher version than the last one.
4. Submission options, restricted capabilities: `runFullTrust` needs a
   reason, for example "Valet is a Win32 desktop app (Tauri) that watches
   and moves files in folders the user chooses; it needs full trust for
   file system access and the Recycle Bin."
5. Store listings in Romanian and English (description, screenshots),
   pricing (free; Pro is bought inside the app through Valet's own
   checkout, which Store policy allows for non-game apps), age rating,
   category Utilities and tools, system requirements (see section 6).
6. Optional and recommended for updates: Packages, "Roll out update
   gradually", so a bad version can be halted (section 7).
7. Submit for certification.

## 4. Versions

- The package version is `X.Y.Z.0` from `tauri.conf.json` `X.Y.Z`. The Store
  requires the fourth part to be 0, and each part is at most 65535.
- Only stable versions go to the Store. A Store run refuses a pre-release
  version (`1.0.0-beta.1`). Dry runs map a pre-release to `X.Y.Z.0` too,
  so `1.0.0-beta.1`, `1.0.0-beta.2` and `1.0.0` would all be `1.0.0.0`; that
  is why such packages must never be submitted. Betas go through GitHub
  Releases and the beta update channel (docs/RELEASE.md).
- If Store betas (package flights) are wanted later, they need their own
  increasing `X.Y.Z` (for example flights only for patch versions that are
  then released unchanged as stable). Not set up now.

## 5. What differs in the Store build

| | Installer build (GitHub, winget) | Store build |
|---|---|---|
| Updates | in-app updater (`tauri-plugin-updater`) | the Store; no updater config in the binary (checked in CI) |
| Auto-start | `HKCU\...\Run` value written by Valet | startup task `ValetStartup` in the manifest, off by default; the user turns it on in Settings, Apps, Startup (or Task Manager, Startup apps). Writes to `HKCU` from a packaged app are virtualized, so a Run value would have no effect. On Windows 10 2004 and later the task starts Valet with `--minimized` (`uap10:Parameters`) |
| App data | `%APPDATA%\ro.valetapp.desktop`, `%LOCALAPPDATA%\ro.valetapp.desktop` | Windows redirects new files in AppData to `%LOCALAPPDATA%\Packages\<PackageFamilyName>\LocalCache\Roaming\...` and `...\LocalCache\Local\...`; the `msix start` annotation shows the path found. Uninstalling the Store app removes that folder, with the journal and the undo history |
| Install folder | `%LOCALAPPDATA%\Valet` | `C:\Program Files\WindowsApps\<PackageFullName>` (read-only) |
| Code signing | unsigned until a certificate exists (SmartScreen warning) | signed by the Store |

The app detects the Store build with `valet_platform::is_packaged()`
(package identity present). Expected behaviour there: no update check and no
update settings; the auto-start switch does not write the Run key but points
to Windows Settings (`ms-settings:startupapps`) or uses the WinRT
`StartupTask` API for `ValetStartup`.

## 6. WebView2

An MSIX package cannot run the WebView2 bootstrapper that the NSIS installer
uses (`webviewInstallMode: downloadBootstrapper`). The Store build relies on
the WebView2 Runtime already installed: Windows 11 includes it, and Windows 10
gets it through Windows Update. Without it the app cannot open its window.
In the Store listing, system requirements: Windows 10 version 1809 or later
(the manifest minimum, `MinVersion 10.0.17763.0`), x64, Microsoft Edge
WebView2 Runtime (included in Windows 11 and in updated Windows 10).

## 7. Rollback

A Store version cannot be replaced by a lower version number.

- With a gradual rollout in progress: Partner Center, the submission, Halt
  rollout. Users who have not received the update stay on the previous
  version.
- Otherwise, or for users who already have the bad version: build the last
  good code under a higher version (for example revert on `main`, release
  `X.Y.Z+1` with the normal steps in docs/RELEASE.md), run this workflow on
  the new tag with `dry_run` unchecked and submit it.
- A submission still in certification can be cancelled in Partner Center.

## 8. Known limits

- The logos are the placeholder icons from `apps/desktop/src-tauri/icons/`
  (docs/RELEASE.md section 7). The package has no `resources.pri`, so there
  are no scale or "unplated" logo variants; the taskbar shows the 44x44 logo
  on a plate. No wide tile.
- Not yet checked in a Store-installed copy: Windows notifications
  (`tauri-plugin-notification`) under the package identity, and what happens
  when data of an earlier installer version already exists in
  `%APPDATA%\ro.valetapp.desktop`.

## 9. Files

| File | Purpose |
|---|---|
| `packaging/msix/AppxManifest.xml` | manifest template (identity and version placeholders) |
| `scripts/msix-manifest.mjs` | identity, version mapping, updater checks, fills the template |
| `scripts/msix-pack.ps1` | package layout and `makeappx pack` |
| `scripts/msix-smoke.ps1` | install smoke with a throwaway certificate (CI or a test VM) |
| `.github/workflows/msix.yml` | the workflow |
