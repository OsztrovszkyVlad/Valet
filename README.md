# Valet

Valet is a rules-based file organizer for Windows, in the spirit of Hazel on macOS. You define rules (conditions on a file, and actions to take), Valet lists what each rule would do to the files in your watched folders, and then performs the actions.

It is a pre-release prototype. There is no installer and no release. The code is a Rust workspace plus a Tauri 2 desktop app (React 19, TypeScript, Vite). The product plan is in [docs/PLAN.md](docs/PLAN.md) (in Romanian).

<!-- screenshot: added in Phase 2 -->

## Status

What works today:

- Rules are stored in SQLite.
- The `dry_run` command walks the configured inbox folders and lists the planned actions for each matching rule. It changes nothing.
- The `execute_operations` command performs `moveTo` and `copyTo` with `std::fs::rename` and `std::fs::copy`.
- Implemented conditions: `ext`, `nameMatches` (regex on the file name), `sizeGt`, `sizeLt`, `pathContains`.
- Actions the planner knows about: `moveTo`, `copyTo`, `tag`, `rename`, `quarantine`. Only `moveTo` and `copyTo` execute. The others fail with "Unsupported operation type".

Known gaps. All of these are planned in Phase 1 or Phase 2 of [docs/PLAN.md](docs/PLAN.md); none is fixed yet:

- No operation journal and no undo.
- The `moveTo` path is used as written, as the full destination file path. There is no `~` expansion, and the file name is not appended to it.
- No default exclusions. Hidden files, `.git` and `node_modules` are scanned.
- No Recycle Bin integration.
- The file watcher only logs events. It does not organize anything.
- UI text is English only and hardcoded. Romanian text (the default language) is planned for Phase 2.
- The form in the Rules view does not match the backend rule schema, so creating a rule from the UI fails.
- The backend has diagnostics, backup and template commands that the UI does not use. Whether to keep them is undecided.

Do not point this build at folders whose contents you cannot lose.

## Repository layout

- `crates/valet-core`: rules, matcher, dry-run engine and SQLite storage. No OS-specific code; builds and tests on Linux.
- `crates/valet-platform`: the file watcher (`notify` with debouncing). Windows-specific features (Recycle Bin, placeholders, long paths, autostart) are planned and not written yet.
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

Clippy and fmt are the required checks before a commit. At the time of writing they report errors that are being cleaned up.

Front end:

```
cd apps/desktop
pnpm install --frozen-lockfile
pnpm build
```

There is no `pnpm test` script yet and no front-end tests.

Tests that exist:

- 52 characterization tests in `crates/valet-core/tests/characterization_*.rs`. They pin the current matcher and dry-run behavior, including the gaps listed above. A change that alters that behavior should change these tests on purpose.
- 4 regression tests in the app crate (`apps/desktop/src-tauri`).
- 1 unit test in `valet-core`.

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

As noted under Status, with the current code the `moveTo` path above would be used literally (`~` is not expanded) and `tag` would fail to execute.

## Safety

The target behavior is listed in `CLAUDE.md` under "Safety invariants" and in [docs/PLAN.md](docs/PLAN.md) section 2.6: never delete permanently, never overwrite, journal every operation before it runs, undo while destination files are unchanged, default exclusions, skip files being written and unhydrated cloud placeholders, no network use for file data.

These are the goal, not a description of the current code. The current code does not meet these yet:

- Journal and undo (no journal exists).
- Default exclusions (none exist).
- Skipping files being written, locked files and cloud placeholders (not implemented).
- Recycle Bin for removals (not implemented; no current action removes files).
- Collision naming. Never overwriting is only partly met: `moveTo` and `copyTo` check whether the destination exists and fail with "Destination file already exists" instead of choosing a new name. The check and the move are separate steps, so it is not atomic.

## License

MIT, see [LICENSE](LICENSE).
