# Safety

This document says what Valet does to your files, why it cannot lose or overwrite them in the cases we have tested, and where it can. It is written from the code, not from the plan. Where the code does less than `docs/PLAN.md` section 2.6 says, it says so.

It is also the source for the future FAQ, so each section can be read on its own.

Written against the Rust code at commit `4098837` (the crates are unchanged since `736f802`). Valet is a pre-release prototype. Nobody should point it at folders they cannot afford to lose.

## How to read this document

- "Test" names are Rust test functions. `executor::tests::x` means function `x` in `crates/valet-core/src/executor/tests.rs`; `planner::tests::x` is `crates/valet-core/src/planner/tests.rs`; `journal::tests::x` is in `crates/valet-core/src/journal.rs`. Platform tests are named by file.
- Most tests use `FakePlatform` (`crates/valet-core/src/platform.rs`, module `testing`): a Recycle Bin that is a folder, and sets of paths that the test declares "locked", "placeholder" or "hidden". They run on Linux. They prove the logic of the planner, executor, recovery and undo. They do not prove that Windows behaves as the code assumes.
- The Windows code (`GetFileAttributesW`, `MoveFileExW`, the registry checks in `can_trash`, the lock probe, the Recycle Bin through the `trash` crate) is compiled only on Windows (`mod win` in `crates/valet-platform/src/fs_win.rs`). Its tests are in `crates/valet-platform/tests/windows.rs`, which starts with `#![cfg(windows)]`. In the Linux session that produced this document that file ran 0 tests. The CI file `.github/workflows/ci.yml` runs `cargo test --workspace` on `windows-latest`; the results of that job were not checked when this was written.
- A statement with no test named is read from the code only. It is marked "no test".

Commands run to check this document, from the repository root:

```
cargo test -p valet-core -p valet-platform
```

All tests in those two crates passed on Linux. (The run used `--locked` and a separate `CARGO_TARGET_DIR`; nothing else was different.)

## 1. The invariants

The seven invariants are in `CLAUDE.md`. Invariant 8 in `docs/PLAN.md` (CPU, memory and speed budgets) is not covered here.

Summary:

| # | Invariant | Status |
|---|---|---|
| 1 | Never delete permanently | Met in code. Windows Recycle Bin edge cases not tested. |
| 2 | Never overwrite | Met on Windows by the API used; on Linux the check is not atomic (not a product target). |
| 3 | Journal first; each file exactly once after a crash | Met for the crash states we tested. Two cases leave a file twice (never zero times). |
| 4 | Undo while destinations are unchanged | Met, with listed limits. |
| 5 | Default exclusions | Partly met. Apply only below the folders the user chose; destinations are not checked. |
| 6 | Skip files being written, locked files, placeholders | Partly met. Heuristics, and a lock check that can be out of date. |
| 7 | No network except updater, license, opt-in crash reports | Met by absence: none of those exist and no code opens a connection. |

### Invariant 1. Never delete permanently

**How the code meets it.**

- There is no call to `remove_file`, `remove_dir` or `remove_dir_all` anywhere under `crates/` or `apps/desktop/src-tauri/src` (checked with grep, tests included). The hook `.claude/hooks/guard-edit.sh` blocks adding them to non-test Rust files.
- Valet takes a file out of its place in only these situations: the source of a verified cross-volume move; the temporary copy `.valet-tmp-<id>` after a failed or mismatched copy; a verified copy that has to be taken back because the source could not be trashed; the copy created by a "copy" action when it is undone; leftovers handled by startup recovery. Every one of them goes through `Platform::trash`.
- On Windows `Platform::trash` is `valet_platform::trash_path` (`crates/valet-platform/src/recycle.rs`), which calls the `trash` crate (version pinned to `=5.2.9`) on a short-lived thread. It first calls `can_trash` and, if that is false, returns an `Unsupported` error and leaves the file where it is.
- `can_trash` on Windows (`crates/valet-platform/src/fs_win.rs`) is true only if the drive type is "fixed", the policy `NoRecycleFiles` is not set (current user and local machine), and the per-volume `NukeOnDelete` setting is not set. Any lookup failure gives false. Network shares and removable drives are therefore refused.
- The only files Valet overwrites are its own: `config.json` and `app_settings.json` (`apps/desktop/src-tauri/src/commands/settings.rs`), written with `std::fs::write`.

**Tests.**

- `executor::tests::cross_volume_move_copies_verifies_and_trashes_source`: the source ends up in the (fake) Recycle Bin, not gone.
- `executor::tests::failed_source_trash_removes_the_copy_again`, `executor::tests::copy_with_hash_mismatch_trashes_temp_and_fails`: the copy is trashed, not deleted.
- `executor::tests::cross_volume_without_recycle_bin_touches_nothing`: no Recycle Bin, nothing is done.
- `executor::tests::undo_restores_byte_identical_tree`: undoing a copy sends the copy to the bin.
- `recycle::tests::trash_file_leaves_dir` and `trash_missing_is_not_found` (`crates/valet-platform/src/recycle.rs`). On Linux they exercise the freedesktop trash, not the Windows one.
- Windows only: `temp_volume_supports_trash` and `trash_long_path_never_loses_the_file_silently` in `crates/valet-platform/tests/windows.rs`. The second accepts either outcome (trashed, or an error with the file still there).

**Not covered.**

