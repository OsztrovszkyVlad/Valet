---
name: windows-file-safety
description: Reference for safe file operations on Windows in Valet: journal protocol, move/copy/collision/locked/placeholder/long-path policies, default exclusions, watcher rules. Load when writing or reviewing any code that touches files.
---

# Windows file safety (Valet)

## Journal protocol (valet-core `journal.rs`)
- Table `operations`: id, batch_id, op_type (move|copy|rename|trash|tag), source_path, dest_path, size, hash_before, hash_after, state (planned|in_progress|done|failed|rolled_back), error, created_at, finished_at. SQLite WAL.
- Order per operation: insert `planned` → update `in_progress` → perform → verify → update `done` (or `failed` with error). One transaction per state change, never one transaction per batch.
- Startup recovery: for every `in_progress`: if dest exists and matches size/hash → `done`; else if source exists → `failed` and nothing to clean; if both exist (copy phase interrupted) → send the partial dest to Recycle Bin, mark `failed`. Write a `recovery` entry in history.
- Undo: inverse ops in reverse order; before each, verify dest hash equals hash_after; mismatch → return `NeedsConfirmation` and let the UI ask.

## Operation policy
| Case | Rule |
|---|---|
| same volume move | `fs::rename` (atomic) |
| cross volume move | copy to temp name in dest dir → fsync → verify blake3 → rename to final name → trash original |
| collision | never overwrite; `name (2).ext`, `name (3).ext`; implemented once in `collision.rs`, property-tested injective |
| locked file | detect via platform fn `is_locked` (open with no sharing); skip with reason `Locked`; retry on next 3 watcher passes |
| being written | skip if extension in {crdownload, part, tmp, download, partial} or name starts with `~$`, or mtime < 10 s ago, or size changed between two reads 2 s apart |
| cloud placeholder | platform fn `is_cloud_placeholder` via FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS / RECALL_ON_OPEN / OFFLINE; skip with reason `CloudPlaceholder`; never open or hash it |
| reparse point | never traverse, never move (walkdir follow_links=false AND attribute check) |
| long paths | all platform calls go through `to_long_path()` adding `\\?\`; test with 300-char paths |
| hidden/system | excluded unless the rule opts in (`includeHidden: true`) |
| removal | `trash::delete` only. `std::fs::remove_*` is forbidden in product code (hook enforced) |

## Default exclusions
`%WINDIR%`, `Program Files`, `Program Files (x86)`, `ProgramData`, `%LOCALAPPDATA%`, `%APPDATA%`, `$Recycle.Bin`, `System Volume Information`, `.git`, `node_modules`, `target`, `.venv`, `venv`, `__pycache__`, any dir containing `.valetignore`. Scopes are allowlists chosen by the user; exclusions apply inside scopes.

## Watcher
`notify` + `notify-debouncer-mini`, 2 s debounce. On overflow error → full rescan of that root. Auto-apply only after `settings.auto_apply_enabled && first_preview_approved`. Pause the watcher while a preview is open. Single app instance.

## Logging and privacy
INFO logs contain no full paths (basename truncated to 40 chars + 8-char blake3 of the parent dir). DEBUG may contain paths, off by default. Crash reports opt-in, scrubbed.
