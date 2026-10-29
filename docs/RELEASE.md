# Releasing Valet

How a version of Valet goes from a commit on `main` to an installer on GitHub
Releases, the update channels and winget. Only Vlad creates tags, publishes
releases, holds the signing keys and submits to winget. Everything else runs in
GitHub Actions.

## 0. Before the first release (one time)

### 0.1 Activate the release workflow

The workflow is in `.github/workflows/release.yml`. It has not run yet. Its
place is `.github/workflows/release.yml`, a path the project hooks protect
(only Vlad changes it), so the agents could not put it there. To activate it:

```
git mv .github/workflows/release.yml .github/workflows/release.yml
git commit -m "Add the release workflow"
git push origin main
```

Then run the dry run and check that it is green before pushing any tag:
GitHub, Actions, Release, Run workflow, branch `main`, `dry_run` checked (the
default). Or with the GitHub CLI: `gh workflow run release.yml -f dry_run=true`.
The dry run builds the NSIS installer, uploads it as a workflow artifact
(`valet-<version>-windows-x64`, kept 14 days) and creates no tag and no
release. The run summary shows one "installer" notice per file with its name,
size and SHA-256.

### 0.2 Updater keys

The updater only installs files signed with Vlad's private key. The public key
goes into the app at build time.

1. On your own machine, in `apps/desktop`:
   `pnpm tauri signer generate -w ~/.tauri/valet-updater.key`
   Choose a password. This writes `valet-updater.key` (private) and
   `valet-updater.key.pub` (public).
2. Keep a backup of the private key and its password outside the computer
   (password manager plus an offline copy). If the key is lost, installed
   copies of Valet cannot be updated any more; every user would have to
   download a new installer by hand.
3. GitHub, repository Settings, Secrets and variables, Actions:
   - Secrets: `TAURI_SIGNING_PRIVATE_KEY` = the full content of
     `valet-updater.key`; `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` = the password.
   - Variables: `VALET_UPDATER_PUBKEY` = the full content of
     `valet-updater.key.pub`.
4. Run the dry run again. The "updater" notice must say "updater artifacts on",
   and the "installer" notices list a `.sig` file next to the `.exe`.

Without the variable and the secret, releases still build, but without
updater files: installed copies would not see that release as an update.

`tauri.conf.json` has no updater section. At build time the workflow writes
`apps/desktop/src-tauri/tauri.updater.conf.json` (gitignored) with the public
key and the two endpoints and passes it to `tauri build --config`.

### 0.3 Optional

- A tag ruleset (Settings, Rules, Rulesets, target tags `v*`) that allows only
  you to create `v*` tags.

## 1. What a release does

| Trigger | What runs |
|---|---|
| push of a tag `v*` | `build` job (Windows): checks that the tag matches the version in all files, builds the NSIS installer (and signs the updater files when the keys are set), creates a **draft** release named "Valet vX.Y.Z" with the installer, `.sig` and `latest.json`. A version with `-` (e.g. `1.0.0-beta.1`) is marked pre-release. The body is the CHANGELOG section for the version plus the SmartScreen note. |
| Actions, Run workflow, `dry_run` checked | `build` job without a release: installer as a workflow artifact. |
| Run workflow on a `v*` tag with `dry_run` unchecked | rebuilds the draft of that tag (fails if the release is already published). |
| you publish a release | `updater-channel` job (Linux): copies the release's `latest.json` to `latest-beta.json` on the pre-release `updater-channel`, unless that would lower the beta version. |
| Run workflow with `beta_tag` set | `updater-channel` job only: points `latest-beta.json` at that release, even a lower version (rollback). |

The workflow never creates a `v*` tag and never publishes a release.

### Update channels (PLAN 4.7)

| Channel | Endpoint | Follows |
|---|---|---|
| stable | `https://github.com/OsztrovszkyVlad/Valet/releases/latest/download/latest.json` | the release GitHub marks "Latest"; drafts and pre-releases are never "Latest" |
| beta | `https://github.com/OsztrovszkyVlad/Valet/releases/download/updater-channel/latest-beta.json` | the newest published version, pre-release or stable |

