# Safety

This document says what Valet does to your files, why it cannot lose or overwrite them in the cases we have tested, and where it can. It is written from the code, not from the plan. Where the code does less than `docs/PLAN.md` section 2.6 says, it says so.

It is also the source for the future FAQ, so each section can be read on its own.

Written against the Rust code at commit `a461d81`. The file index (`index.rs` and the storage functions it uses) was being changed at the same time and is not described here. Valet is a pre-release prototype. Nobody should point it at folders they cannot afford to lose.

## How to read this document

- "Test" names are Rust test functions. `executor::tests::x` means function `x` in `crates/valet-core/src/executor/tests.rs`; `planner::tests::x` is `crates/valet-core/src/planner/tests.rs`; `journal::tests::x` is in `crates/valet-core/src/journal.rs`. Files under `crates/valet-core/tests/` are named by file (`fault_injection.rs`, `prop_planner.rs`, `prop_collision.rs`, `integration_10k.rs`). `commands::operations::tests::x` and `state::tests::x` are in the app crate (`apps/desktop/src-tauri/src`). Platform tests are named by file.
- Most tests use `FakePlatform` (`crates/valet-core/src/platform.rs`, module `testing`): a Recycle Bin that is a folder, and sets of paths that the test declares "locked", "placeholder" or "hidden". They run on Linux. They prove the logic of the planner, executor, recovery and undo. They do not prove that Windows behaves as the code assumes.
- The Windows code (`GetFileAttributesW`, `MoveFileExW`, the registry checks in `can_trash`, the lock probe, the Recycle Bin through the `trash` crate) is compiled only on Windows (`mod win` in `crates/valet-platform/src/fs_win.rs`). Its tests are in `crates/valet-platform/tests/windows.rs` and `crates/valet-platform/tests/windows_e2e.rs`; both start with `#![cfg(windows)]`. `windows_e2e.rs` runs the real `SystemPlatform` with valet-core's planner, executor and journal on NTFS and needs the feature `valet-core` (CI passes `--features valet-core`). In the Linux session that produced this document both files ran 0 tests (`windows_e2e` is not even built there).
- CI (`.github/workflows/ci.yml`) runs the Windows tests on `windows-latest`. Checked with `gh run list`: the last run that passed is for `7800ef5`, the commit that added `windows.rs`. The runs for `1f51239`, `818d65c`, `736f802`, `a461d81` and `a582813` failed at the "Clippy" step in both the Linux and the Windows job, so their test steps did not run. Clippy passes locally (`cargo clippy -p valet-core -p valet-platform --all-targets -- -D warnings`). The CI log could not be fetched, so the cause is unknown. `windows_e2e.rs` and the 100-iteration fault injection have never run in CI. No Windows test result for the current code has been seen.
- A statement with no test named is read from the code only. It is marked "no test".

Commands run to check this document, from the repository root, on Linux (all with `--locked` and a separate `CARGO_TARGET_DIR`; nothing else was different):

```
cargo test -p valet-core -p valet-platform
VALET_FAULT_ITERS=100 cargo test -p valet-core --features test-support --test fault_injection -- --nocapture
cargo test -p app
```

- The first command passed: 119 unit tests in `valet-core`, 24 in `valet-platform`, `fault_injection` (1 test, 10 iterations by default), `integration_10k` (2 tests: 1k and 10k files, about 50 s in a debug build), `prop_collision` (6), `prop_planner` (2), the characterization tests, and `force_cross_volume`. The perf tests are `#[ignore]` and did not run.
- The second command ran 100 iterations: 0 failures, 49.8 s. See invariant 3 for what it showed.
- The third command passed: 12 tests in the app crate. It needs the webkit2gtk-4.1 development packages, which this machine has.

## 1. The invariants

The seven invariants are in `CLAUDE.md`. Invariant 8 in `docs/PLAN.md` (CPU, memory and speed budgets) is not covered here.

Summary:

