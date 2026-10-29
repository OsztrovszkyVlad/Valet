# Contributing

Valet is a pre-release prototype. Read [README.md](README.md) for the current status and [docs/PLAN.md](docs/PLAN.md) for the plan. Working rules are in [CLAUDE.md](CLAUDE.md). How a version is released is in [docs/RELEASE.md](docs/RELEASE.md); only the owner creates tags and releases.

## Prerequisites

- Rust, Node.js LTS, pnpm (`corepack enable`). `rust-toolchain.toml` pins Rust 1.99.1 with clippy and rustfmt; inside the repository rustup uses that version, so local runs and CI use the same compiler. A change to the pin is a change to that file.
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
pnpm lint
pnpm test
pnpm check:i18n
pnpm build
```

- `pnpm lint` runs ESLint on `src` (`.ts` and `.tsx`).
- `pnpm test` runs the vitest tests (15 files, 181 tests when this was written): views, components, the rule model and the i18n helpers. A new view or component gets a test next to it (`Name.test.tsx`).
- `pnpm check:i18n` fails when `src/i18n/ro.json` and `src/i18n/en.json` differ in keys, when a value is not a string, or when a literal `t('key')` in `src/` has no entry in `ro.json`. CI also runs a type check (`npx tsc -p tsconfig.app.json --noEmit`).

The UI smoke test drives the built app through onboarding, preview, organize, undo, history, Rules and Settings with `tauri-driver`. Run it after changes to a screen the test visits, or to a text it reads (it finds elements by the Romanian labels in `ro.json`). On Linux it needs `tauri-driver` (`cargo install tauri-driver --locked --version 2.0.6`), `WebKitWebDriver` (apt package `webkit2gtk-driver`), Xvfb and Node 22:

```
cd apps/desktop && pnpm build && cd ../..
cargo build -p app --features tauri/custom-protocol
scripts/e2e-linux.sh target/debug/app /tmp/valet-shots
```

The script starts Xvfb when `DISPLAY` is unset and runs the app with a temporary `HOME`, so it does not touch your own Valet data. The second argument is a folder for screenshots; the committed ones in `docs/images/` are written by the same test, so pass another folder unless you mean to replace them. On Windows the test cannot redirect the config folder and known folders, so it refuses to run outside CI unless `--allow-real-user-dirs` is given (see the header of `tests/e2e/smoke.mjs`).

Do not commit with a failing check. For a change a user can see, add a line to the "Unreleased" section of [CHANGELOG.md](CHANGELOG.md); the release notes are built from it.

## Language

- Code, identifiers, comments and commit messages: English.
- UI text: only through i18n keys, never hardcoded in components. Romanian is the default language and English is the second. The files are `apps/desktop/src/i18n/ro.json` and `en.json`; every key goes in both, and `pnpm check:i18n` checks that.
- Describe what the software does. No marketing wording in the README, UI, comments or reports.

## Safety rules

The safety invariants in [CLAUDE.md](CLAUDE.md) apply to every change. The main ones:

- Never delete permanently. Removal goes to the Recycle Bin through the `trash` crate. `std::fs::remove_file`, `remove_dir` and `remove_dir_all` are not allowed outside tests.
- Never overwrite a file. Name collisions get a deterministic new name.
- Journal every file operation before it runs.
- Do not touch default-excluded folders or files unless the user opts in.
- No network calls except the updater, the license check and opt-in crash reports. The updater is the only one that exists; a change that adds another request, or changes what a request contains, must also change [docs/PRIVACY.md](docs/PRIVACY.md) and its Romanian version, and the invariant 7 section of `docs/SAFETY.md`.
- No file path in an INFO log line. Paths belong at DEBUG level only (`log::debug!`); the DEBUG level is a user setting and its lines stay on the computer.

The current code does not meet all of these yet; the README and [docs/SAFETY.md](docs/SAFETY.md) list the gaps. When a change alters what Valet does to files, update `docs/SAFETY.md` with the tests that cover it.

## Dependencies

Add only the crates and npm packages in the allowlist in [CLAUDE.md](CLAUDE.md). Ask the owner before adding anything else.

## License

By contributing you agree that your work is released under the MIT license (see [LICENSE](LICENSE)).