`updater-channel` is a pre-release that the workflow creates on the first
publish, with a tag of the same name. It is never marked "Latest". Do not
delete it; if it is deleted, the next publish (or a run with `beta_tag`)
creates it again.

The updater plugin tries the endpoints in the order given and stops at the
first one that answers, so the build config lists `[stable, beta]` and the app
sets the endpoint of the user's channel at run time.

## 2. Release step by step

Example: the first beta, `1.0.0-beta.1`.

1. Set the version in every file that carries it:
   `node scripts/set-version.mjs 1.0.0-beta.1`
   (changes `tauri.conf.json`, `apps/desktop/package.json`,
   `apps/desktop/src-tauri/Cargo.toml` and `Cargo.lock`), then
   `node scripts/check-version.mjs` must print OK.
2. In `CHANGELOG.md`, turn `## [Unreleased]` into
   `## [1.0.0-beta.1] - YYYY-MM-DD`, add a new empty `## [Unreleased]` above
   it, and add the link line at the bottom:
   `[1.0.0-beta.1]: https://github.com/OsztrovszkyVlad/Valet/releases/tag/v1.0.0-beta.1`.
   The tag build fails if this section is missing.
3. `cargo check -p app` (Cargo.lock stays consistent), commit
   (`Release 1.0.0-beta.1`), `git push origin main`, wait for CI to be green.
4. Tag and push the tag:
   `git tag v1.0.0-beta.1` then `git push origin v1.0.0-beta.1`.
5. Wait for the Release workflow (Actions). The summary shows the installer
   name, size and SHA-256, and the link to the draft.
6. Review the draft (Releases): assets `Valet_1.0.0-beta.1_x64-setup.exe`,
   `.exe.sig` and `latest.json` (when the keys are set); "pre-release" ticked
   for a `-beta` version; the notes. Download the installer and test it on the
   Windows 10 and 11 VMs (install, update from the previous version,
   uninstall).
7. Publish:
   - beta: keep "Set as a pre-release" ticked;
   - stable: untick pre-release and keep "Set as the latest release" ticked.
   Publishing starts the `updater-channel` job. Check its notice
   ("latest-beta.json now ...").
8. Check the endpoints:
   - `curl -sL https://github.com/OsztrovszkyVlad/Valet/releases/download/updater-channel/latest-beta.json`
   - `curl -sL https://github.com/OsztrovszkyVlad/Valet/releases/latest/download/latest.json`
     (stable only; it returns 404 until the first stable release).
9. Stable after beta (PLAN 4.7): after 7 days with no reports of lost files,
   release `1.0.0` with the same steps. A pre-release cannot be "promoted",
   because the version is built into the binary and the updater compares it.
10. winget, after publishing (section 4).

## 3. Code signing and SmartScreen

The installer and the app executable are not code-signed yet (PLAN 3.3: SignPath
Foundation or Certum Open Source, then OV as a fallback). Until then Windows
SmartScreen shows "Windows protected your PC" on download; the user chooses
"More info", then "Run anyway". The release notes and the README say so. The
updater signature (minisign) is separate and works without a certificate.

When a certificate exists, signing is added in `tauri.conf.json` >
`bundle.windows.signCommand` (or `certificateThumbprint`) by Vlad; the hooks
stop agents from editing those keys.

## 4. winget

Identifier `OsztrovszkyVlad.Valet` for stable versions and
`OsztrovszkyVlad.Valet.Beta` for pre-releases (winget-pkgs keeps pre-releases
under their own identifier so `winget upgrade` does not move stable users to a
beta). Manifest schema 1.10.0, installer type `nullsoft`, scope `user`,
locales en-US (default) and ro-RO. Templates: `packaging/winget/templates/`.

1. Take the installer URL from the published release and the SHA-256 from the
   workflow notice (or `Get-FileHash <file> -Algorithm SHA256`).
2. `node scripts/winget-manifest.mjs 1.0.0-beta.1 https://github.com/OsztrovszkyVlad/Valet/releases/download/v1.0.0-beta.1/Valet_1.0.0-beta.1_x64-setup.exe <sha256> --beta`
   writes `packaging/winget/out/manifests/o/OsztrovszkyVlad/Valet/Beta/1.0.0-beta.1/` (gitignored).
