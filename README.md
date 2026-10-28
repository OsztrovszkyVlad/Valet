# Valet

Valet is a rules-based file organizer for Windows, in the spirit of Hazel on macOS. You define rules (conditions on a file, and actions to take), Valet lists what each rule would do to the files in your watched folders, and then performs the actions.

It is a pre-release prototype. There is no installer and no release. The code is a Rust workspace plus a Tauri 2 desktop app (React 19, TypeScript, Vite). The product plan is in [docs/PLAN.md](docs/PLAN.md) (in Romanian).

<!-- screenshot: added in Phase 2 -->

## Status

What works today:

- Rules are stored in SQLite.
- The `dry_run` command walks the configured inbox folders (`planner::plan`) and lists the planned moves and copies, plus the files it skipped and why (excluded folder, hidden, being written, cloud placeholder, and so on). It changes nothing.
- The `execute_operations` command runs planned operations through the operation journal (`executor::execute`). Each operation is journaled, checked again right before it runs, and moved without replacing anything. Removals go to the Recycle Bin.
- The `undo_batch`, `list_batches` and `get_batch` commands undo and inspect journaled batches. On startup the app recovers operations that a crash left unfinished.
- Implemented conditions: `ext`, `nameMatches` (regex on the file name), `sizeGt`, `sizeLt`, `pathContains`.
- Actions that execute: `moveTo` and `copyTo`. The `path` parameter is the destination folder; the file name is appended and a taken name becomes `name (2).ext`. The folder may use `~`, `%VAR%`, `{year}`, `{month}` and `{ext}`; a relative folder is resolved against the scan root. `tag`, `rename`, `quarantine` and any other action type are not run; the preview reports them as `unsupportedAction` when no move or copy was planned for the file.

Known gaps. The details, with the tests that cover each case, are in [docs/SAFETY.md](docs/SAFETY.md):

- The file watcher only logs a count of changed files. It does not organize anything, and on Windows `notify` 6.1 does not report a buffer overflow, so it cannot ask for a rescan.
- UI text: Romanian is the default language and the i18n files are in `apps/desktop/src/i18n/`, but the Rules, Settings, Preferences and Statistics views still have hardcoded English text.
- The form in the Rules view does not match the backend rule schema, so creating a rule from the UI fails.
- The backend has diagnostics, backup and template commands that the UI does not use. Whether to keep them is undecided.
- No check that stops a scan root such as `C:\`, and the default exclusions are not applied to destination folders.
- Two cases can leave a file at both the source and the destination after a double failure (never zero copies); see SAFETY.md, invariant 3.
- Debug-level log lines contain full paths, and `main.rs` sets the log level to Debug.

The Windows-specific code (Recycle Bin, placeholders, locks, `MoveFileExW`) is tested by `crates/valet-platform/tests/windows.rs`, which runs only on Windows. Do not point this build at folders whose contents you cannot lose.

## Repository layout

- `crates/valet-core`: rules, matcher, planner, collision naming, executor (with startup recovery and undo), operation journal and SQLite storage. No OS-specific code; builds and tests on Linux.
- `crates/valet-platform`: the Windows layer: Recycle Bin (`trash` crate), cloud placeholder, hidden, reparse-point and lock checks, volume check, no-replace rename, `\\?\` long paths, and the file watcher (`notify` with debouncing). Off Windows the functions return neutral answers so the workspace builds on Linux. Autostart is in the Tauri app, not in this crate.
- `apps/desktop/src-tauri`: Tauri commands, split by module under `src/commands/`, and shared state in `src/state.rs`.
- `apps/desktop/src`: React and TypeScript front end.
- `docs/PLAN.md`: product and architecture plan.
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
```

There is no `pnpm test` script yet and no front-end tests.

Tests that exist:

- 52 characterization tests in `crates/valet-core/tests/characterization_*.rs`. They pin the matcher and dry-run behavior. A change that alters that behavior should change these tests on purpose.
- Unit tests next to the code in `valet-core` (planner, executor with recovery and undo, journal, collision naming) and in `valet-platform`. `cargo test -p valet-core -p valet-platform` runs them on Linux.
- `crates/valet-platform/tests/windows.rs` runs only on Windows.
- Unit tests in the app crate (`apps/desktop/src-tauri`).

On Linux, building the Tauri crate needs the webkit2gtk-4.1 development packages. This is only for building and testing; the app targets Windows.

## Rule format

Rules are JSON. This is the sample in `crates/valet-core/sample_rules.json`:

```json
[
  {
    "id": "00000000-0000-4000-8000-000000000001",
    "name": "Invoices → Finance",
    "enabled": true,
    "alwaysApply": false,
    "conditions": [
      { "type": "ext", "value": "pdf" },
      { "type": "nameMatches", "value": "(?i)invoice|factura|bill" }
    ],
    "actions": [
      { "type": "moveTo", "params": { "path": "~/Documents/Finance" } },
      { "type": "tag", "params": { "tags": ["finance","invoice"] } }
    ],
    "version": 1,
    "options": { "askConfirmation": false }
  }
]
```

Fields (defined in `crates/valet-core/src/rules.rs`):

- `id`: UUID.
- `name`: display name.
- `enabled`: a disabled rule never matches.
- `alwaysApply`: optional, default `false`. When `true`, the rule matches every file and its conditions are not checked.
- `conditions`: list of `{ "type", "value" }`. All conditions must hold (they are ANDed). A condition type the matcher does not implement never matches, so the rule never matches.
- `actions`: list of `{ "type", "params" }`.
- `version`: optional, default `1`.
- `options`: free-form JSON object, optional.

Condition notes: `ext` compares the extension without the dot, ignoring case. `nameMatches` is a regex on the file name only and is case-sensitive unless it starts with `(?i)`. `sizeGt` and `sizeLt` take a size in bytes. `pathContains` is a case-insensitive substring match on the full path.

With the current code the `moveTo` path above is a destination folder: `~` expands to the home folder and the file name is appended. The `tag` action is not run; the preview reports it as `unsupportedAction` only when no move or copy was planned for the file.

## Safety

The invariants are listed in `CLAUDE.md` under "Safety invariants" and in [docs/PLAN.md](docs/PLAN.md) section 2.6: never delete permanently, never overwrite, journal every operation before it runs, undo while destination files are unchanged, default exclusions, skip files being written and unhydrated cloud placeholders, no network use for file data.

[docs/SAFETY.md](docs/SAFETY.md) says, for each invariant, how the code meets it and which tests cover it, what happens in each edge case, how the journal works, and what is not covered yet. Where the code does less than the plan, it says so. Short version:

- Met in code and covered by tests that run on Linux with a fake Recycle Bin: journal before each operation, recovery after a crash, no replacing of existing files, undo with a hash check, default exclusions below a scan root, skipping of placeholders, locked and recently written files.
- Covered only by tests that run on Windows (the CI job `windows`; its results were not checked), or not at all: the Windows API calls themselves, and real OneDrive or Dropbox placeholders.
- Partly met: exclusions are not applied to destinations or to the scan root itself; "being written" is a name and modification-time heuristic; copies and cross-volume moves need a Recycle Bin on the destination volume (and on the source volume for moves); paths that are not valid Unicode are refused.
- Not met: the watcher does not apply rules, Windows directory-change overflow is not reported by `notify` 6.1, and two double-failure cases can leave a file at both source and destination.
- No network code exists in the workspace. The updater, license check and crash reports are not built.

## License

MIT, see [LICENSE](LICENSE).
