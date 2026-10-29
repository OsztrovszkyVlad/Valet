# Changelog

All notable changes to Valet are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and versions follow
[Semantic Versioning](https://semver.org/). Pre-releases (`-beta.N`) go to the
beta update channel first.

## [Unreleased]

First public version. Valet sorts the files in the folders you choose by the
rules you set.

### Added

- Rules with conditions on name, extension, file type, size, age, path, empty
  folders and duplicates, and the actions move, copy, send to the Recycle Bin
  and report. Destination folders can use the date and extension of the file.
- 8 ready-made presets: Downloads by type, Screenshots, Invoices and receipts,
  Large files, Duplicates, Clean desktop, Old installers and Empty folders.
- A 3-step first-run guide: watched folders, presets, first preview.
- Preview: the list of what each rule would do before anything moves. Organize
  runs exactly the previewed operations.
- Undo for every batch from the History screen, including files sent to the
  Recycle Bin.
- Automatic organizing (off by default), only for rules and folders you have
  approved in a preview.
- Tray menu with Show, Pause and Quit; start with Windows; a notification after
  each batch.
- Romanian and English interface (Romanian is the default).
- Windows installer for the current user, no administrator rights needed. It
  installs Microsoft WebView2 if it is missing, offers English or Romanian,
  and its uninstaller keeps your data unless you tick "Delete the application
  data". The installer is not code-signed yet, so Windows SmartScreen shows
  "Windows protected your PC": choose "More info", then "Run anyway".
- Updates in Settings: the installed version, a Stable or Beta channel, "Check
  now", and install after a confirmation. Valet also checks 30 seconds after it
  starts and every 24 hours, and shows a notification when a version is
  found. It never installs without your click, and the installation waits for a
  running batch. Builds made without the updater keys have no updates.
- Settings, Support: a "Detailed log" switch that applies without a restart.
  Logs are in `%LOCALAPPDATA%\ro.valetapp.desktop\logs`, 5 files of at most
  2 MB, INFO level by default. A program error is logged with paths removed.
- Settings, About: version, license and repository.
- A watched folder that does not exist right now (for example an unplugged
  drive) is saved and marked "Unavailable right now". Valet skips it and
  watches it again when it is back, within 10 minutes, and it keeps its
  approvals.
- A 14-day Pro trial with all Pro features. It starts the first time Valet
  runs, needs no card and uses no network. A small indicator at the top shows
  the days left.
- A Free plan for after the trial: 3 own rules enabled at once (the 8 presets
  are not counted), 2 watched folders active, History showing the last 7 days,
  and undo for the newest organize batch only. Rule export and import are not
  included. Nothing is deleted when a limit applies: the rules and folders
  over the limit stay saved, turned off, and come back with Pro.
- A dialog "Choose what stays active on the Free plan", shown when the plan
  falls back to Free with more than 3 own rules or 2 folders. You pick which 3
  rules and which 2 folders stay active; "Decide later" keeps the first 3
  rules and the first 2 folders until you choose.
- Settings, Valet Pro: the plan, what each plan includes, the price (29 EUR,
  VAT included, one-time, 12 months of updates) and a way to import a license
  file, or its text pasted in. A license is activated online and works on up
  to 3 computers. Valet asks the license server again at most every 30 days
  and keeps working as Pro for up to 30 more days without a connection, then
  works as Free until the next successful check. "Remove the license from this
  computer" frees one of the 3 places; the license files are moved to a
  `license-removed` folder, not deleted.
- Export and import of rules (Pro and trial), on the Rules screen.
- A warning before installing an update that your license does not cover: the
  new version will run as Free; the current version stays Pro.
- Settings, Updates: the checkbox "Check for updates automatically", on by
  default. "Check now" works when it is off.
- A package for the Microsoft Store (MSIX) is built in CI; it is not published.
  In that build, updates come from the Store, starting with Windows is set in
  the Windows settings (Settings has a button that opens them), and
  uninstalling the app deletes its data, including History.

Licenses are not active in this version: no license server is configured, so
buying and activating a license do not work yet. The trial and the Free plan
work.

### Changed

- The unused statistics table (`operation_history`) is removed from the
  database. History comes from the journal and is not affected.
- The automatic update check can now be switched off in Settings (it could
  not before).
- Valet opens its database while the main window is created, so it starts
  faster. Recovery of interrupted operations still finishes before the folder
  watcher starts.
- A silent install (`/S`, winget, scripts) no longer creates a desktop
  shortcut. The interactive installer still offers it on its last page, and an
  update keeps the shortcut you already have.
- The license check is the second network use besides updates, and it only
  happens with a Pro license on a build that has a license server. It sends
  the license id, a one-way hash of the computer id and the app version (see
  `docs/PRIVACY.md`).

### Fixed

- Updates installed without a window (the updater runs the installer in
  passive mode) no longer stop at the installer's language dialog.

### Safety

- Valet never deletes a file permanently and never overwrites one: removal
  goes to the Recycle Bin, and a name collision gets a new name.
- Every operation is written to a journal before it runs. After a crash, each
  file ends up either where it was or where it was going.
- System folders, AppData, Program Files, `.git`, `node_modules` and similar
  folders are skipped; hidden files are skipped unless a rule includes them.
  Files being written, locked files and OneDrive/Dropbox files that are not
  downloaded are skipped too.
- No file names, paths or contents leave your computer. The only network use
  is the update check and download and, with a Pro license, the license check
  (see `docs/PRIVACY.md`); there is no telemetry and no crash reporting.
- The plan limits never delete or trim the journal, rules or folders, and
  license files are moved, not deleted (`docs/SAFETY.md`).
- Databases from earlier test builds (Phase 1 and Phase 2) open with the
  current version, with the journal unchanged (`upgrade_journal` tests).

[Unreleased]: https://github.com/OsztrovszkyVlad/Valet/commits/main