| # | Invariant | Status |
|---|---|---|
| 1 | Never delete permanently | Met in code. Windows Recycle Bin edge cases not tested. |
| 2 | Never overwrite | Met on Windows by the API used; on Linux the check is not atomic (not a product target). |
| 3 | Journal first; each file exactly once after a crash | Met for the crash states we tested: unit tests, and a kill-and-recover test (SIGKILL, Linux, fake platform). Two cases leave a file twice (never zero times). Nothing tested on Windows. |
| 4 | Undo while destinations are unchanged | Met, with listed limits. |
| 5 | Default exclusions | Mostly met. Sources: apply only below the folders the user chose. Destinations inside an excluded folder or under a `.valetignore` are refused, with an exemption for the path shared with the scan root. The scan root itself is not checked. |
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
- Windows only: `temp_volume_supports_trash` and `trash_long_path_never_loses_the_file_silently` in `crates/valet-platform/tests/windows.rs`. The second accepts either outcome (trashed, or an error with the file still there). `windows_e2e.rs` does not use the Recycle Bin (its moves are same-volume).
- `fault_injection.rs` (odd iterations use the copy, verify, trash path with the fake bin) checks after each crash that no `.valet-tmp-*` file is left outside the fake bin and that originals exist once.

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
- `crates/valet-core/tests/prop_collision.rs` (proptest, 256 generated cases per property): `repeated_reservations_are_distinct_and_free`, `reservations_distinct_case_insensitively`, `stable_across_runs`, `suffix_rule_and_smallest_free_number`, `no_extension_suffix_at_end`, plus the fixed `double_extension_example`.
- `crates/valet-core/tests/prop_planner.rs` (`plan_invariants`, 64 generated trees and rule lists, and `case_variants_into_one_folder`): one op per source, destinations unique ignoring case and not present on disk, destination folder equal to the expanded template, first matching rule wins, same plan on a second run.
- `crates/valet-core/tests/integration_10k.rs` (`integration_1k`, `integration_10k`): pre-existing files at destination names are byte-identical after execute, and a collision count above zero is asserted so that renamed destinations occur. Same-volume path only.
- `fault_injection.rs`: pre-existing files at planned destination names are unchanged after kill, recovery and undo.
- `planner::tests::existing_destination_file_gets_next_free_name`, `two_planned_ops_never_share_a_destination`, `reservation_is_case_insensitive`, `copy_reserves_destination_too`.
- `executor::tests::collision_at_execution_time_gets_next_free_name` (same-volume move, cross-volume move, copy).
- `fs_win::tests::rename_no_replace_refuses_existing_destination`, `rename_no_replace_refuses_existing_directory` (run on every OS), `system::tests::delegates_rename_and_trash`, and `state::tests::platform_never_replaces_existing_files` in the app crate (the app crate was not built in the session that wrote this document).
- Windows only: `long_path_rename_no_replace`; end to end, `same_volume_move_then_undo_restores_tree` in `windows_e2e.rs` (a taken name becomes `a (2).pdf`, the pre-existing file keeps its content).

**Not met / partly met.**

- Off Windows, `rename_no_replace` checks that the target is absent and then calls `std::fs::rename`. Another process can create the target in between and the rename then replaces it. The code comments say this is accepted on Linux because Linux is not a product target. The tests that run on Linux therefore show that the refusal works, not that it is race-free. No test creates a file between the check and the rename.
- The limit of 100 collision retries has no test.
- If a user file already has the name `.valet-tmp-<id>` in the destination folder, the copy fails with `Io` and nothing is overwritten (code only, no test).

### Invariant 3. Journal before, mark after; each file exactly once after a crash

**How the code meets it.**

- Each operation gets a `planned` row in the journal immediately before it is looked at, then `in_progress` (with the source hash) before any file is touched, then `done` or `failed` after. See section 3.
- On startup the app calls `executor::recover` before it starts the watcher (`apps/desktop/src-tauri/src/setup.rs`). It decides every unfinished operation from what is on disk, without asking the user. The decisions are in the crash table in section 2.
- The order of steps in a cross-volume move is: copy to a temp file, fsync, compare blake3 with the hash taken before, rename into place, trash the source. The source is trashed last, so at every moment before that the original is still in place.
- By the design of the single-instance plugin, only one Valet process runs at a time, so recovery does not run while another process executes a batch (not tested, see below). `tauri-plugin-single-instance` (pinned `=2.5.5`) is the first plugin in `main.rs`, before the log plugin. In Tauri 2.12.5 (read from its source) plugins are initialized in `Builder::build` and the `setup` hook, which runs recovery, is called later from `App::run`. On Windows the plugin creates a named mutex (`<app identifier>-sim`) in its setup; if the mutex already exists, the second process passes its arguments to the first one and calls `std::process::exit(0)` inside that call (`platform_impl/windows.rs` in the plugin). The first instance shows its window. So a second process exits before `setup` and never reaches recovery. Inside one process, `file_ops` in `state.rs` keeps batches and undo from overlapping.

**Tests.** `executor::tests::recover_*` (listed in section 2) and in particular `recover_leaves_every_moved_file_exactly_once`, which builds five interrupted moves (planned, not started, mid-copy, placed but source not trashed, finished) and checks that each file's content exists once, no temp file is left, and no operation stays unfinished. The process-kill test `kill_during_apply_then_recover_and_undo` in `crates/valet-core/tests/fault_injection.rs` is described next. The single-instance behavior has no test: nothing starts two processes, and what is stated above comes from the plugin source.

**The kill test.** Each iteration builds a fixture: 40 files of 0 to 256 KiB in three folder levels, six extensions, some sharing a name, plus pre-existing files at planned destination names. It starts `valet-fault apply` (`crates/valet-core/src/bin/valet-fault.rs`, built only with `test-support`, using `FakePlatform` with a Recycle Bin folder) as a child process, waits 10 to 500 ms and kills it with `Child::kill` (SIGKILL on Linux) unless it has finished. Then it runs `valet-fault recover` as a new process and checks:

