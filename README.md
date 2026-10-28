# Valet

Valet is a rules-based file organizer for Windows, in the spirit of Hazel on macOS. You define rules (conditions on a file, and an action), Valet lists what each rule would do to the files in your watched folders, and then performs the actions. Every batch can be undone.

It is a pre-release prototype. There is no installer and no release. The code is a Rust workspace plus a Tauri 2 desktop app (React 19, TypeScript, Vite). The product plan is in [docs/PLAN.md](docs/PLAN.md) (in Romanian).

User guides (for people who use the app, not for programmers):

- [docs/USER_GUIDE.ro.md](docs/USER_GUIDE.ro.md) (Romanian, the primary one)
- [docs/USER_GUIDE.en.md](docs/USER_GUIDE.en.md) (English)

![The Preview screen: the list of operations Valet would perform](docs/images/preview.png)

The screenshot is from the automatic UI smoke test, taken on Linux with test folders (the paths start with `/tmp/valet-e2e-…`). The app is in Romanian, its default language.

## Status

Phase 2 is built. What works today, with the tests that cover each point in [docs/SAFETY.md](docs/SAFETY.md):

- **First run.** A 3-step guide: choose watched folders (Downloads and Desktop are pre-checked), choose presets, see the first preview. Nothing moves in it.
- **Presets.** The 8 presets of PLAN 2.4 ship as JSON files in `crates/valet-core/presets/`, each with an integration test on a synthetic home folder (`crates/valet-core/tests/presets.rs`): Downloads by type, Screenshots, Invoices and receipts, Large files (report only), Duplicates (report only), Clean desktop, Old installers (to the Recycle Bin) and Empty folders (to the Recycle Bin). Four are checked by default; none that trashes is. Screenshots and Invoices match whole words in the file name.
- **Rules from the UI.** The Rules screen lists, creates, edits, turns on and off, and deletes rules. A rule is validated when it is saved (stable error codes, translated in the UI). A new rule goes to the end of the list, after the presets; the first enabled rule that moves, copies or trashes a file wins.
  - Conditions: `ext`, `extIn`, `extNotIn`, `typeGroup`, `nameContains`, `nameMatches` (regex), `pathContains`, `sizeGt`, `sizeLt`, `modifiedOlderThanDays`, `createdOlderThanDays`, `emptyFolder`, `duplicate`.
  - Actions: `moveTo`, `copyTo`, `trash` (Recycle Bin, never a permanent delete) and `report` (list the file in the preview, no operation). `tag` and `rename` are not implemented; a rule with such an action is refused when saved.
  - Destination folders may use `~`, `%VAR%`, the folder tokens `{home}`, `{desktop}`, `{documents}`, `{downloads}`, `{pictures}`, `{videos}`, `{music}` (first component only) and `{year}`, `{month}`, `{day}`, `{ext}` (file modification date and extension).
  - A rule can be limited to some watched folders (`scope`) and to files directly in the folder (`topLevelOnly`).
