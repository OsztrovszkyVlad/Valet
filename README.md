# Valet

Valet is a rules-based file organizer for Windows, in the spirit of Hazel on macOS. You define rules (conditions on a file, and an action), Valet lists what each rule would do to the files in your watched folders, and then performs the actions. Every batch can be undone.

It is a pre-release prototype. No version has been published yet; the release workflow builds a per-user Windows installer (see [Install](#install-windows)). The code is a Rust workspace plus a Tauri 2 desktop app (React 19, TypeScript, Vite). The product plan is in [docs/PLAN.md](docs/PLAN.md) (in Romanian).

User guides (for people who use the app, not for programmers):

- [docs/USER_GUIDE.ro.md](docs/USER_GUIDE.ro.md) (Romanian, the primary one)
- [docs/USER_GUIDE.en.md](docs/USER_GUIDE.en.md) (English)

**Pro and Free.** Valet has a 14-day Pro trial that starts at the first run, then a Free plan (3 own rules enabled, 2 watched folders active, History showing 7 days, undo of the newest organize batch only, no rule export or import) and a Pro plan (29 EUR with VAT included, one-time, 12 months of updates). Nothing is deleted when a Free limit applies; a dialog lets you pick which rules and folders stay active. Licenses are not active in the current builds: no license server is configured, so buying and activating a license do not work yet, while the trial and the Free plan do. The user guide has the details, including importing and removing a license, working offline and the Microsoft Store build: [section 18](docs/USER_GUIDE.en.md#18-valet-pro-and-the-free-plan) ([Romanian](docs/USER_GUIDE.ro.md#18-valet-pro-și-planul-gratuit)). The contract between the code parts is [docs/LICENSING.md](docs/LICENSING.md).

Other documents: [docs/PRIVACY.md](docs/PRIVACY.md) ([Romanian](docs/PRIVACY.ro.md)): what goes over the network and what is stored; [docs/SAFETY.md](docs/SAFETY.md): what Valet does to files and how that is tested; [docs/RELEASE.md](docs/RELEASE.md): how a version is released; [CHANGELOG.md](CHANGELOG.md).

![The Preview screen: the list of operations Valet would perform](docs/images/preview.png)

The screenshot is from the automatic UI smoke test, taken on Linux with test folders (the paths start with `/tmp/valet-e2e-…`). The app is in Romanian, its default language.

## Install (Windows)

No release has been published yet. When one is, the installer is on [GitHub Releases](https://github.com/OsztrovszkyVlad/Valet/releases) as `Valet_<version>_x64-setup.exe`. The release workflow (`.github/workflows/release.yml`) builds it; a dry run at `1d910c9` produced `Valet_0.1.0_x64-setup.exe` (4.94 MB) as a workflow artifact and published nothing. The installer was not run on Windows in the sessions that wrote this file.

What the installer is set up to do (`apps/desktop/src-tauri/tauri.conf.json`, NSIS):

- Installs for the current user only, with no administrator rights. The folder is `%LOCALAPPDATA%\Valet` (the Tauri NSIS template's default for a per-user install).
- Offers English and Romanian in a language selector, adds a Start menu folder "Valet" and an optional desktop shortcut. The shortcut is offered on the last page of an interactive install; a silent install (`/S`, winget, scripts) does not create one (`packaging/nsis/hooks.nsh`).
- Downloads the Microsoft WebView2 bootstrapper if WebView2 is missing (the only case where the installer needs the internet).
- The uninstaller keeps your data unless you tick "Delete the application data". Details in [docs/PRIVACY.md](docs/PRIVACY.md), section 4.

**SmartScreen warning.** The installer and the app are not code-signed yet, so Windows SmartScreen shows "Windows protected your PC" when you run the downloaded installer. To go on, select "More info", then "Run anyway". Windows shows it for programs that are unsigned and not widely downloaded. The reason here is that no code-signing certificate exists yet: the plan is a free certificate for open-source projects (SignPath Foundation or Certum Open Source, PLAN 3.3) and neither has been obtained. If you want to check the download, the Release workflow run lists the SHA-256 of the installer; compare it with `Get-FileHash Valet_<version>_x64-setup.exe -Algorithm SHA256`.

**Updates.** Settings, Updates shows the installed version and has a channel list, Stable or Beta, and a "Check now" button. Beta gets new versions before they are marked stable. When updates are configured, Valet also checks 30 seconds after it starts and then every 24 hours while it runs, unless you uncheck "Check for updates automatically" in Settings, Updates (checked by default); a found update is shown as a notification and a mark on Settings, and is installed only after you press "Install and restart" and confirm. The installation waits for a running batch of file operations to finish. The installer's signature is checked with a minisign public key built into the app. Updates exist only in builds made with the signing keys (`docs/RELEASE.md`, section 0.2); the dry run at `1d910c9` was built without them (its log says "updater artifacts off"), and a build without them answers "Updates are not configured in this version". `pnpm tauri dev` builds are of that kind.

The network requests Valet makes are these update checks and downloads, to the two GitHub addresses listed in [docs/PRIVACY.md](docs/PRIVACY.md), and the license check, which happens only if a Pro license is imported in a build with a license server configured (the current builds have none). The license check sends the license id, a one-way hash of the computer id and the app version. No file name, path or content is sent.

**Microsoft Store build.** An MSIX package is built in CI (`packaging/msix/README.md`); it is not published and was not tried in a Store install. In that build updates come from the Store (Valet makes no version check), starting with Windows is set in the Windows settings (Settings has a button that opens them), and uninstalling the app from the Store deletes its data, including History and undo.

**winget.** Manifest templates are in `packaging/winget/templates/` and [docs/RELEASE.md](docs/RELEASE.md), section 4, describes the submission. As far as this repository shows, nothing has been submitted.

## Status

Phase 2 is built. Phase 3 (delivery) is in progress, and Phase 4 (licensing and plans) is in the code (see "Phase 4" below): the installer configuration, the release workflow, the in-app updater, rotating logs and the panic hook, unavailable watched folders and the upgrade tests are in the code; code signing, crash reports, a published release and winget are not done (see "Phase 3" below). What works today, with the tests that cover each point in [docs/SAFETY.md](docs/SAFETY.md):

- **First run.** A 3-step guide: choose watched folders (Downloads and Desktop are pre-checked; a language list at the top of step 1 switches the UI and saves the language, which the preset texts and folder names follow), choose presets, see the first preview. Nothing moves in it.
- **Presets.** The 8 presets of PLAN 2.4 ship as JSON files in `crates/valet-core/presets/`, each with an integration test on a synthetic home folder (`crates/valet-core/tests/presets.rs`): Downloads by type, Screenshots, Invoices and receipts, Large files (report only), Duplicates (report only), Clean desktop, Old installers (to the Recycle Bin) and Empty folders (to the Recycle Bin). Four are checked by default; none that trashes is. Screenshots and Invoices match whole words in the file name.
- **Rules from the UI.** The Rules screen lists, creates, edits, turns on and off, and deletes rules. A rule is validated when it is saved (stable error codes, translated in the UI). "Add presets" installs presets later (installing one that is already installed resets its rules to the shipped version). `Db::list_rules` lists the user's rules before the preset rules, so a rule the user writes wins over an overlapping preset whenever it was written; the first enabled rule in that order that moves, copies or trashes a file wins.
  - Conditions: `ext`, `extIn`, `extNotIn`, `typeGroup`, `nameContains`, `nameMatches` (regex), `pathContains`, `sizeGt`, `sizeLt`, `modifiedOlderThanDays`, `createdOlderThanDays`, `emptyFolder`, `duplicate`.
  - Actions: `moveTo`, `copyTo`, `trash` (Recycle Bin, never a permanent delete) and `report` (list the file in the preview, no operation). `tag` and `rename` are not implemented; a rule with such an action is refused when saved.
  - Destination folders may use `~`, `%VAR%`, the folder tokens `{home}`, `{desktop}`, `{documents}`, `{downloads}`, `{pictures}`, `{videos}`, `{music}` (first component only) and `{year}`, `{month}`, `{day}`, `{ext}` (file modification date and extension).
  - A rule can be limited to some watched folders (`scope`) and to files directly in the folder (`topLevelOnly`).
- **Preview and Organize.** `dry_run` walks the watched folders (`planner::plan`) and lists the planned moves, copies and trashes, the report entries (large files, duplicate groups) and the files it skipped with the reason. It changes no file, keeps the plan in memory and returns a `plan_id`. `execute_operations` takes that `plan_id` and runs exactly the previewed operations through the operation journal; it does not plan again. The plan can be used once; without a matching preview the command returns `preview_required` and does nothing. Each operation is journaled, checked again right before it runs, and never replaces an existing file.
- **Trash as an action.** A file, or an empty folder, goes to the Recycle Bin through the `trash` crate after prechecks (placeholder, exists, unchanged, folder still empty, not locked, `can_trash`). A volume without a usable Recycle Bin is refused.
- **Undo.** `undo_batch` undoes a finished batch. Moved and copied files go back (a changed file or an occupied original path asks for confirmation). Trashed files and folders are restored from the Recycle Bin (journaled `restore` operations, migration 0004) without replacing anything. The History screen lists batches (the newest 50) and any finished batch can be undone from it. On startup the app recovers operations that a crash left unfinished.
- **Automatic organizing.** Off by default and available only after the user has approved a preview. It runs when the watcher reports changes, every 10 minutes (full rescan), and after the folder list changes. It executes only rules approved at their current version under the watched folder of the file (`approvals.json` keeps, per rule, the version and the watched folders of the approving preview), waits while a preview is open or the tray pause is on, and retries a locked file on the next 3 passes. A watched folder that is added, or removed and added again, needs a preview before automatic organizing touches it.
- **Tray, auto-start, notifications.** Closing the window hides it; the tray menu has Show, Pause and Quit. Auto-start writes a per-user `HKCU\...\Run` value and starts Valet hidden (`--minimized`). A short notification follows each batch unless it is turned off. Only one Valet process runs at a time (`tauri-plugin-single-instance`); a second launch shows the window of the first.
- **Settings.** Watched folders (a drive root such as `C:\` and a path with a default-excluded folder name such as `AppData` are refused; the same checks run when `config.json` is read, and a hand-edited entry that fails is not watched, stays in the file and is listed in Settings with the reason). A folder that does not exist right now, for example an unplugged drive, is saved, marked "Unavailable right now" in Settings and skipped; it is watched again when it exists (see below). Language (Romanian default, English), notification, auto-start and automatic organizing. Phase 3 added three sections: Updates (installed version, channel, check, install), Support (the detailed log switch) and About (version, license, repository address).
- **Interface text.** UI text comes from `apps/desktop/src/i18n/ro.json` and `en.json`; `pnpm run check:i18n` fails if the two files differ in keys or if a literal `t('key')` has no entry.

Phase 4 (licensing and plans), what is in the code at `fc6acc0`:

- **Plans.** The 14-day trial, the Free limits (3 own rules, 2 folders, 7 days of History, undo of the newest organize batch), the "Choose what stays active" dialog, and rule export and import for Pro and the trial. `crates/valet-core/src/entitlements.rs` and `license.rs` hold the pure logic; `apps/desktop/src-tauri/src/licensing.rs` and `commands/license.rs` apply it. Limits are enforced in the backend commands; the UI mirrors them. Tests: `cargo test -p valet-core --test license`, `cargo test -p app -- licensing:: license_client::`.
- **License.** A signed license file (minisign) is imported from a file or pasted text, activated online for up to 3 computers, checked again at most every 30 days, with a grace period that ends 60 days after the last successful check. "Remove the license" sends a deactivation request when it can and moves the two license files to `license-removed/` (never deletes them). The license service (`services/license/`, Node, no dependencies) has the activate, validate and deactivate calls and the `issue`, `reset` and `revoke` tools; `licensing::service_tests` run the app against it on localhost. `crates/valet-core/license/api.txt`, `pubkey.txt` and `checkout.txt` are empty, so nothing contacts a license server and "Buy Pro" is disabled.
- **Update setting and Store build.** `auto_update_check` in `app_settings.json` (default on). The MSIX package (`packaging/msix/`), `valet_platform::is_packaged()`, and the Store-build differences listed under Install.
- **Startup.** The database is opened while the main window is created; journal recovery still finishes before the plan is resolved and the watcher starts (`setup.rs`).
- **Installer.** No desktop shortcut on silent installs; the installer language is stored on silent installs so that passive updates show no language dialog.

Phase 3 (in progress), what is in the code at `1d910c9`:

- **Installer.** NSIS, per user, English and Romanian, WebView2 bootstrapper (`tauri.conf.json`; Romanian installer texts in `packaging/nsis/Romanian.nsh`). The window is 1100x750 (minimum 900x600) and the web view has a content security policy that allows only its own files and the Tauri IPC. The icon is a placeholder.
- **Release workflow.** A tag `v*` builds the installer and creates a draft GitHub release; a manual dry run builds it as a workflow artifact. Version scripts (`scripts/set-version.mjs`, `scripts/check-version.mjs`) keep `tauri.conf.json`, `package.json`, `Cargo.toml` and `Cargo.lock` equal. The procedure is in [docs/RELEASE.md](docs/RELEASE.md). The dry runs at `1350d62` and `1d910c9` succeeded; no tag and no release exist.
- **Updater.** `apps/desktop/src-tauri/src/updater.rs`: the commands `get_app_version`, `check_for_update` and `install_update`, a check 30 seconds after start and every 24 hours, one endpoint per channel. Install takes the `file_ops` lock without waiting, so it is refused (`update.busy`, the UI retries) while a batch runs, and no batch starts once the install holds the lock. Tests: `cargo test -p app updater::` (9 tests). The plugin is initialized only when the build config has `plugins.updater`.
- **Logs and panic hook.** `logging.rs`: `valet.log` in the app log folder, 5 files of at most 2 MB, INFO by default, DEBUG from the Settings switch without a restart; a panic is logged with absolute paths replaced and the previous hook still runs. The hook does not touch the database; interrupted operations are decided by recovery at the next start. Tests: `cargo test -p app logging::` (5 tests).
- **Unavailable folders.** `save_config` and onboarding no longer refuse a folder that does not exist. `get_config` returns `unavailable_paths` next to `invalid_paths`. Manual previews and `save_config` read the disk each time; the automatic loop reads the folder list again before each full rescan (every 10 minutes), so a folder that comes back is watched again without a restart and keeps its approvals. Tests: `commands::settings::tests::unavailable_folder_is_picked_up_again_when_it_reappears`, `auto::tests::rescan_picks_up_folders_that_reappear_or_disappear`, `pass_over_an_unavailable_folder_skips_it`.
- **Statistics removed.** The commands `get_statistics` and `clear_operation_history` and the table `operation_history` are gone (migration `0005_drop_operation_history.sql`). The journal is the only history.
- **Upgrade tests.** `crates/valet-core/tests/upgrade_journal.rs` builds the database of a Phase 1 and of a Phase 2 install from the shipped migrations, opens it with the current code, and checks that journal, index and rule rows are unchanged, the old table is gone, nothing needs recovery and an old batch still undoes to an identical tree. It runs on Linux.

Not done in Phase 3: code signing, crash reports (Sentry is not integrated), a published release, a winget submission, and the install, update and uninstall test on Windows 10 and 11 virtual machines.

Known gaps. The details, with the tests that cover each case, are in [docs/SAFETY.md](docs/SAFETY.md):

- Windows CI. At `2f398fb` three Windows test steps failed on test-helper and temp-folder problems, and at `6cf4f19` four Recycle Bin restore tests failed on the runner. After fixes (`487f1b6`, `998817f`, `2da3329`, `c5acb7d`) the three Windows test steps passed for `c5acb7d`, and the job `workspace (Windows)` succeeded for `d36cc51`; no log was readable, so these are step-level and job-level results. Most runs after `d36cc51` were cancelled by newer pushes, and for `1d910c9` the job `workspace (Windows)` had not finished when this was written, so there is no Windows test result for the Phase 3 code. The Windows UI smoke test (report only) failed at `d36cc51` at "start tauri-driver and the app" with `session not created: DevToolsActivePort file doesn't exist`; `e7ab643` gave the e2e build a WebView2 DevTools port through `TAURI_CONFIG`, and the smoke test passed at `1d910c9`. Restore has been confirmed on the CI runner's Recycle Bin only.
- A preview approves its rules for all watched folders at once; there is no choice per rule or per folder, and the app does not list the operations an automatic pass left waiting (it logs their count; the next preview shows them).
- On Windows, `notify` 6.1 does not report a directory-change buffer overflow. The 10-minute full rescan picks up lost changes, so a missed file can wait up to 10 minutes.
- A locked file is retried only in automatic passes, and the retry count is kept in memory.
- Rules cannot be reordered from the UI, and undo works on whole batches only.
- The watched-folder check is on path text and runs when folders are saved and when `config.json` is read; the planner does not repeat it for the scan root, and a destination folder that is a junction into an excluded folder is not detected.
- Two cases can leave a file at both the source and the destination after a double failure (never zero copies); see SAFETY.md, invariant 3.
- Debug-level log lines can contain paths (Settings, Support, "Detailed log"; off by default, local only). `logging::level_for` gives Info in release builds unless that switch is on (`logging::tests::level_follows_debug_logging`); no installed release build was run to check what it writes. No test checks the text of INFO lines; WARN and ERROR lines print some operating system and database error texts that were not checked for paths.
- Nothing is installed without a click. The automatic update check can be switched off in Settings.
- No license server is deployed, so no license has been activated outside tests, and nothing about licensing was run on Windows. The merchant (Paddle, Polar or Stripe) and the checkout address are not chosen.
- No release exists, so the update path (install of a newer version over an older one with the NSIS installer) has not run. The journal side of an upgrade is tested on Linux (`upgrade_journal.rs`).
- No code signing: Windows SmartScreen warns on the installer (see Install).
- Real OneDrive and Dropbox placeholders, USB drives, network shares and power loss have not been tested.

Do not point this build at folders whose contents you cannot lose.

## Repository layout

- `crates/valet-core`: rules, matcher, planner, collision naming, executor (with startup recovery and undo), operation journal, SQLite storage and the shipped presets (`presets/*.json`). No OS-specific code; builds and tests on Linux.
- `crates/valet-platform`: the Windows layer: Recycle Bin (`trash` crate) with restore, cloud placeholder, hidden, reparse-point and lock checks, volume check, no-replace rename, `\\?\` long paths, per-user auto-start, Explorer reveal and the file watcher (`notify` with debouncing). Off Windows the functions return neutral answers so the workspace builds on Linux.
- `apps/desktop/src-tauri`: Tauri commands, split by module under `src/commands/`, shared state in `src/state.rs`, the auto-apply loop in `src/auto.rs`, the tray in `src/setup.rs`.
- `apps/desktop/src`: React and TypeScript front end (views for Preview, Rules, History, Settings and the first-run guide).
- `docs/USER_GUIDE.ro.md`, `docs/USER_GUIDE.en.md`: user guides. `docs/SAFETY.md`: what Valet does to files and how that is tested. `docs/PRIVACY.md`, `docs/PRIVACY.ro.md`: network use and stored data. `docs/RELEASE.md`: releasing, update channels, rollback, winget. `docs/images/`: screenshots from the UI smoke test. `docs/PLAN.md`: product and architecture plan. `docs/reports/`: phase reports.
- `.github/workflows/release.yml`, `scripts/` (version, release notes, updater config, winget manifest scripts), `packaging/` (NSIS Romanian texts, winget templates): the release pipeline. `CHANGELOG.md`: changes per version.
- `CLAUDE.md`: working rules and safety invariants for the project.

## Build on Windows

Windows is the only supported target.

1. In an elevated PowerShell, run `scripts/windows_dev_setup.ps1`. It installs Git, Node.js LTS, Rustup and Visual Studio 2022 Build Tools with winget.
2. Finish the manual steps the script prints:
   - Open the Visual Studio Installer and add the "Desktop development with C++" workload, with the Windows 10/11 SDK selected.
   - Run `rustup default stable-x86_64-pc-windows-msvc`. Inside the repository, `rust-toolchain.toml` pins Rust 1.99.1 (with clippy and rustfmt), and rustup uses that version instead of the default.
   - Run `corepack enable`.
3. Start the app:

```
cd apps/desktop
pnpm install
pnpm tauri:dev
```

`pnpm tauri:dev` is the script in `apps/desktop/package.json`; it runs `tauri dev`.

The Windows setup script and `pnpm tauri:dev` were not run in the session that wrote this README (it ran on Linux).

## Build and test (dev)

From the repository root:

```
cargo build --workspace
cargo test --workspace
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

Clippy (with `-D warnings`), fmt and the tests are the required checks before a commit. CI runs them on `ubuntu-latest` (valet-core and valet-platform) and `windows-latest` (whole workspace, after `pnpm build`), and runs `pnpm lint`, `pnpm test`, `pnpm check:i18n`, a type check and `pnpm build` on `ubuntu-latest`; see `.github/workflows/ci.yml`.

Front end:

```
cd apps/desktop
pnpm install --frozen-lockfile
pnpm build
pnpm test
pnpm lint
pnpm run check:i18n
```

Tests that exist:

- 52 characterization tests in `crates/valet-core/tests/characterization_*.rs`. They pin the matcher and dry-run behavior. A change that alters that behavior should change these tests on purpose.
- Unit tests next to the code in `valet-core` (planner, matcher, executor with recovery, undo and trash, journal, collision naming, presets) and in `valet-platform`. `cargo test -p valet-core -p valet-platform` runs them on Linux.
- `crates/valet-core/tests/presets.rs`: each of the 8 presets on a synthetic home folder (install, plan, exact operations, execute, undo, compare the tree), and the interactions between presets.
- Property tests for the planner and for collision naming (`prop_planner.rs`, `prop_collision.rs`).
- `upgrade_journal.rs`: databases left by a Phase 1 and a Phase 2 install are upgraded by the current code; rows are unchanged, `operation_history` is gone, an old batch undoes to an identical tree.
- `fault_injection.rs`: kills a `valet-fault` child process at a random time during a batch (including batches that send files to a fake Recycle Bin), runs recovery, checks that every file exists once, then undoes the batch. 10 iterations by default; `VALET_FAULT_ITERS=100` sets more. It uses the fake platform and runs on Linux.
- `integration_10k.rs`: plan, execute and undo on a generated tree of 1,000 and 10,000 files (about a minute in a debug build). The perf tests (`perf_*.rs`) are `#[ignore]`.
- `crates/valet-platform/tests/windows.rs` and `windows_e2e.rs` run only on Windows (`windows_e2e.rs` needs `--features valet-core`).
- Unit tests in the app crate (`apps/desktop/src-tauri`): `cargo test -p app`. They cover the preview-only execution, rule validation, settings and approvals, presets, the auto-apply scheduler, gate and locked-file retries, the updater (channel endpoints, 24-hour timer, busy install) and the log setup and panic hook.
- Front-end tests (vitest, `pnpm test`) for the views, the rule model and the i18n helpers. `pnpm lint` runs ESLint on `src`.
- A UI smoke test (`tests/e2e/smoke.mjs`, started by `scripts/e2e-linux.sh`) drives the built app with `tauri-driver` through onboarding, preview, organize, undo, history, Rules and Settings, and takes the screenshots in `docs/images/`. On Linux it needs the app built with `pnpm build` and `cargo build -p app --features tauri/custom-protocol`, `tauri-driver`, `WebKitWebDriver` and Xvfb (see the header of the script). CI runs it on Linux; a Windows run is report-only. It passed in the session that updated this README, on Linux, with the screenshots written to a scratch folder.

On Linux, building the Tauri crate needs the webkit2gtk-4.1 development packages. This is only for building and testing; the app targets Windows.

## Rule format

Rules are JSON objects, stored in SQLite and created by the Rules screen. This is one, as the app accepts it (`valet_core::rules::Rule`, validated by `apps/desktop/src-tauri/src/commands/rules.rs`):

```json
{
  "id": "6f1f3b0e-8d4a-4c8e-9a53-1f0c2d7b5a10",
  "name": "Invoices to Documents",
  "enabled": true,
  "conditions": [
    { "type": "ext", "value": "pdf" },
    { "type": "nameContains", "value": ["factura", "invoice"] }
  ],
  "actions": [
    { "type": "moveTo", "params": { "path": "{documents}/Invoices/{year}" } }
  ],
  "scope": ["{downloads}"],
  "options": { "topLevelOnly": true }
}
```

Fields:

- `id`: UUID.
- `name`: display name, not blank.
- `enabled`: a disabled rule never matches.
- `alwaysApply`: optional, default `false`. When `true`, the rule matches every file and needs no conditions.
- `conditions`: list of `{ "type", "value" }`. All conditions must hold (they are ANDed). At least one is required unless `alwaysApply` is set.
- `actions`: list of `{ "type", "params" }`, at least one. At most one of `moveTo`, `copyTo` and `trash`.
- `version`: optional, default `1`. `upsert_rule` raises it when anything else in the rule changes (including `enabled`). Automatic organizing runs a rule only at the version the user approved.
- `options`: optional object. `includeHidden` and `topLevelOnly` are booleans; other keys are kept and ignored (presets store `presetId`).
- `scope`: optional list of watched folders the rule applies to (a folder token such as `{downloads}`, `~`, `%VAR%` or an absolute path). Empty means all watched folders.

Conditions (text comparisons ignore case unless noted):

| `type` | `value` |
|---|---|
| `ext` | one extension without the dot |
| `extIn`, `extNotIn` | non-empty list of extensions; `extNotIn` also holds for files without extension |
| `typeGroup` | `image`, `document`, `archive`, `installer`, `video`, `audio` or `code` (lists in `crates/valet-core/src/typegroups.rs`) |
| `nameContains` | a text, or a list of texts (any of them), in the file name |
| `nameMatches` | a regular expression (Rust `regex` syntax) on the file name; case-sensitive unless it starts with `(?i)` |
| `pathContains` | a text in the full path |
| `sizeGt`, `sizeLt` | a size in bytes |
| `modifiedOlderThanDays`, `createdOlderThanDays` | whole days, 0 to 36500; the creation condition is false where the OS gives no creation time |
| `emptyFolder` | `true`; the rule then sees only empty folders, never a watched folder itself; it cannot be combined with file conditions or with `moveTo`/`copyTo` |
| `duplicate` | `true`; the file has a byte-identical copy among the files the other conditions match; with `trash` every copy except the oldest goes to the Recycle Bin; it cannot be combined with `moveTo`/`copyTo` |

Actions: `moveTo` and `copyTo` take `{ "path": "<destination folder>" }`; the file name is appended and a taken name becomes `name (2).ext`. A relative folder is resolved against the watched folder the file was found in. `trash` and `report` take no parameters.

A condition or action type the code does not know is refused by `upsert_rule`. An unknown condition type stored earlier never matches.

## Safety

The invariants are listed in `CLAUDE.md` under "Safety invariants" and in [docs/PLAN.md](docs/PLAN.md) section 2.6: never delete permanently, never overwrite, journal every operation before it runs, undo while destination files are unchanged, default exclusions, skip files being written and unhydrated cloud placeholders, no network use for file data.

[docs/SAFETY.md](docs/SAFETY.md) says, for each invariant, how the code meets it and which tests cover it, what happens in each edge case (including trash as an action, undo by restore, automatic organizing and the approvals), how the journal works, and what is not covered yet. Where the code does less than the plan, it says so. Short version:

- Met in code and covered by tests that run on Linux with a fake Recycle Bin: journal before each operation, recovery after a crash (unit tests and a process-kill test with a trash variant), no replacing of existing files, undo with a hash check, restore of trashed files, default exclusions below a scan root and for destinations, skipping of placeholders, locked and recently written files, executing only the previewed plan, and executing automatically only rules approved at their current version under the file's watched folder.
- Covered only by tests that run on Windows (the CI job `workspace (Windows)`: its test steps passed at `c5acb7d` and the job succeeded at `d36cc51`; for the Phase 3 commits there is no result yet), or not at all: the Windows API calls themselves (including the Recycle Bin restore), power loss, and real OneDrive or Dropbox placeholders.
- Partly met: the scan root is checked when it is saved and when `config.json` is read, not at scan time, and destination checks work on path text only; "being written" is a name and modification-time heuristic; copies and cross-volume moves need a Recycle Bin on the destination volume (and on the source volume for moves); paths that are not valid Unicode are refused.
- Not met: Windows directory-change overflow is not reported by `notify` 6.1 (mitigated by the 10-minute rescan), a preview approves all watched folders at once, and two double-failure cases can leave a file at both source and destination.
- Network: the code that opens a connection is the updater (`updater.rs`) and the license client (`license_client.rs`). The updater asks one of two GitHub addresses for a static JSON file and downloads the installer after a click. The license client sends `license_id`, `machine` and `app_version` and only runs when `api.txt` and `pubkey.txt` are filled in (they are empty now). No request has a file name, path or content ([docs/PRIVACY.md](docs/PRIVACY.md)). Crash reports are not built, and no test fails if another network call is added.
- Updates: the installer's signature is checked by `tauri-plugin-updater` with a minisign public key (read from the plugin source; Valet has no test of that check). An install is refused while a batch runs (`updater::tests::busy_while_file_ops_lock_is_held`).

## License

MIT, see [LICENSE](LICENSE).