- every original file exists exactly once (at its source, or at one destination) with its original blake3, and the pre-existing files are unchanged;
- no `.valet-tmp-*` file is left, no operation and no batch is unfinished, the recovery batch has a summary, and recovery decided exactly as many operations as the crash left open;
- every `done` operation has its `hash_after` at its destination and its source gone, and every `failed` operation has error text;
- then `valet-fault undo` restores the original tree byte for byte and leaves only the pre-existing files in the destination.

Even iterations use the same-volume rename path; odd ones set `VALET_FORCE_CROSS_VOLUME=1` (copy, verify, rename, trash). Half of the runs add a 5 to 20 ms pause after each journal change, a quarter 1 to 4 ms, a quarter none, so kills also land inside file I/O and commits. Each iteration's fixture and delays come from a fixed seed (the iteration number), but the kill time is wall-clock, so two runs do not hit the same states. The default is 10 iterations (`VALET_FAULT_ITERS`); CI sets 100.

Result of one local run with 100 iterations: 0 failures, 49.8 s. 76 runs were killed during the batch, 5 before the batch was created, 19 had finished. Recovery made 55 decisions: 27 `planned` to `failed` ("never started"), 26 `in_progress` to `failed` ("not at destination"), 2 `in_progress` to `done` ("verified at destination; source gone": the move had already completed). In 8 iterations the disk was not yet in the "exactly once" state when recovery started (this counts any failed check, not only duplicates). 91 batches were then undone. In this run no kill left an `in_progress` operation with the file at both source and destination, so the branch "copy verified at destination; source sent to the Recycle Bin" and the other branches in the crash table were exercised by the unit tests only. Other runs can land elsewhere.

**Limits of the evidence.**

- The kill test drives `planner::plan`, `executor::execute`, `recover` and `undo_batch` through the `valet-fault` driver. It does not run the Tauri app, `SystemPlatform` or Windows. The "volumes" are one volume with `same_volume` forced to false, and the Recycle Bin is a folder.
- A killed process is not a power loss. The operating system still writes out what the process had written. Power loss was not tested. The code does not set SQLite's `synchronous` setting; sqlx leaves SQLite's default (FULL), so commits are flushed in WAL mode. The `.valet-tmp` file is fsynced (`sync_all`) before the compare; a plain same-volume rename is not followed by a directory flush.
- The recovery unit tests construct the journal and disk state that a crash would leave; they do not kill anything.
- The kill test has not run in CI (see "How to read this document").

**Two cases where a file can exist twice.** In both, the file is never lost; it is present at the source and at the destination, the operation is marked `failed`, and the error text names both paths.

1. A cross-volume move (or a copy that has to be taken back) where the source cannot be trashed, and then the copy cannot be trashed either. The first failure is handled (the copy goes to the Recycle Bin and the source stays: `executor::tests::failed_source_trash_removes_the_copy_again`). The second failure has no test. The same double failure is possible in undo of a cross-volume move and in recovery.
2. Recovery of a move that was in progress, finding that the destination is a cloud placeholder while the source file still exists. Recovery does not hash a placeholder, so it cannot verify the destination; it leaves both in place and marks the operation `failed`. No test (the test `recover_placeholder_destination_with_source_gone_is_done_unhashed` covers the case where the source is gone).

**Other gaps.**