- No test sets `NoRecycleFiles` or `NukeOnDelete`, or uses a removable drive or a network share. Only the positive case (a temp folder on a fixed drive) is tested.
- The `trash` crate on Windows runs a shell file operation with `FOF_NO_UI | FOF_ALLOWUNDO | FOF_WANTNUKEWARNING` and returns an error if the shell reports an aborted operation. What Windows does when a file is larger than the Recycle Bin's size limit was not verified, and no test covers it.
- Recovery treats a `trash` operation that was in progress as done when the file is no longer at its path. It does not look in the Recycle Bin (`executor/recover.rs`, test `recover_trash_op_by_disk_state`).

### Invariant 2. Never overwrite

**How the code meets it.**

- At preview time the planner picks a free name with `next_free_name` (`crates/valet-core/src/collision.rs`). It checks the disk and also the destinations it has already given to earlier files in the same plan, ignoring case. The names are `name (2).ext`, `name (3).ext`, filling gaps first. Only the last extension is kept apart (`name.tar.gz` becomes `name.tar (2).gz`). The result depends only on the destination and on what is taken.
- At execution time every move and every copy ends with `Platform::rename_no_replace`. On Windows that is `MoveFileExW(from, to, 0)`: no replace flag, no copy flag, so it fails with `AlreadyExists` when the target exists. The check and the rename are one operation in the file system. If the name was taken meanwhile, the executor picks the next free name, writes the new name into the journal first, and tries again (up to 100 times, then it fails with `Io`).
- A cross-volume copy is written to `.valet-tmp-<operation id>` in the destination folder, opened with `create_new` (fails if the file exists), and only then renamed into place with `rename_no_replace`.
- Undo never replaces either: see the undo table in section 2.

**Tests.**

- `collision::tests::*` (7 tests: first free name, gaps, double extension, no extension, dot files, determinism).
- `planner::tests::existing_destination_file_gets_next_free_name`, `two_planned_ops_never_share_a_destination`, `reservation_is_case_insensitive`, `copy_reserves_destination_too`.
- `executor::tests::collision_at_execution_time_gets_next_free_name` (same-volume move, cross-volume move, copy).
- `fs_win::tests::rename_no_replace_refuses_existing_destination`, `rename_no_replace_refuses_existing_directory` (run on every OS), `system::tests::delegates_rename_and_trash`, and `state::tests::platform_never_replaces_existing_files` in the app crate (the app crate was not built in the session that wrote this document).
- Windows only: `long_path_rename_no_replace`.

**Not met / partly met.**

- Off Windows, `rename_no_replace` checks that the target is absent and then calls `std::fs::rename`. Another process can create the target in between and the rename then replaces it. The code comments say this is accepted on Linux because Linux is not a product target. The tests that run on Linux therefore show that the refusal works, not that it is race-free. No test creates a file between the check and the rename.
- The limit of 100 collision retries has no test.
- If a user file already has the name `.valet-tmp-<id>` in the destination folder, the copy fails with `Io` and nothing is overwritten (code only, no test).

### Invariant 3. Journal before, mark after; each file exactly once after a crash

**How the code meets it.**

- Each operation gets a `planned` row in the journal immediately before it is looked at, then `in_progress` (with the source hash) before any file is touched, then `done` or `failed` after. See section 3.
- On startup the app calls `executor::recover` before it starts the watcher (`apps/desktop/src-tauri/src/setup.rs`). It decides every unfinished operation from what is on disk, without asking the user. The decisions are in the crash table in section 2.
- The order of steps in a cross-volume move is: copy to a temp file, fsync, compare blake3 with the hash taken before, rename into place, trash the source. The source is trashed last, so at every moment before that the original is still in place.

**Tests.** `executor::tests::recover_*` (listed in section 2) and in particular `recover_leaves_every_moved_file_exactly_once`, which builds five interrupted moves (planned, not started, mid-copy, placed but source not trashed, finished) and checks that each file's content exists once, no temp file is left, and no operation stays unfinished.

**Limits of the evidence.**

- The recovery tests construct the journal and the disk state that a crash would leave. They do not kill a process. `crates/valet-core/src/bin/valet-fault.rs` is a driver for that (built only with the `test-support` feature; `VALET_FAULT_SLOW_MS` makes it sleep after each journal change so that it can be killed). A kill-and-recover test built on it was not part of the committed code when this was written.
- Power loss was not tested. The code does not set SQLite's `synchronous` setting; sqlx leaves SQLite's default (FULL), so commits are flushed in WAL mode. The `.valet-tmp` file is fsynced (`sync_all`) before the compare; a plain same-volume rename is not followed by a directory flush.

**Two cases where a file can exist twice.** In both, the file is never lost; it is present at the source and at the destination, the operation is marked `failed`, and the error text names both paths.

1. A cross-volume move (or a copy that has to be taken back) where the source cannot be trashed, and then the copy cannot be trashed either. The first failure is handled (the copy goes to the Recycle Bin and the source stays: `executor::tests::failed_source_trash_removes_the_copy_again`). The second failure has no test. The same double failure is possible in undo of a cross-volume move and in recovery.
2. Recovery of a move that was in progress, finding that the destination is a cloud placeholder while the source file still exists. Recovery does not hash a placeholder, so it cannot verify the destination; it leaves both in place and marks the operation `failed`. No test (the test `recover_placeholder_destination_with_source_gone_is_done_unhashed` covers the case where the source is gone).

