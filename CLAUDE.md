# Valet

Rules-based file organizer for Windows ("Hazel for Windows"). Rust core + Tauri 2 desktop app.
The product plan is `docs/PLAN.md`. Read it before any phase work. It wins over this file on product questions; this file wins on working rules.

## Who does what

- Vlad: owner. Approves phases, tests manually, decides prices, keys, accounts, dependencies.
- Main session: project manager (PM). Reads the plan, dispatches subagents, verifies, writes `docs/reports/phase-N.md`, stops.
- Subagents (`.claude/agents/`): researcher (read-only), rust-core, tauri-platform, frontend, qa, release, docs-writer. One area each.

## Architecture map

- `crates/valet-core` — OS-agnostic: rules, matcher, planner, journal, executor policy, SQLite storage. Must build and test on Linux. No Windows crates here.
- `crates/valet-platform` — Windows only: watcher, Recycle Bin (`trash`), cloud placeholder detection, long paths, shell integration, autostart, notifications.
- `apps/desktop/src-tauri` — thin Tauri commands by module under `src/commands/`, shared state in `state.rs`. No business logic here.
- `apps/desktop/src` — React + TypeScript + Vite. UI text only via i18n keys (`src/i18n/ro.json`, `src/i18n/en.json`). Romanian is the default language.
- `crates/valet-core/presets/` — the shipped presets as JSON, each with an integration test.

## Safety invariants (never break, never "temporarily" bypass)

1. Never delete permanently. Removal means Recycle Bin via the `trash` crate. `std::fs::remove_file`, `remove_dir`, `remove_dir_all` are forbidden outside tests (a hook blocks them).
2. Never overwrite. Name collisions get a deterministic new name.
3. Every file operation is journaled before it runs and marked after. Crash recovery on startup must leave each file exactly once, at source or at destination.
4. Everything is undoable while destination files are unchanged.
5. Default exclusions (system dirs, AppData, Program Files, `.git`, `node_modules`, `target`, hidden/system files) are never touched unless the user opts in explicitly.
6. Skip files being written, locked files, and unhydrated OneDrive/Dropbox placeholders. Never hash a placeholder.
7. No network calls except updater, license check, and opt-in crash reports. No file names, paths, or contents ever leave the machine.

## Working rules

- Investigate before changing: read the code paths involved, run the existing tests, then edit.
- One targeted change at a time. After each change: `cargo check -p <crate>`; `cargo test -p <crate>` before moving on; `pnpm build` in `apps/desktop` for frontend changes.
- No over-engineering: no abstractions the current phase does not need, no new crates or npm packages outside the allowlist below without asking Vlad.
- Commit directly to `main`, no branches or PRs, short imperative commit messages. Worktrees only when the PM runs two subagents on disjoint areas; the PM merges locally.
- Never run `git push --force`, never rewrite history, never touch `.env*`, signing keys, `tauri.conf.json` updater/signing sections, or CI secrets without Vlad saying so in the current prompt.
- UI text in Romanian (and English in `en.json`). Code, identifiers, comments, commit messages in English.
- No marketing or AI-cliché vocabulary anywhere (README, UI, comments, reports): not "seamless", "powerful", "enterprise-grade", "leverage", "revolutionary", "robust", "cutting-edge", "delve". Say what it does.
- Windows is the only target now. Keep `valet-core` free of OS code so macOS stays possible.

## Commands

- Setup: `scripts/windows_dev_setup.ps1`
- Core: `cargo build`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check`
- Desktop: `cd apps/desktop && pnpm install && pnpm tauri dev`, `pnpm build`, `pnpm test`
- Release (Phase 3+): `pnpm tauri build`, tags `vX.Y.Z` trigger `tauri-action`

## Dependency allowlist (no question needed)

Rust: `trash`, `notify`, `notify-debouncer-mini`, `walkdir`, `blake3`, `sqlx` or `rusqlite`, `serde`, `serde_json`, `thiserror`, `anyhow`, `regex`, `uuid`, `proptest`, `tempfile`, `criterion`, `tracing`, `tracing-subscriber`, `windows-sys` (platform crate only), `minisign-verify` (license check), `reqwest` (desktop app only, license check; approved by Vlad, 2026-10-29), `rustls` with the `ring` feature (desktop app only, crypto provider for `reqwest`; approved by Vlad), `tauri-plugin-{log,single-instance,autostart,notification,updater,dialog,fs,shell}`.
Frontend: `react`, `react-i18next`, `i18next`, `zustand`, `vitest`, `@testing-library/react`, and as devDependencies only `@testing-library/dom`, `jsdom` (approved by Vlad, 2026-10-29).

## Phase protocol

1. `/phase N` starts phase N from `docs/PLAN.md`.
2. PM writes a task list, dispatches subagents with mechanically checkable acceptance criteria, waits for each, verifies with the real commands, fixes or re-dispatches.
3. PM runs the full test suite, then writes `docs/reports/phase-N.md` using the `phase` skill's report template, commits, and stops. Nothing from phase N+1 starts without "Faza N aprobată" from Vlad.
4. If blocked by something only Vlad can do (keys, accounts, prices, a dependency outside the allowlist, an ambiguous product decision), stop and list the questions in the report instead of guessing.

## Report rules

Reports state what was built, what was tested (commands and summarized output), measured numbers against the budgets in `docs/PLAN.md` section 2.6, known issues, open decisions, and the manual checklist for Vlad. No adjectives.
