# Contributing

Valet is a pre-release prototype. Read [README.md](README.md) for the current status and [docs/PLAN.md](docs/PLAN.md) for the plan. Working rules are in [CLAUDE.md](CLAUDE.md).

## Prerequisites

- Rust (stable), Node.js LTS, pnpm (`corepack enable`).
- Windows is the only supported target. On Windows, `scripts/windows_dev_setup.ps1` installs the tools; see the README for the manual steps.
- `valet-core` also builds and tests on Linux. Building the Tauri crate on Linux needs the webkit2gtk-4.1 development packages.

## Checks before a commit

From the repository root:

```
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
```

For front-end changes:

```
cd apps/desktop
pnpm build
```

There are no front-end tests yet. Do not commit with a failing check.

## Language

- Code, identifiers, comments and commit messages: English.
- UI text: only through i18n keys, never hardcoded in components. Romanian is the default language and English is the second. The i18n files (`src/i18n/ro.json`, `src/i18n/en.json`) do not exist yet; they are planned for Phase 2.
- Describe what the software does. No marketing wording in the README, UI, comments or reports.

## Safety rules

The safety invariants in [CLAUDE.md](CLAUDE.md) apply to every change. The main ones:

- Never delete permanently. Removal goes to the Recycle Bin through the `trash` crate. `std::fs::remove_file`, `remove_dir` and `remove_dir_all` are not allowed outside tests.
- Never overwrite a file. Name collisions get a deterministic new name.
- Journal every file operation before it runs.
- Do not touch default-excluded folders or files unless the user opts in.
- No network calls except the updater, the license check and opt-in crash reports.

The current code does not meet all of these yet; the README lists the gaps.

## Dependencies

Add only the crates and npm packages in the allowlist in [CLAUDE.md](CLAUDE.md). Ask the owner before adding anything else.

## License

By contributing you agree that your work is released under the MIT license (see [LICENSE](LICENSE)).