**Other gaps.**

- Recovery runs only at startup. If it fails (for example the journal cannot be opened for writing), `setup.rs` logs an error and starts anyway.
- There is no single-instance guard (no `tauri-plugin-single-instance` in `apps/desktop/src-tauri/Cargo.toml`). The lock that keeps batches from overlapping (`file_ops` in `state.rs`) is inside one process. A second Valet process would run recovery at its startup and could decide operations that the first process is still executing. PLAN assigns single-instance to Phase 2.
- There is no panic hook that marks `in_progress` operations (PLAN 4.5). A panic is treated like any crash: recovery decides at the next start.
- A test hook is compiled into the shipped executor: if the environment variable `VALET_FAULT_SLOW_MS` is a number, the executor sleeps that many milliseconds after each journal change (`executor.rs`, `fault_pause`). It does nothing when the variable is unset.
- At commit `4098837` the app's `execute_operations` command plans again and runs the new plan immediately; it does not run the list the user saw in an earlier `dry_run`. The "changed since preview" check then compares against a plan made moments earlier in the same call. A change that makes the command run the stored preview was being written when this document was written and is not described here; re-read `commands/operations.rs` before relying on this paragraph.

### Invariant 4. Undo while destinations are unchanged

**How the code meets it.** `executor::undo_batch` (`crates/valet-core/src/executor/undo.rs`) undoes the `done` operations of one finished `apply` batch, last first, as a new `undo` batch. Before touching a file it hashes the file at the destination and compares with the hash recorded when it was moved. If they differ, or if the original path is now taken, nothing is moved and the operation is returned as "needs confirmation". Calling again with `force` does it anyway, still without overwriting. The details are in the undo table in section 2.

**Tests.** `executor::tests::undo_restores_byte_identical_tree` (same-volume and cross-volume; compares the files of the tree by content before and after; a second undo finds nothing to do), `undo_with_modified_destination_needs_confirmation`, `undo_with_occupied_source_needs_confirmation`, `undo_skips_failed_ops_and_lists_them`, `undo_copy_with_original_gone_needs_confirmation`, `undo_cross_volume_without_recycle_bin_fails_untouched`, `undo_rejects_unknown_open_and_non_apply_batches`, `recover_interrupted_undo_rolls_back_original`, `journal::tests::undo_done_and_rollback_are_atomic`.

**Limits.**

- The comparison is a content hash, not a "modified by the user" flag. A file edited and then edited back to the same bytes counts as unchanged.
- Only files are compared in the test; folders that a batch created are left behind (see the undo table).
- Undo cannot restore a file that was moved or deleted after the batch (`SourceMissing`, reported as a failure).
- Operations that are not `done` (failed, already rolled back) are listed as skipped and not touched.
- The Tauri commands `undo_batch` and `execute_operations` (`apps/desktop/src-tauri/src/commands/operations.rs`) take the same lock, so a batch and an undo do not run at the same time in one process.

### Invariant 5. Default exclusions

**How the code meets it.** The planner walks each scan root with `walkdir` (`follow_links(false)`) and skips, without descending: directories whose name (ASCII case-insensitive) is one of `Windows`, `Program Files`, `Program Files (x86)`, `ProgramData`, `AppData`, `$Recycle.Bin`, `System Volume Information`, `.git`, `node_modules`, `target`, `.venv`, `venv`, `__pycache__` (`DEFAULT_EXCLUDED_DIRS` in `planner.rs`); any directory that contains a file named `.valetignore`; directories and files that are hidden (a name starting with `.`, or the Windows hidden or system attribute); reparse points; cloud placeholders. Each skip is reported with a reason (section 2).

**Tests.** `planner::tests::each_default_excluded_dir_is_skipped_once_and_not_descended`, `excluded_names_match_ascii_case_insensitively`, `excluded_name_only_applies_to_directories`, `valetignore_excludes_its_dir_and_everything_below`, `root_containing_valetignore_is_excluded`, `dot_file_is_hidden_and_dot_dir_not_descended`, `platform_hidden_file_and_dir_are_skipped`, `include_hidden_rule_plans_hidden_files`, `include_hidden_is_decided_by_the_winning_rule`. Also `index::tests::index_skips_placeholders_and_exclusions` for the file index.

**Partly met.**

- The exclusions apply to what is found below a scan root. A scan root is the user's choice, and a root that is itself called `node_modules`, or sits inside such a folder, is scanned (`excluded_names_in_root_path_are_an_explicit_opt_in`). The app checks only that a chosen folder exists (`save_config`). Nothing stops a user from choosing `C:\`; the exclusions would then apply to the folders below it by name, but not to the root.
- Exclusion is by directory name at any depth below a root, not by full path. A folder called `target` or `venv` anywhere below a root is skipped. `C:\Windows` is skipped only when it is reached from a root above it.
- The exclusions are not applied to destinations. A rule whose `moveTo` points into `C:\Program Files` is planned as written. No test; no code checks it.
- The list differs from PLAN 4.3: the code adds `venv` and `__pycache__`, and lists `Program Files` and `Program Files (x86)` as exact names instead of `Program Files*`.
- `includeHidden` (a rule option, `true` or `false`, default off) makes hidden files eligible for that rule. It does not make hidden folders scanned: hidden folders are skipped before any rule is looked at (read from the code; no test uses `includeHidden` with a hidden folder).