- **Preview and Organize.** `dry_run` walks the watched folders (`planner::plan`) and lists the planned moves, copies and trashes, the report entries (large files, duplicate groups) and the files it skipped with the reason. It changes no file, keeps the plan in memory and returns a `plan_id`. `execute_operations` takes that `plan_id` and runs exactly the previewed operations through the operation journal; it does not plan again. The plan can be used once; without a matching preview the command returns `preview_required` and does nothing. Each operation is journaled, checked again right before it runs, and never replaces an existing file.
- **Trash as an action.** A file, or an empty folder, goes to the Recycle Bin through the `trash` crate after prechecks (placeholder, exists, unchanged, folder still empty, not locked, `can_trash`). A volume without a usable Recycle Bin is refused.
- **Undo.** `undo_batch` undoes a finished batch. Moved and copied files go back (a changed file or an occupied original path asks for confirmation). Trashed files and folders are restored from the Recycle Bin (journaled `restore` operations, migration 0004) without replacing anything. The History screen lists batches (the newest 50) and any finished batch can be undone from it. On startup the app recovers operations that a crash left unfinished.
- **Automatic organizing.** Off by default and available only after the user has approved a preview. It runs when the watcher reports changes, every 10 minutes (full rescan), and after the folder list changes. It executes only rules approved at their current version (`approvals.json`), waits while a preview is open or the tray pause is on, and retries a locked file on the next 3 passes. The approval is per rule, not per folder.
- **Tray, auto-start, notifications.** Closing the window hides it; the tray menu has Show, Pause and Quit. Auto-start writes a per-user `HKCU\...\Run` value and starts Valet hidden (`--minimized`). A short notification follows each batch unless it is turned off. Only one Valet process runs at a time (`tauri-plugin-single-instance`); a second launch shows the window of the first.
- **Settings.** Watched folders (a drive root such as `C:\`, a path with a default-excluded folder name such as `AppData`, and a missing folder are refused), language (Romanian default, English), notification, auto-start and automatic organizing.
- **Interface text.** UI text comes from `apps/desktop/src/i18n/ro.json` and `en.json`; `pnpm run check:i18n` fails if the two files differ in keys or if a literal `t('key')` has no entry.

Known gaps. The details, with the tests that cover each case, are in [docs/SAFETY.md](docs/SAFETY.md):

- No passing Windows test result exists for the current code. The last CI run in which every job passed is for the Phase 1 report (`cc00f94`). For the latest Phase 2 commit checked, `2f398fb`, three Windows test steps failed. The annotated failures (capped at 10 per step, so possibly incomplete) are eight preset tests, two other core tests and two app tests, which reads as test-helper and temp-folder problems; the Windows end-to-end step has no per-test annotation. Earlier, at `6cf4f19`, four Recycle Bin restore tests failed on the runner. Restore from the Windows Recycle Bin is therefore unconfirmed.
- Approval for automatic organizing belongs to a rule, not to a folder. A watched folder added later is organized by the rules already approved, with no preview of that folder.
- On Windows, `notify` 6.1 does not report a directory-change buffer overflow. The 10-minute full rescan picks up lost changes, so a missed file can wait up to 10 minutes.
- A locked file is retried only in automatic passes, and the retry count is kept in memory.
- Rules cannot be reordered from the UI, a deleted preset cannot be reinstalled from the UI, and undo works on whole batches only.
- The watched-folder check is on path text and runs when folders are saved; the planner does not repeat it for the scan root, and a destination folder that is a junction into an excluded folder is not detected.
- Two cases can leave a file at both the source and the destination after a double failure (never zero copies); see SAFETY.md, invariant 3.
- Debug-level log lines contain full paths. `main.rs` sets the level to Debug in dev builds and to Info in release builds; no release build was made to confirm this.
- Real OneDrive and Dropbox placeholders, USB drives, network shares and power loss have not been tested.

Do not point this build at folders whose contents you cannot lose.

## Repository layout

- `crates/valet-core`: rules, matcher, planner, collision naming, executor (with startup recovery and undo), operation journal, SQLite storage and the shipped presets (`presets/*.json`). No OS-specific code; builds and tests on Linux.
- `crates/valet-platform`: the Windows layer: Recycle Bin (`trash` crate) with restore, cloud placeholder, hidden, reparse-point and lock checks, volume check, no-replace rename, `\\?\` long paths, per-user auto-start, Explorer reveal and the file watcher (`notify` with debouncing). Off Windows the functions return neutral answers so the workspace builds on Linux.
- `apps/desktop/src-tauri`: Tauri commands, split by module under `src/commands/`, shared state in `src/state.rs`, the auto-apply loop in `src/auto.rs`, the tray in `src/setup.rs`.
- `apps/desktop/src`: React and TypeScript front end (views for Preview, Rules, History, Settings and the first-run guide).
- `docs/USER_GUIDE.ro.md`, `docs/USER_GUIDE.en.md`: user guides. `docs/SAFETY.md`: what Valet does to files and how that is tested. `docs/images/`: screenshots from the UI smoke test. `docs/PLAN.md`: product and architecture plan. `docs/reports/`: phase reports.
- `CLAUDE.md`: working rules and safety invariants for the project.

## Build on Windows

Windows is the only supported target.

1. In an elevated PowerShell, run `scripts/windows_dev_setup.ps1`. It installs Git, Node.js LTS, Rustup and Visual Studio 2022 Build Tools with winget.
2. Finish the manual steps the script prints:
   - Open the Visual Studio Installer and add the "Desktop development with C++" workload, with the Windows 10/11 SDK selected.
   - Run `rustup default stable-x86_64-pc-windows-msvc`.
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

Clippy (with `-D warnings`), fmt and the tests are the required checks before a commit. CI runs them on `ubuntu-latest` (valet-core and valet-platform) and `windows-latest` (whole workspace, after `pnpm build`), see `.github/workflows/ci.yml`.

Front end:

```
cd apps/desktop
pnpm install --frozen-lockfile
pnpm build
pnpm test
pnpm run check:i18n
```

Tests that exist:

- 52 characterization tests in `crates/valet-core/tests/characterization_*.rs`. They pin the matcher and dry-run behavior. A change that alters that behavior should change these tests on purpose.
- Unit tests next to the code in `valet-core` (planner, matcher, executor with recovery, undo and trash, journal, collision naming, presets) and in `valet-platform`. `cargo test -p valet-core -p valet-platform` runs them on Linux.
- `crates/valet-core/tests/presets.rs`: each of the 8 presets on a synthetic home folder (install, plan, exact operations, execute, undo, compare the tree), and the interactions between presets.
- Property tests for the planner and for collision naming (`prop_planner.rs`, `prop_collision.rs`).
- `fault_injection.rs`: kills a `valet-fault` child process at a random time during a batch (including batches that send files to a fake Recycle Bin), runs recovery, checks that every file exists once, then undoes the batch. 10 iterations by default; `VALET_FAULT_ITERS=100` sets more. It uses the fake platform and runs on Linux.
- `integration_10k.rs`: plan, execute and undo on a generated tree of 1,000 and 10,000 files (about a minute in a debug build). The perf tests (`perf_*.rs`) are `#[ignore]`.
- `crates/valet-platform/tests/windows.rs` and `windows_e2e.rs` run only on Windows (`windows_e2e.rs` needs `--features valet-core`).
- Unit tests in the app crate (`apps/desktop/src-tauri`): `cargo test -p app`. They cover the preview-only execution, rule validation, settings and approvals, presets, and the auto-apply scheduler, gate and locked-file retries.
- Front-end tests (vitest, `pnpm test`) for the views, the rule model and the i18n helpers.
- A UI smoke test (`tests/e2e/smoke.mjs`, started by `scripts/e2e-linux.sh`) drives the built app with `tauri-driver` through onboarding, preview, organize, undo and history, and takes the screenshots in `docs/images/`. CI runs it on Linux; a Windows run is report-only. It was not run in the session that wrote this README.

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

- Met in code and covered by tests that run on Linux with a fake Recycle Bin: journal before each operation, recovery after a crash (unit tests and a process-kill test with a trash variant), no replacing of existing files, undo with a hash check, restore of trashed files, default exclusions below a scan root and for destinations, skipping of placeholders, locked and recently written files, executing only the previewed plan, and executing automatically only rules approved at their current version.
- Covered only by tests that run on Windows (the CI job `windows`, with no passing result for the current code), or not at all: the Windows API calls themselves (including the Recycle Bin restore), power loss, and real OneDrive or Dropbox placeholders.
- Partly met: the scan root is checked when it is saved, not at scan time, and destination checks work on path text only; "being written" is a name and modification-time heuristic; copies and cross-volume moves need a Recycle Bin on the destination volume (and on the source volume for moves); paths that are not valid Unicode are refused.
- Not met: Windows directory-change overflow is not reported by `notify` 6.1 (mitigated by the 10-minute rescan), approval for automatic organizing is per rule and not per folder, and two double-failure cases can leave a file at both source and destination.
- No network code exists in the workspace. The updater, license check and crash reports are not built.

## License

MIT, see [LICENSE](LICENSE).
