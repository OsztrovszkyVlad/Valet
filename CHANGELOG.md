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
  installs Microsoft WebView2 if it is missing.

### Safety

- Valet never deletes a file permanently and never overwrites one: removal
  goes to the Recycle Bin, and a name collision gets a new name.
- Every operation is written to a journal before it runs. After a crash, each
  file ends up either where it was or where it was going.
- System folders, AppData, Program Files, `.git`, `node_modules` and similar
  folders are skipped; hidden files are skipped unless a rule includes them.
  Files being written, locked files and OneDrive/Dropbox files that are not
  downloaded are skipped too.
- No file names, paths or contents leave your computer.

[Unreleased]: https://github.com/OsztrovszkyVlad/Valet/commits/main