### Invariant 6. Skip files being written, locked files and cloud placeholders

**How the code meets it.**

- Being written: the planner skips a file whose last extension is `crdownload`, `part`, `tmp`, `download` or `partial` (any case), whose name starts with `~$`, or whose modification time is less than 10 seconds before the time of the plan (`RECENT_WRITE_WINDOW`).
- Locked: right before acting on a file, the executor asks `Platform::is_locked`. On Windows that opens the file for reading with sharing mode 0 and closes it at once, and treats errors 32 and 33 as "locked". It reads no data.
- Changed: the executor refuses a file whose size, modification time or type differs from the plan (`ChangedSincePreview`).
- Placeholders: see the cloud placeholder row in section 2. Short version: detected from file attributes without opening the file, checked before anything else touches the file, and never hashed.

**Tests.** `planner::tests::each_being_written_case_is_skipped`, `recently_modified_file_is_being_written`, `cloud_placeholder_file_and_dir_are_skipped`, `placeholder_wins_over_being_written_and_hidden`; `executor::tests::changed_size_or_mtime_fails_without_touching`, `locked_source_is_skipped`, `placeholder_is_never_hashed_or_moved`, `recover_placeholder_source_is_not_trashed_or_hashed`, `recover_placeholder_destination_with_source_gone_is_done_unhashed`. Windows only: `exclusive_handle_means_locked`, `offline_attribute_is_placeholder` (it sets the OFFLINE attribute on a normal file; it does not use a real OneDrive or Dropbox placeholder). Attribute bits: `fs_win::tests::attribute_bits`.

**Partly met.**

- The "being written" test is a heuristic on name and modification time. PLAN 4.3 also asks to ignore files whose size changes between two checks; that is not implemented.
- The lock check is a snapshot. A file can become locked right after the check. Then the rename or copy fails with an `Io` error and the operation is marked `failed`; for a copy the temp file goes to the Recycle Bin. No test for that race.
- PLAN 4.3 says a locked file is retried on the next three watcher passes. Not implemented: the watcher does not apply anything yet (section 4).
- Real OneDrive and Dropbox placeholders have not been used in any test. Detection relies on the attributes OFFLINE, RECALL_ON_OPEN and RECALL_ON_DATA_ACCESS. A directory that carries one of them is skipped with everything below it.

### Invariant 7. No network except updater, license check and opt-in crash reports

There is no network code in the workspace to restrict. The updater, license check and crash reports do not exist yet (Phase 3 and 4).

What was checked:

- `grep` for `reqwest`, `hyper`, `ureq`, `isahc`, `surf`, `attohttpc`, `TcpStream`, `UdpSocket`, `fetch(`, `XMLHttpRequest`, `WebSocket`, `updater` and `sentry` in all `.rs`, `.toml`, `.ts`, `.tsx`, `.json` and `.html` files outside `node_modules` and `target` (and `Cargo.lock`) finds nothing except the word "surfaced" in two test comments. The only HTTP address is `devUrl: http://localhost:5173` in `apps/desktop/src-tauri/tauri.conf.json`, the Vite dev server.
- `Cargo.lock` does contain `reqwest` and `hyper`. `cargo tree -i reqwest` shows they come from `tauri` and `tauri-plugin-log`. No Valet crate depends on them directly.
- The Tauri configuration has `"csp": null`, so the web view has no content security policy limiting what the page can request. The front end source contains no request calls (see the grep above).
- The journal, the history table and the logs hold full file paths on the local disk only.

Not verified: whether Tauri or its log plugin makes any request at runtime. No test or network capture exists. There is no test that fails when a network call is added.

## 2. What happens in each case

"Preview" is `dry_run`, which calls `planner::plan` and changes nothing. "Execute" is `executor::execute`. Before acting on a file, execute checks, in this order: the paths are valid Unicode and the destination has a folder; the source is not a placeholder; the source exists; it is a regular file and not a reparse point; size and modification time equal the plan; the file is not locked. Then it hashes the source, creates the destination folder if needed, and chooses the method.

Preview reasons for leaving a file alone (`SkipReason`, camelCase in the JSON): `excluded`, `hidden`, `reparsePoint`, `beingWritten`, `cloudPlaceholder`, `conflict` (a later rule that also matches), `alreadyInPlace`, `noExtension` (a `{ext}` folder and the file has no extension), `invalidDestination`, `unsupportedAction`, `unreadable`. Test: `planner::tests::skip_reason_serializes_camel_case` (checks one value).