3. On Windows: `winget validate --manifest <that folder>`, then
   `winget install --manifest <that folder>` in a VM (needs
   `winget settings --enable LocalManifestFiles` once, as administrator).
4. `wingetcreate submit --token <GitHub token> <that folder>` opens the pull
   request on microsoft/winget-pkgs. A moderator reviews the first submission;
   an unsigned installer can get extra scanning.

## 5. Rollback

### A bad draft (not published)

Delete the draft release on GitHub, delete the tag
(`git push origin --delete v1.2.3` and `git tag -d v1.2.3`), fix, and release
with a new version number. Never reuse a version number that was published.

### A bad stable release, published

1. Stop new updates and downloads: open the previous good release, Edit, tick
   "Set as the latest release", Update. From then on
   `releases/latest/download/latest.json` serves the previous version, so no
   installed copy sees the bad version as an update. GitHub may serve the old
   redirect for a few minutes.
2. If `latest-beta.json` also points at the bad version: Actions, Release, Run
   workflow, `beta_tag` = the previous good tag (for example `v1.2.2`).
3. Optionally tick "Set as a pre-release" on the bad release, or delete it.
   Deleting removes its assets: the `latest.json` of any channel that still
   points at it then fails to download, and the updater reports an error
   instead of installing.
4. Users already on the bad version: the updater never installs a lower
   version. Release the fix as a higher version (`1.2.4`) with the normal
   steps; once it is published and "Latest", those users get it on the next
   update check, provided the updater of the bad version works. If it does
   not, they install the new installer by hand over the old one (same install
   folder, settings and journal kept); say so in the release notes and the
   README.
5. Installing an older installer over a newer version is possible
   (`allowDowngrades` defaults to true) but the older version may not open a
   database that a newer version migrated. Do not advise a downgrade unless
   the release notes of the newer version say it is safe.

### A bad beta, published

Run the workflow with `beta_tag` = the previous good release, optionally mark
or delete the bad pre-release, then release `-beta.N+1` with the fix.

## 6. What the installer does

- Installs for the current user only, no administrator rights, into
  `%LOCALAPPDATA%\Valet`; uninstall entry under `HKCU`.
- Language selector (English, Romanian); Start menu shortcut in the folder
  "Valet"; optional desktop shortcut.
- Downloads and runs the WebView2 bootstrapper if WebView2 is missing (needs
  internet only in that case).
- The uninstaller offers "Delete the application data" (unticked by default).
  Ticking it removes `%APPDATA%\ro.valetapp.desktop`, which holds the
  settings, the rules and the operation journal, so History and undo are gone
  after a reinstall. Files Valet moved stay where they are.
- The Romanian installer texts that Tauri does not translate are in
  `packaging/nsis/Romanian.nsh`.

## 7. Icon

The icons in `apps/desktop/src-tauri/icons/` are a **placeholder**: a white
"V" on a solid blue-grey square, generated with ImageMagick into
`icons/app-icon-source.png`. Replace them when the designed icon exists:
put a 1024x1024 PNG at `apps/desktop/src-tauri/icons/app-icon-source.png`, run
`pnpm tauri icon src-tauri/icons/app-icon-source.png` in `apps/desktop`, move
out the `android/` and `ios/` folders it also creates (not used), and commit.

## 8. Files

| File | Purpose |
|---|---|
| `.github/workflows/release.yml` | the release workflow, until moved to `.github/workflows/` (0.1) |
| `apps/desktop/src-tauri/tauri.conf.json` | version, bundle and NSIS settings, CSP |
| `scripts/set-version.mjs`, `scripts/check-version.mjs` | the version in one step, and the check |
| `scripts/release-notes.mjs` | release body from `CHANGELOG.md` |
| `scripts/write-updater-config.mjs` | build-time updater config (public key, endpoints) |
| `scripts/version-gt.mjs` | version comparison for the beta channel |
| `scripts/winget-manifest.mjs`, `packaging/winget/templates/` | winget manifests |
| `packaging/nsis/Romanian.nsh` | Romanian installer texts |
| `CHANGELOG.md` | user-facing changes per version |