- Recovery runs only at startup. If it fails (for example the journal cannot be opened for writing), `setup.rs` logs an error and starts anyway.
- The instance lock is a named mutex without a `Global\` prefix. On Windows such a name should be limited to one logon session (from the Windows naming rules, not verified here), so the same user in two sessions at once could start two instances on the same journal. Not tested.
- There is no panic hook that marks `in_progress` operations (PLAN 4.5). A panic is treated like any crash: recovery decides at the next start.
- The test pause `VALET_FAULT_SLOW_MS` is compiled only with the `test-support` feature (`executor.rs`, `fault_pause`; without the feature it is an empty function). The app depends on `valet-core` without that feature (`apps/desktop/src-tauri/Cargo.toml`) and `valet-fault` requires it. Checked: a `valet-core` library build without the feature (debug and release) contains the name only inside a doc comment, not as a constant; with the feature the constant is there; and the test binary of the app crate built here contains no occurrence (`strings`). The release installer was not built. There is no test for the absence: `cargo test` turns the feature on through a self dev-dependency (the code comment says so).
- A different switch is in shipped code: `valet_platform::same_volume` (`fs_win.rs`) returns "not the same volume" when the environment variable `VALET_FORCE_CROSS_VOLUME` is `1`. In that case every move takes the copy, verify, trash path, which needs a Recycle Bin on both volumes. Nothing in it skips a check. Test: `crates/valet-platform/tests/force_cross_volume.rs`.
- Execution runs the list the user previewed, not a new plan. The details are in the "Execute after preview" row of section 2. The plan is kept only in memory and has no expiry. The executor re-checks missing, changed, placeholder and locked files, but not exclusions, hidden state or `.valetignore`: those are decided when the preview is made.

### Invariant 4. Undo while destinations are unchanged

**How the code meets it.** `executor::undo_batch` (`crates/valet-core/src/executor/undo.rs`) undoes the `done` operations of one finished `apply` batch, last first, as a new `undo` batch. Before touching a file it hashes the file at the destination and compares with the hash recorded when it was moved. If they differ, or if the original path is now taken, nothing is moved and the operation is returned as "needs confirmation". Calling again with `force` does it anyway, still without overwriting. The details are in the undo table in section 2.

**Tests.** `executor::tests::undo_restores_byte_identical_tree` (same-volume and cross-volume; compares the files of the tree by content before and after; a second undo finds nothing to do), `undo_with_modified_destination_needs_confirmation`, `undo_with_occupied_source_needs_confirmation`, `undo_skips_failed_ops_and_lists_them`, `undo_copy_with_original_gone_needs_confirmation`, `undo_cross_volume_without_recycle_bin_fails_untouched`, `undo_rejects_unknown_open_and_non_apply_batches`, `recover_interrupted_undo_rolls_back_original`, `journal::tests::undo_done_and_rollback_are_atomic`. Larger runs: `integration_10k.rs` undoes 1,000 and 10,000 same-volume moves and compares the whole tree by content hash; `fault_injection.rs` undoes the batch that a killed and recovered run left (91 of 100 iterations in the local run) and requires the original tree back, byte for byte, with no failed, unconfirmed or renamed restores. Windows only, never seen passing: `same_volume_move_then_undo_restores_tree` and `long_paths_move_and_undo` in `windows_e2e.rs`.

**Limits.**

- The comparison is a content hash, not a "modified by the user" flag. A file edited and then edited back to the same bytes counts as unchanged.
- Only files are compared in the test; folders that a batch created are left behind (see the undo table).
- Undo cannot restore a file that was moved or deleted after the batch (`SourceMissing`, reported as a failure).
- Operations that are not `done` (failed, already rolled back) are listed as skipped and not touched.
- The Tauri commands `undo_batch` and `execute_operations` (`apps/desktop/src-tauri/src/commands/operations.rs`) take the same lock, so a batch and an undo do not run at the same time in one process.

### Invariant 5. Default exclusions

**How the code meets it.** The planner walks each scan root with `walkdir` (`follow_links(false)`) and skips, without descending: directories whose name (ASCII case-insensitive) is one of `Windows`, `Program Files`, `Program Files (x86)`, `ProgramData`, `AppData`, `$Recycle.Bin`, `System Volume Information`, `.git`, `node_modules`, `target`, `.venv`, `venv`, `__pycache__` (`DEFAULT_EXCLUDED_DIRS` in `planner.rs`); any directory that contains a file named `.valetignore`; directories and files that are hidden (a name starting with `.`, or the Windows hidden or system attribute); reparse points; cloud placeholders. Each skip is reported with a reason (section 2).

**Destinations.** The planner also checks where a file would go. After it expands the destination template (`~`, `%VAR%`, `{year}`, `{month}`, `{ext}`), joins a relative folder to the scan root and resolves `.` and `..` in the path text, it skips the file as `invalidDestination` (no operation is planned) when:

- a component of the folder is one of the `DEFAULT_EXCLUDED_DIRS` names (ASCII case-insensitive), looking only at the components after the part the folder shares with the scan root; or
- the folder, or any folder above it up to the file system root, contains a file named `.valetignore`. The folder itself does not have to exist yet.

"Shares with the scan root" means the leading components of the folder that equal, ignoring ASCII case, the leading components of the root the file was found under (`in_excluded_dir` in `planner.rs`). The exemption is there because a scan root can be inside an excluded name: on Windows the temp folder is under `AppData`, where the tests create their folders. The `.valetignore` lookup has no such exemption.

**Tests.** Sources: `planner::tests::each_default_excluded_dir_is_skipped_once_and_not_descended`, `excluded_names_match_ascii_case_insensitively`, `excluded_name_only_applies_to_directories`, `valetignore_excludes_its_dir_and_everything_below`, `root_containing_valetignore_is_excluded`, `dot_file_is_hidden_and_dot_dir_not_descended`, `platform_hidden_file_and_dir_are_skipped`, `include_hidden_rule_plans_hidden_files`, `include_hidden_is_decided_by_the_winning_rule`. Also `index::tests::index_skips_placeholders_and_exclusions` for the file index. Destinations: `planner::tests::destination_inside_windows_dir_is_invalid`, `destination_inside_appdata_is_invalid`, `destination_inside_node_modules_or_git_is_invalid`, `every_default_excluded_name_is_an_invalid_destination` (every name in the list), `destination_with_valetignore_in_folder_or_ancestor_is_invalid`, `excluded_name_in_the_chosen_root_does_not_block_destinations_below_it`, `normal_destination_outside_root_still_plans` (names that only contain an excluded word are fine). Larger run: `integration_10k.rs` builds `.git`, `node_modules`, a `.valetignore` folder, dot folders, dot files and files with the simulated hidden attribute, and asserts that none of them is moved and that the excluded folders are reported as skipped.

**Partly met.**

- The exclusions apply to what is found below a scan root. A scan root is the user's choice, and a root that is itself called `node_modules`, or sits inside such a folder, is scanned (`excluded_names_in_root_path_are_an_explicit_opt_in`). The app checks only that a chosen folder exists (`save_config`). Nothing stops a user from choosing `C:\`; the exclusions would then apply to the folders below it by name, but not to the root.
- Exclusion is by directory name at any depth below a root, not by full path. A folder called `target` or `venv` anywhere below a root is skipped. `C:\Windows` is skipped only when it is reached from a root above it. A destination is judged the same way, by names after the shared part. So with a root such as `C:\Users\me\Downloads` a destination under `C:\Users\me\AppData` is refused, but with a root inside `AppData` every destination under that `AppData` is allowed (`excluded_name_in_the_chosen_root_does_not_block_destinations_below_it` covers a sibling folder).
- The destination check works on path text. It does not resolve junctions or symlinks, so a destination folder that is a junction into an excluded folder is not detected, and it does not check whether a destination folder is hidden or a reparse point. No test.
- A `.valetignore` in a folder above the scan root also blocks destinations: the lookup goes up to the file system root and has no exemption for the root's own ancestors. The scan itself does not look above the root. Read from the code; the test puts the marker in the destination's own path only.
- Destinations are checked when the preview is made. The executor does not check exclusions again, so a `.valetignore` created after the preview does not stop a planned operation.
- The list differs from PLAN 4.3: the code adds `venv` and `__pycache__`, and lists `Program Files` and `Program Files (x86)` as exact names instead of `Program Files*`.
- `includeHidden` (a rule option, `true` or `false`, default off) makes hidden files eligible for that rule. It does not make hidden folders scanned: hidden folders are skipped before any rule is looked at (read from the code; no test uses `includeHidden` with a hidden folder).

### Invariant 6. Skip files being written, locked files and cloud placeholders

**How the code meets it.**

- Being written: the planner skips a file whose last extension is `crdownload`, `part`, `tmp`, `download` or `partial` (any case), whose name starts with `~$`, or whose modification time is less than 10 seconds before the time of the plan (`RECENT_WRITE_WINDOW`).
- Locked: right before acting on a file, the executor asks `Platform::is_locked`. On Windows that opens the file for reading with sharing mode 0 and closes it at once, and treats errors 32 and 33 as "locked". It reads no data.
- Changed: the executor refuses a file whose size, modification time or type differs from the plan (`ChangedSincePreview`).
- Placeholders: see the cloud placeholder row in section 2. Short version: detected from file attributes without opening the file, checked before anything else touches the file, and never hashed.

**Tests.** `planner::tests::each_being_written_case_is_skipped`, `recently_modified_file_is_being_written`, `cloud_placeholder_file_and_dir_are_skipped`, `placeholder_wins_over_being_written_and_hidden`; `executor::tests::changed_size_or_mtime_fails_without_touching`, `locked_source_is_skipped`, `placeholder_is_never_hashed_or_moved`, `recover_placeholder_source_is_not_trashed_or_hashed`, `recover_placeholder_destination_with_source_gone_is_done_unhashed`. Windows only: `exclusive_handle_means_locked`, `offline_attribute_is_placeholder` (it sets the OFFLINE attribute on a normal file; it does not use a real OneDrive or Dropbox placeholder). End to end on Windows, in `windows_e2e.rs`: `offline_file_is_skipped_and_never_hashed` (the planner skips an OFFLINE file; a stale plan entry for it fails with `CloudPlaceholder` and the journal row has no `hash_before`) and `locked_file_fails_with_locked_and_is_untouched` (a file opened with sharing mode 0 fails with `Locked` and stays). Neither has been seen passing, since CI has not reached them. Attribute bits: `fs_win::tests::attribute_bits`.

**Partly met.**

- The "being written" test is a heuristic on name and modification time. PLAN 4.3 also asks to ignore files whose size changes between two checks; that is not implemented.
- The lock check is a snapshot. A file can become locked right after the check. Then the rename or copy fails with an `Io` error and the operation is marked `failed`; for a copy the temp file goes to the Recycle Bin. No test for that race.
- PLAN 4.3 says a locked file is retried on the next three watcher passes. Not implemented: the watcher does not apply anything yet (section 4).
- Real OneDrive and Dropbox placeholders have not been used in any test. Detection relies on the attributes OFFLINE, RECALL_ON_OPEN and RECALL_ON_DATA_ACCESS. A directory that carries one of them is skipped with everything below it.

### Invariant 7. No network except updater, license check and opt-in crash reports

There is no network code in the workspace to restrict. The updater, license check and crash reports do not exist yet (Phase 3 and 4).

What was checked:

- `grep -i` for `reqwest`, `hyper`, `ureq`, `isahc`, `surf`, `attohttpc`, `TcpStream`, `UdpSocket`, `fetch(`, `XMLHttpRequest`, `WebSocket`, `updater` and `sentry` in all `.rs`, `.toml`, `.ts`, `.tsx`, `.json` and `.html` files outside `node_modules`, `target` and `.claude` (and `Cargo.lock`) finds nothing at `a461d81`. The only HTTP address is `devUrl: http://localhost:5173` in `apps/desktop/src-tauri/tauri.conf.json`, the Vite dev server.
- `Cargo.lock` does contain `reqwest` and `hyper`. `cargo tree -p app --target x86_64-pc-windows-msvc -i reqwest` (and `-i hyper`) prints nothing, and neither does the same command for the Linux host: neither crate is built for those targets. With `--target all` the parent of `reqwest` is `tauri`, so it is built for some other target. No Valet crate depends on them directly.
- `tauri-plugin-single-instance`, added since the first version of this document, does not open a network connection on Windows: its code (read in the plugin source) uses a named mutex, `FindWindowW` and `SendMessageTimeoutW`. On Linux it uses D-Bus and on macOS a Unix socket, per the plugin documentation; those were not read or tested.
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
| Execute after preview | `dry_run` plans, keeps the plan in memory in the app state under a new UUID (`plan_id`) and returns the id with the list. `execute_operations(plan_id)` takes the batch lock, removes that plan from the state and runs exactly its operations with `executor::execute`. It does not plan again, so files that appeared after the preview are not touched, and rules or folder settings changed after the preview do not change what runs. The executor still re-checks each planned file (missing, changed, placeholder, locked). The plan is single use and is removed before execution starts: a second call, a call with another id, a call after a newer `dry_run` replaced the plan, or a call after the app restarted returns the error `preview_required` and touches nothing (no journal batch, no statistics row). A call with a wrong id leaves the stored plan in place. If the journal cannot be written during execution the plan is already gone and a new preview is needed. The plan has no expiry. The tray item "Organize Files Now" (`execute-now`) was removed; the tray's dry-run item only logs and stores no plan. In `DryRunView.tsx` the Execute button is disabled without a `plan_id`; the id is dropped after every execute attempt, and `preview_required` shows the text under the key `preview.expired`. | `commands::operations::tests::execute_without_matching_preview_touches_nothing` (no plan; plan under another id), `execute_runs_only_the_previewed_ops_and_consumes_the_plan` (a file added after the preview stays; second call fails), `state::tests::take_preview_without_stored_plan_requires_preview`, `take_preview_with_wrong_id_keeps_the_stored_plan`, `take_preview_is_single_use`, `newer_preview_replaces_older_one`. The tests call `execute_previewed`, not the Tauri command (it needs an app handle). The front end has no tests |
| Destination inside an excluded folder or under a `.valetignore` | Skipped as `invalidDestination`; nothing planned for that file. Excluded names are checked after the part of the path shared with the scan root; the `.valetignore` lookup covers the folder and every folder above it. See invariant 5 for the limits. | `planner::tests::destination_inside_windows_dir_is_invalid`, `destination_inside_appdata_is_invalid`, `destination_inside_node_modules_or_git_is_invalid`, `every_default_excluded_name_is_an_invalid_destination`, `destination_with_valetignore_in_folder_or_ancestor_is_invalid`, `excluded_name_in_the_chosen_root_does_not_block_destinations_below_it`, `normal_destination_outside_root_still_plans` |
| Name taken at preview time | The planner gives the file `name (2).ext`, then `(3)`, and so on. Two files going to the same name in one plan get different names, case ignored. | `planner::tests::existing_destination_file_gets_next_free_name`, `two_planned_ops_never_share_a_destination`, `reservation_is_case_insensitive`, `collision::tests::*` |
| Name taken at execution time (appeared after the preview) | `rename_no_replace` fails with `AlreadyExists`; the executor picks the next free name, records it in the journal, and retries. The existing file is untouched. | `collision_at_execution_time_gets_next_free_name` |
| Two rules match the same file | The first enabled rule with a `moveTo` or `copyTo` action is used. Later rules with such an action are listed as `conflict`. | `planner::tests::first_transfer_rule_wins_and_later_ones_conflict` |
| File already in the destination folder | Skipped as `alreadyInPlace` (case-insensitive folder comparison). | `planner::tests::file_already_in_destination_folder_is_already_in_place` |
| Destination folder missing | Created (`create_dir_all`) before the operation. It stays if the operation fails or is undone. | `executor::tests::undo_restores_byte_identical_tree` creates folders through execution; it compares files only |
| Action other than `moveTo` and `copyTo` (`tag`, `rename`, `quarantine`, anything else) | Not run. Reported once per rule as `unsupportedAction` when nothing was planned for the file. | `planner::tests::unsupported_actions_reported_once_per_rule_when_nothing_planned`, `unsupported_actions_reported_when_winner_destination_fails`, `tag_only_rule_before_mover_is_not_reported` |
| Destination template (`~`, `%VAR%`, `{year}`, `{month}`, `{ext}`) cannot be expanded | Skipped as `invalidDestination` (no home folder, unset variable, empty path; also the excluded-folder and `.valetignore` cases above) or `noExtension`. | `planner::tests::tilde_without_home_is_invalid_destination`, `missing_env_var_is_invalid_destination_and_set_var_expands`, `empty_or_missing_path_param_is_invalid_destination`, `ext_token_without_extension_is_no_extension` |
| Path is not valid Unicode | The planner lists it (names are converted with lossy conversion for matching). The executor refuses it with `InvalidPath` before any other check, because the journal stores paths as text and could not name the file exactly. | no test |

### Files that change, disappear, or are in use

| Situation | What Valet does | Tests |
|---|---|---|
| File changed after the preview (size, modification time, or no longer a regular file) | `failed` with `ChangedSincePreview`; nothing touched; no hash taken. A change during the copy is caught by the hash comparison (`VerifyFailed`, temp file to the Recycle Bin). A change in the short time between the check and a same-volume rename is not detected. | `changed_size_or_mtime_fails_without_touching`, `copy_with_hash_mismatch_trashes_temp_and_fails` |
| File missing at execution | `failed` with `SourceMissing`; the batch goes on. | `missing_source_fails_and_batch_continues` |
| File locked by another process | `failed` with `Locked`; nothing touched; no hash taken. Not retried (the watcher that would retry does not apply anything yet). | `locked_source_is_skipped`; Windows: `exclusive_handle_means_locked`, `windows_e2e.rs::locked_file_fails_with_locked_and_is_untouched` |
| File being written | Skipped at preview as `beingWritten`: extensions `.crdownload .part .tmp .download .partial`, names starting with `~$`, or modified less than 10 seconds before the plan. The 10 seconds are measured from the time of the plan, not of the execution. | `planner::tests::each_being_written_case_is_skipped`, `recently_modified_file_is_being_written` |
| Cloud placeholder (OneDrive, Dropbox, not downloaded) | Detected from attributes (`FILE_ATTRIBUTE_OFFLINE`, `RECALL_ON_OPEN`, `RECALL_ON_DATA_ACCESS`) with `GetFileAttributesW`, which reads the directory entry and does not open the file. The planner checks it first for every entry, before the symlink, hidden and "being written" checks, and a placeholder directory is not entered. The executor checks it again before it opens or hashes anything (`failed`, `CloudPlaceholder`). The lock probe also refuses to open a placeholder. Recovery and undo check it before every hash. Valet never opens, reads or hashes one. | `planner::tests::cloud_placeholder_file_and_dir_are_skipped`, `placeholder_wins_over_being_written_and_hidden`; `executor::tests::placeholder_is_never_hashed_or_moved`, `recover_placeholder_*`; `index::tests::index_skips_placeholders_and_exclusions`. Windows (simulated by the OFFLINE attribute): `offline_attribute_is_placeholder`, `windows_e2e.rs::offline_file_is_skipped_and_never_hashed` |
| Junction, symlink, other reparse point | Skipped as `reparsePoint`, not entered, not moved. On Windows only name-surrogate tags count (symlink, junction, mount point). OneDrive and Dropbox reparse points are not treated as links. If the tag cannot be read, the entry is treated as a link. The executor refuses a source that has become a reparse point (`ChangedSincePreview`). A scan root that is itself a link was not checked. | `planner::tests::symlinked_file_and_dir_are_reparse_points` (Linux symlinks), `platform_reparse_dir_is_not_descended`; `fs_win::tests::reparse_tag_name_surrogate_bit`; Windows (each skips itself if links cannot be created): `symlink_is_reparse_point`, `junction_is_reparse_point`, `plain_file_and_dir_are_not_reparse_points` |
| Hidden or system files | Names starting with `.` and files or folders with the Windows hidden or system attribute are skipped as `hidden`. A rule with `"options": {"includeHidden": true}` lets its hidden files be planned. Which rule decides: the winning rule, or the first matching rule when none wins. Hidden folders stay skipped. | `planner::tests::dot_file_is_hidden_and_dot_dir_not_descended`, `platform_hidden_file_and_dir_are_skipped`, `include_hidden_rule_plans_hidden_files`, `include_hidden_is_decided_by_the_winning_rule`, `dot_named_root_is_scanned`; Windows: `hidden_and_system_attributes` |
| Default exclusions and `.valetignore` | Excluded folders are skipped with everything below, one `excluded` entry for the folder. A `.valetignore` file (any content) in a folder, including a scan root, excludes the folder. A scan root named like an excluded folder is scanned; excluded folders below it are not. Destinations are checked too (row above). See invariant 5 for the limits. | `planner::tests::each_default_excluded_dir_is_skipped_once_and_not_descended`, `valetignore_excludes_its_dir_and_everything_below`, `root_containing_valetignore_is_excluded`, `excluded_names_in_root_path_are_an_explicit_opt_in`, `excluded_dir_chosen_as_root_is_scanned_in_any_root_order`, `nested_root_with_valetignore_is_excluded_in_any_root_order` |
| Overlapping or missing scan roots | Each file is planned once. A root that does not exist is reported as `unreadable` and the other roots are still scanned. | `planner::tests::overlapping_and_repeated_roots_plan_each_file_once`, `nonexistent_root_is_unreadable_and_others_still_scanned` |
| Long paths (over 260 characters) | Every file call in the planner and executor goes through `Platform::os_path`, which on Windows adds the `\\?\` prefix (`\\?\UNC\` for network paths), turns `/` into `\` and resolves `.` and `..` in the text. The attribute, lock, volume and rename helpers build the prefixed form themselves. The journal stores the path without the prefix. The `trash` crate is called with the unprefixed path. | `fs_win::tests::long_path_*` (string rules, any OS); Windows: `long_path_create_rename_read`, `long_path_rename_no_replace`, `trash_long_path_never_loses_the_file_silently` (accepts a refusal by the shell), and `windows_e2e.rs::long_paths_move_and_undo` (source and destination over 300 characters, through planner, executor and undo) |
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
| process killed at a random time during a 40-file batch (same-volume and copy-and-trash paths) | whatever the kill left | Every original exists once with its content, pre-existing files unchanged, no temp file, no unfinished operation or batch; undo then restores the original tree. | `crates/valet-core/tests/fault_injection.rs`: 100 iterations, 0 failures in one local run (Linux, fake platform, SIGKILL); see invariant 3 for what the run did and did not reach |

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
| Test that a second process cannot run recovery | `tauri-plugin-single-instance` is registered first (invariant 3). No test starts two processes. The lock is a named mutex without `Global\`, so it probably covers one Windows logon session only. | Not assigned |
| Stale previews | A preview stays valid until it is used, replaced or the app restarts. Exclusions, hidden state, "being written" and `.valetignore` are decided at preview time and not re-checked at execution. | Not assigned |
| Explained error for Controlled Folder Access | Not implemented; shows as `Io`. | Not assigned |
| Panic hook that marks `in_progress` operations | Not implemented. Recovery at the next start covers the same case. | Not assigned (PLAN 4.5) |
| Log scrubbing, log rotation, DEBUG switch in settings | `main.rs` sets the log level to Debug when `debug_assertions` is on (dev builds) and to Info otherwise. There is no `[profile]` override in the workspace, so a release build has them off; no release build was made here. DEBUG lines print full paths (watched folders, changed files, planned operations, notification text). The INFO, WARN and ERROR calls in `apps/desktop/src-tauri/src` log counts, batch ids, rule names and fixed text, not file paths or config contents (commit `b2b1969` removed the config path and contents). The error values some of them print (`{:?}` of OS errors, sqlx errors) were not checked, and no test checks any log line. `valet-core` and `valet-platform` write no log lines. The log plugin is used with its defaults: stdout and a log file, one file kept. Rotation of 5 files of 2 MB and a DEBUG toggle are not implemented. | Phase 3 (PLAN 4.5, tauri-platform "loguri cu rotație") |
| Opt-in crash reports with path scrubbing; updater; license check | None of the three exists. | Phase 3 and 4 |
| Crash tests beyond a SIGKILL on Linux | The kill test uses the fake platform. Not tested: a kill or power loss on Windows, a real Recycle Bin, lost writes, and (in the local run) the recovery branches that need the file at both source and destination. | Not assigned |
| Windows tests in CI | `windows.rs`, `windows_e2e.rs` and the 100-iteration fault injection are in `ci.yml` but the Clippy step fails before them (see "How to read this document"). | Not assigned |
| Real OneDrive and Dropbox, removable drives, network shares, Recycle Bin size limit, Controlled Folder Access | Not tested. Placeholders are simulated with the OFFLINE attribute; `can_trash` is tested only for a normal fixed drive. | Manual checklist for Phase 1 |
| Guarding the scan root (for example `C:\`) | Not implemented (see invariant 5). | Not assigned |
| Destination checks through junctions and symlinks | Not implemented; the destination check works on path text (see invariant 5). | Not assigned |
| `VALET_FORCE_CROSS_VOLUME` in shipped code | Read by `valet_platform::same_volume` in every build. It only forces the slower copy path (see invariant 3). | Not assigned |