Execution failures (`FailCode`, stored in the journal's `error` column as `Code` or `Code: detail`): `SourceMissing`, `ChangedSincePreview`, `CloudPlaceholder`, `Locked`, `NoRecycleBin`, `VerifyFailed`, `TrashFailed`, `Occupied`, `Interrupted`, `InvalidPath`, `Io`. A failed operation does not stop the batch.

### Normal operations and name handling

| Situation | What Valet does | Tests |
|---|---|---|
| Move, same volume | Hashes the source, journals `in_progress`, `rename_no_replace`, checks that the destination has the expected size and the source is gone, journals `done` with the hash. No Recycle Bin needed. | `executor::tests::same_volume_move_renames_and_records_hashes`, `same_volume_move_needs_no_recycle_bin` |
| Move, different volumes (or the volume check fails) | Copies to `.valet-tmp-<id>` in the destination folder, fsyncs, compares blake3 with the source hash, renames into place, sends the source to the Recycle Bin, checks the result. Needs a usable Recycle Bin on the source volume and on the destination volume. If the volume check returns an error, Valet takes this path. | `cross_volume_move_copies_verifies_and_trashes_source`, `failed_source_trash_removes_the_copy_again`, `copy_with_hash_mismatch_trashes_temp_and_fails`, `cross_volume_without_recycle_bin_touches_nothing` |
| Copy | Same as the cross-volume path without trashing the source, always (also within one volume). Needs a usable Recycle Bin on the destination volume only, because cleaning up a failed copy and undoing a copy both use it. | `copy_keeps_source_and_writes_identical_dest`, `cross_volume_without_recycle_bin_touches_nothing` (copy variant) |
| Name taken at preview time | The planner gives the file `name (2).ext`, then `(3)`, and so on. Two files going to the same name in one plan get different names, case ignored. | `planner::tests::existing_destination_file_gets_next_free_name`, `two_planned_ops_never_share_a_destination`, `reservation_is_case_insensitive`, `collision::tests::*` |
| Name taken at execution time (appeared after the preview) | `rename_no_replace` fails with `AlreadyExists`; the executor picks the next free name, records it in the journal, and retries. The existing file is untouched. | `collision_at_execution_time_gets_next_free_name` |
| Two rules match the same file | The first enabled rule with a `moveTo` or `copyTo` action is used. Later rules with such an action are listed as `conflict`. | `planner::tests::first_transfer_rule_wins_and_later_ones_conflict` |
| File already in the destination folder | Skipped as `alreadyInPlace` (case-insensitive folder comparison). | `planner::tests::file_already_in_destination_folder_is_already_in_place` |
| Destination folder missing | Created (`create_dir_all`) before the operation. It stays if the operation fails or is undone. | `executor::tests::undo_restores_byte_identical_tree` creates folders through execution; it compares files only |
| Action other than `moveTo` and `copyTo` (`tag`, `rename`, `quarantine`, anything else) | Not run. Reported once per rule as `unsupportedAction` when nothing was planned for the file. | `planner::tests::unsupported_actions_reported_once_per_rule_when_nothing_planned`, `unsupported_actions_reported_when_winner_destination_fails`, `tag_only_rule_before_mover_is_not_reported` |
| Destination template (`~`, `%VAR%`, `{year}`, `{month}`, `{ext}`) cannot be expanded | Skipped as `invalidDestination` (no home folder, unset variable, empty path) or `noExtension`. | `planner::tests::tilde_without_home_is_invalid_destination`, `missing_env_var_is_invalid_destination_and_set_var_expands`, `empty_or_missing_path_param_is_invalid_destination`, `ext_token_without_extension_is_no_extension` |
| Path is not valid Unicode | The planner lists it (names are converted with lossy conversion for matching). The executor refuses it with `InvalidPath` before any other check, because the journal stores paths as text and could not name the file exactly. | no test |

### Files that change, disappear, or are in use

| Situation | What Valet does | Tests |
|---|---|---|
| File changed after the preview (size, modification time, or no longer a regular file) | `failed` with `ChangedSincePreview`; nothing touched; no hash taken. A change during the copy is caught by the hash comparison (`VerifyFailed`, temp file to the Recycle Bin). A change in the short time between the check and a same-volume rename is not detected. | `changed_size_or_mtime_fails_without_touching`, `copy_with_hash_mismatch_trashes_temp_and_fails` |
| File missing at execution | `failed` with `SourceMissing`; the batch goes on. | `missing_source_fails_and_batch_continues` |
| File locked by another process | `failed` with `Locked`; nothing touched; no hash taken. Not retried (the watcher that would retry does not apply anything yet). | `locked_source_is_skipped`; Windows: `exclusive_handle_means_locked` |
| File being written | Skipped at preview as `beingWritten`: extensions `.crdownload .part .tmp .download .partial`, names starting with `~$`, or modified less than 10 seconds before the plan. The 10 seconds are measured from the time of the plan, not of the execution. | `planner::tests::each_being_written_case_is_skipped`, `recently_modified_file_is_being_written` |
| Cloud placeholder (OneDrive, Dropbox, not downloaded) | Detected from attributes (`FILE_ATTRIBUTE_OFFLINE`, `RECALL_ON_OPEN`, `RECALL_ON_DATA_ACCESS`) with `GetFileAttributesW`, which reads the directory entry and does not open the file. The planner checks it first for every entry, before the symlink, hidden and "being written" checks, and a placeholder directory is not entered. The executor checks it again before it opens or hashes anything (`failed`, `CloudPlaceholder`). The lock probe also refuses to open a placeholder. Recovery and undo check it before every hash. Valet never opens, reads or hashes one. | `planner::tests::cloud_placeholder_file_and_dir_are_skipped`, `placeholder_wins_over_being_written_and_hidden`; `executor::tests::placeholder_is_never_hashed_or_moved`, `recover_placeholder_*`; `index::tests::index_skips_placeholders_and_exclusions`. Windows (simulated by the OFFLINE attribute): `offline_attribute_is_placeholder` |
| Junction, symlink, other reparse point | Skipped as `reparsePoint`, not entered, not moved. On Windows only name-surrogate tags count (symlink, junction, mount point). OneDrive and Dropbox reparse points are not treated as links. If the tag cannot be read, the entry is treated as a link. The executor refuses a source that has become a reparse point (`ChangedSincePreview`). A scan root that is itself a link was not checked. | `planner::tests::symlinked_file_and_dir_are_reparse_points` (Linux symlinks), `platform_reparse_dir_is_not_descended`; `fs_win::tests::reparse_tag_name_surrogate_bit`; Windows (each skips itself if links cannot be created): `symlink_is_reparse_point`, `junction_is_reparse_point`, `plain_file_and_dir_are_not_reparse_points` |
| Hidden or system files | Names starting with `.` and files or folders with the Windows hidden or system attribute are skipped as `hidden`. A rule with `"options": {"includeHidden": true}` lets its hidden files be planned. Which rule decides: the winning rule, or the first matching rule when none wins. Hidden folders stay skipped. | `planner::tests::dot_file_is_hidden_and_dot_dir_not_descended`, `platform_hidden_file_and_dir_are_skipped`, `include_hidden_rule_plans_hidden_files`, `include_hidden_is_decided_by_the_winning_rule`, `dot_named_root_is_scanned`; Windows: `hidden_and_system_attributes` |
| Default exclusions and `.valetignore` | Excluded folders are skipped with everything below, one `excluded` entry for the folder. A `.valetignore` file (any content) in a folder, including a scan root, excludes the folder. A scan root named like an excluded folder is scanned; excluded folders below it are not. See invariant 5 for the limits. | `planner::tests::each_default_excluded_dir_is_skipped_once_and_not_descended`, `valetignore_excludes_its_dir_and_everything_below`, `root_containing_valetignore_is_excluded`, `excluded_names_in_root_path_are_an_explicit_opt_in`, `excluded_dir_chosen_as_root_is_scanned_in_any_root_order`, `nested_root_with_valetignore_is_excluded_in_any_root_order` |
| Overlapping or missing scan roots | Each file is planned once. A root that does not exist is reported as `unreadable` and the other roots are still scanned. | `planner::tests::overlapping_and_repeated_roots_plan_each_file_once`, `nonexistent_root_is_unreadable_and_others_still_scanned` |
| Long paths (over 260 characters) | Every file call in the planner and executor goes through `Platform::os_path`, which on Windows adds the `\\?\` prefix (`\\?\UNC\` for network paths), turns `/` into `\` and resolves `.` and `..` in the text. The attribute, lock, volume and rename helpers build the prefixed form themselves. The journal stores the path without the prefix. The `trash` crate is called with the unprefixed path. | `fs_win::tests::long_path_*` (string rules, any OS); Windows: `long_path_create_rename_read`, `long_path_rename_no_replace`, `trash_long_path_never_loses_the_file_silently` (accepts a refusal by the shell) |
| Volume without a usable Recycle Bin (`can_trash` false) | A same-volume move still works (no trash needed). A cross-volume move or a copy fails with `NoRecycleBin` before anything is copied. Undo of a cross-volume move fails with `NoRecycleBin` and leaves the file where it is. `trash_path` refuses with `Unsupported`. | `cross_volume_without_recycle_bin_touches_nothing`, `same_volume_move_needs_no_recycle_bin`, `undo_cross_volume_without_recycle_bin_fails_untouched`. The real `can_trash` is tested only for the positive case (Windows `temp_volume_supports_trash`) |
| Permission denied, Controlled Folder Access, disk full, other I/O error | `failed` with `Io: <system message>`. No special wording for Controlled Folder Access and no retry. If a copy had started, its temp file goes to the Recycle Bin. | no test |

### Crash in the middle of a batch

Each operation is written to the journal as `planned` right before it is executed, so a crash leaves at most the operation being run (and possibly its trash step) unfinished. At the next start, `recover` reads every `planned` or `in_progress` row and decides it from the disk. It compares the destination with the source hash saved when the operation went `in_progress`. It writes one `recovery` batch, whose summary lists each decision, and closes the batches the crash left open ("interrupted; recovered by batch ..."). Files that recovery has to remove (a leftover temp copy, the source of a finished copy) go to the Recycle Bin as journaled `trash` operations in that batch. A second run finds nothing to do.

| Journal state | On disk | Decision | Tests |
|---|---|---|---|
| `planned` | not looked at | `failed` (`Interrupted`): "never started; nothing was touched". | `recover_planned_op_fails_and_closes_batch` (also checks that a second run does nothing) |
| `in_progress`, move or copy | destination has the journaled hash, source gone | `done`, with the destination hash. | `recover_move_already_at_destination_is_done` |
| `in_progress`, copy | destination has the hash, source present | `done`. | `recover_copy_states` |
| `in_progress`, move | destination has the hash, source present and still has the same hash | Source goes to the Recycle Bin (a journaled `trash` operation); `done`. | `recover_move_copied_but_source_not_trashed_trashes_source` |
| same | the source cannot be sent to the Recycle Bin | The copy at the destination goes to the Recycle Bin; the file stays at the source; `failed` (`TrashFailed`). If the copy cannot be trashed either, the file exists twice (duplicate case 1 above). | `recover_move_source_trash_failure_removes_copy` (the second failure: no test) |
| same | source present but changed, unreadable, or a placeholder | The copy at the destination goes to the Recycle Bin; source kept; `failed`. The source is not hashed if it is a placeholder. | `recover_move_changed_source_keeps_source_and_trashes_copy`, `recover_placeholder_source_is_not_trashed_or_hashed` |
| `in_progress`, move or copy | destination missing, source present | A leftover `.valet-tmp-<id>` next to the destination goes to the Recycle Bin; source kept; `failed`. | `recover_move_interrupted_mid_copy_trashes_temp_keeps_source`, `recover_move_not_started_on_disk_fails_without_cleanup`, `recover_copy_states` |
| same | destination exists with other content, source present | Destination left alone, temp (if any) to the Recycle Bin, source kept; `failed`. | `recover_leaves_foreign_destination_alone` |
| same | destination differs, source gone | Left as is; `failed`, with a note that the destination differs from the journal. | no test |
| same | destination and source both missing | Nothing to do; `failed`, with a note (names the temp file if one exists). | no test |
| same | destination is a placeholder | Not hashed. Source gone: `done` without a hash. Copy with source present: `done` without a hash. Move with source present: left as is, `failed` (duplicate case 2 above). | `recover_placeholder_destination_with_source_gone_is_done_unhashed` (the other two: no test) |
| `in_progress`, `trash` | file still at its path | `failed`. | `recover_trash_op_by_disk_state` |
| same | file no longer at its path | `done` (not checked in the Recycle Bin). | `recover_trash_op_by_disk_state` |
| `in_progress`, an undo operation | as for move or copy | When it is decided `done`, the operation it reverts becomes `rolled_back` in the same transaction. | `recover_interrupted_undo_rolls_back_original` |
| all of the above together | five interrupted moves | Every file exists once (at source or destination), no temp file left, no unfinished operation. | `recover_leaves_every_moved_file_exactly_once` |

### Undo

Undo applies to one `apply` batch that has finished (recovery closes batches a crash left open, so run it first; `undo_batch` refuses an unfinished batch). It cannot undo an `undo` or `recovery` batch. It walks the batch's operations from last to first.

| Situation | What Valet does | Tests |
|---|---|---|
| Destination file unchanged, original path free | Moves it back. Same volume: `rename_no_replace`. Different volumes: copy, verify, rename, destination copy to the Recycle Bin (needs the bin on both sides). The original becomes `rolled_back` in the same transaction that marks the undo step `done`. | `undo_restores_byte_identical_tree` (both paths) |
| Destination file modified (hash differs from the recorded one) | Nothing is moved; the operation is returned as needing confirmation (`Modified`). With `force` it is moved back as it is now. | `undo_with_modified_destination_needs_confirmation` |
| Original path occupied (a file with that name exists again) | Nothing is moved; needs confirmation (`SourceOccupied`). With `force` the file is restored beside it as `name (2).ext` and the new path is listed in `restored_as`. The existing file is not touched. If the path is taken between the check and the rename and `force` was not given, the operation is returned as needing confirmation. | `undo_with_occupied_source_needs_confirmation` (the race: no test) |
| Undoing a copy | The copy goes to the Recycle Bin. If the original no longer exists, the copy is the last one: needs confirmation (`OriginalMissing`). | `undo_restores_byte_identical_tree` (copy sent to the bin), `undo_copy_with_original_gone_needs_confirmation` |
| File no longer at the destination | Reported as failed (`SourceMissing`). | no test |
| Destination is a placeholder, or locked, or not a regular file | Reported as failed (`CloudPlaceholder`, `Locked`, `ChangedSincePreview`). A placeholder is not hashed. | no test |
| Operation that was not `done` (failed, already rolled back) | Not touched; listed as skipped. A second undo of the same batch finds nothing to do. | `undo_skips_failed_ops_and_lists_them`, `undo_restores_byte_identical_tree` |
| Folders that the batch created | Left in place. Valet has no code that removes a folder. The tests compare files, so they do not assert this. | no test |
| `trash` operations written by recovery | Not undone by Valet; the file can be restored from the Windows Recycle Bin by hand. | no test |
| No Recycle Bin on the volumes involved (cross-volume restore) | Fails with `NoRecycleBin`; the file stays where it is. | `undo_cross_volume_without_recycle_bin_fails_untouched` |
| Unknown, unfinished or non-`apply` batch | Error, nothing touched. | `undo_rejects_unknown_open_and_non_apply_batches` |

## 3. The journal

The journal is two SQLite tables in the same file as the rules (`valet.sqlite3` in the app's config folder), created by `crates/valet-core/migrations/0003_journal.sql`. Code: `crates/valet-core/src/journal.rs`.

**Batches** (`batches`): `id` (UUID), `kind` (`apply`, `undo` or `recovery`), `created_at`, `finished_at` (empty while the batch is open), `summary`.

**Operations** (`operations`): `id` (autoincrement, never reused, so `.valet-tmp-<id>` names stay unique), `batch_id`, `seq`, `op_type` (`move`, `copy` or `trash`), `source_path`, `dest_path`, `size`, `hash_before`, `hash_after`, `state`, `error`, `created_at`, `finished_at`, `undo_of` (the operation an undo step reverts). The database enforces the allowed values of `kind`, `op_type` and `state` with `CHECK` constraints.

**States and allowed moves.** `planned`, `in_progress`, `done`, `failed`, `rolled_back`:

| From | To |
|---|---|
| `planned` | `in_progress`, `failed` |
| `in_progress` | `done`, `failed` |
| `done` | `rolled_back` |
| `failed`, `rolled_back` | none (final) |

Any other change is rejected with `InvalidTransition` and the state stays as it was. The update itself carries the allowed source states in its `WHERE` clause, so a stale caller cannot move a row that already changed. Tests: `journal::tests::state_machine_table`, `every_disallowed_transition_is_rejected_and_state_kept`, `missing_op_is_reported`, `happy_path_records_fields_per_state`.

**What is written when.**

- `planned`: source, destination, size, written right before the operation is looked at.
- `in_progress`: `hash_before` (blake3 of the source), written before any file is changed.
- `done`: `hash_after` and `finished_at`. For a rename or a verified copy this is the same as `hash_before`.
- `failed`: `error` (`Code` or `Code: detail`) and `finished_at`.
- `rolled_back`: set on the original when its undo step is `done`.
- `dest_path` is rewritten when a collision at execution time picks a new name, before the retry, so recovery looks in the right place (`journal::tests::set_dest_rejected_once_finished` covers that a finished row refuses it).

**One transaction per state change.** Each change of state is its own transaction (`Journal::transition`). A batch is never one transaction. The only transaction that touches two rows is `mark_undo_done`: it marks the undo step `done` and the original `rolled_back` together, or neither (`journal::tests::undo_done_and_rollback_are_atomic`). `set_dest` is a single statement and not a state change.

**WAL.** `Db::connect` opens SQLite in WAL mode (`crates/valet-core/src/storage.rs`). Test: `journal::tests::connect_enables_wal`. The code leaves `synchronous` at the SQLite default.

**Other points.**

- Paths are stored as text. The executor refuses paths that are not valid Unicode before they reach the journal.
- Nothing deletes journal rows. There is no retention limit, so the journal grows.
- The older `operation_history` table (the statistics and "recent operations" the UI used before the journal) is separate. `clear_operation_history` clears only that table. Test: `storage::tests::clearing_history_keeps_the_journal`.
- The journal holds full paths. It stays on the local disk.

## 4. What is not covered yet

Planned items come from `docs/PLAN.md`. Nothing here is a promise beyond what the tests above show.

| Gap | State in the code | Where planned |
|---|---|---|
| Actions other than move and copy: rename, tag, send to the Recycle Bin as a rule action, report | The planner reports them as `unsupportedAction` and the executor does not run them. The journal's `trash` type exists, but only for Valet's own cleanup in recovery. | Phase 2 (PLAN: "acțiuni: trimite la Coș, raportează") |
| Watcher that applies rules by itself, only after the first approved preview, paused during preview | The watcher (`valet-platform`, `notify` with a 2 second debounce) delivers batches of changed paths. The app's callback only logs a count (`setup.rs`, a `TODO`). The `paused` flag set from the tray menu is not read by any other code. | Phase 2 (PLAN 4.4) |
| Rescan when the Windows directory-change buffer overflows | Not working on Windows. `notify` 6.1.1 does not report the overflow: its Windows backend ignores the byte count of the completion and sets no rescan flag (read from `windows.rs` in the crate source). Valet's watcher asks for a rescan of every root on a rescan flag (which Linux inotify and macOS FSEvents send) and on a watcher error, so the overflow case on Windows is a gap: events can be lost silently. Tests: `watcher::tests::rescan_event_becomes_one_event_per_root`, `watcher_error_requests_rescan_of_all_roots` use synthetic events, not a real overflow. Nothing consumes the rescan request yet. | Not assigned |
| Retry of locked files over three watcher passes; "size changed between two checks" | Not implemented (see invariant 6). | Not assigned |
| Single instance | Not implemented (see invariant 3). | Phase 2 (PLAN: tauri-platform) |
| Explained error for Controlled Folder Access | Not implemented; shows as `Io`. | Not assigned |
| Panic hook that marks `in_progress` operations | Not implemented. Recovery at the next start covers the same case. | Not assigned (PLAN 4.5) |
| Log scrubbing, log rotation, DEBUG switch in settings | At `4098837`, INFO lines carry counts and no paths (commit `b2b1969`; before it, `setup.rs` logged the config file path and contents at INFO). But `apps/desktop/src-tauri/src/main.rs` sets the log level to Debug in every build, and DEBUG lines still print full paths (watched folders, changed files, planned operations). The log plugin is used with its defaults: stdout and a log file, one file kept. Rotation of 5 files of 2 MB and a DEBUG toggle are not implemented. | Phase 3 (PLAN 4.5, tauri-platform "loguri cu rotație") |
| Opt-in crash reports with path scrubbing; updater; license check | None of the three exists. | Phase 3 and 4 |
| Kill-the-process crash test | The driver exists (`valet-fault`); the in-process tests above do not kill a process. | Phase 1 (qa) |
| Real OneDrive and Dropbox, removable drives, network shares, Recycle Bin size limit, Controlled Folder Access | Not tested. Placeholders are simulated with the OFFLINE attribute; `can_trash` is tested only for a normal fixed drive. | Manual checklist for Phase 1 |
| Excluding destinations; guarding the scan root (for example `C:\`) | Not implemented (see invariant 5). | Not assigned |
| Executing exactly the previewed list | At `4098837`, `execute_operations` plans again (see invariant 3). | Phase 2 (preview and apply in the UI) |
