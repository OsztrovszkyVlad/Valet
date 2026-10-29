# Licensing and plan limits (contract)

This is the contract between `valet-core` (verification and limits), the desktop app (commands, online check), the license service (`services/license/`) and the UI. Product rules come from `docs/PLAN.md` 3.1 and 3.2. Change this file first, then the code.

## Plans

| | Free | Trial (14 days) and Pro |
|---|---|---|
| Presets (all 8) | yes, not counted | yes |
| Own rules enabled at once | 3 | unlimited |
| Watched folders active at once | 2 | unlimited |
| History shown | batches from the last 7 days | all |
| Undo | the most recent `apply` batch only (undo and recovery batches don't count) | any batch in history |
| Export and import of rules | no | yes |
| Condition and action types | all | all |

- "Own rule" = a rule without a non-empty `options.presetId`.
- Limits are enforced in the backend (core policy, called by the Tauri commands). The UI only mirrors them.
- Nothing is deleted when a limit applies. The journal is never trimmed for plan reasons.

### Trial

- Starts the first time a build with this module opens the database: `meta` key `trial_started_at` (Unix seconds, UTC). Upgraders from 0.1 start their trial on that first start.
- Lasts 14 × 86,400 s. No card, no network.
- Deleting the database resets it. Accepted (PLAN 3.2: no DRM beyond this).

### Falling back to Free (trial ended, license gone or not valid for this build)

- Own rules: if more than 3 are enabled, the first 3 in evaluation order stay enabled. The rest are disabled with `options.planDisabled = true`. `meta` key `plan_choice_pending = 1` asks the UI to let the user pick which 3 stay active.
- Watched folders: the first 2 in `inbox_paths` order stay active. The rest stay in the config and are reported as `inactive_paths`. The watcher and planning skip them, as for unavailable folders. Approvals are kept. The user picks which 2 stay active (PLAN 3.1) in the same chooser as the rules. The choice is saved by reordering `inbox_paths`, so the picked folders come first; no extra state.
- Back to Pro or Trial: rules with `planDisabled = true` are enabled again and the flag is removed. Rules the user disabled stay disabled. All folders become active again.
- Enabling an own rule, adding one enabled, or adding a folder over the limit on Free fails with error code `plan.limit` (`{ kind: "rules" | "folders", limit }`). It is not silently disabled.

## License file

A UTF-8 JSON file, extension `.valet-license`:

```json
{
  "payload": "{\"v\":1,\"kind\":\"license\",\"license_id\":\"…\",\"product\":\"valet-pro\",\"issued_at\":1793000000,\"updates_until\":1824536000,\"max_activations\":3,\"email\":\"…\"}",
  "signature": "untrusted comment: …\nRW…\ntrusted comment: …\n…"
}
```

- `payload` is a string. The signed bytes are exactly its UTF-8 bytes.
- `signature` is a minisign signature file, prehashed (`ED`, BLAKE2b-512). The app verifies it with `minisign-verify` and `allow_legacy = false`.
- `license_id`: UUID v4, the only identifier sent over the network.
- `updates_until` = `issued_at` + 365 days, or later after a renewal. A build is covered when its build date (`VALET_BUILD_DATE`, Unix seconds, set at compile time; it falls back to the compile time) is ≤ `updates_until`. A license that doesn't cover this build gives Free plus a message naming the date. Older builds keep working.
- Before installing an update, the app compares the update's `pub_date` (from the updater manifest) with `updates_until`. If Pro is active and the new build isn't covered, the confirmation says the new version will run as Free and names the date. The user can still install it.
- `email` stays on the machine. It is shown in Settings so the user knows whose license it is.
- The file is stored in the app data folder as `license.valet-license` after a successful import.

## Activation token

The server returns a token in the same envelope (`payload` + `signature`), signed with the same key:

```json
{"v":1,"kind":"activation","license_id":"…","machine":"…","checked_at":1793000000}
```

- `machine` = the first 32 hex characters of `blake3("valet-machine-v1:" + MachineGuid)`. MachineGuid is read from `HKLM\SOFTWARE\Microsoft\Cryptography`. Neither the raw GUID nor the computer name leaves the machine.
- Stored next to the license as `activation.valet-token`.

## Plan resolution (pure function in core)

Inputs: license file (optional), activation token (optional), machine id, `now`, build date, `trial_started_at`.

1. A valid license, `kind = license`, `product = valet-pro`, covering this build, and a token with valid signature, `kind = activation`, same `license_id`, same `machine`, and `now - checked_at ≤ 60 days` → **Pro**.
2. Otherwise, `now < trial_started_at + 14 days` → **Trial** (with its end date).
3. Otherwise → **Free**.

Every reason for not getting Pro is reported as a code: `license.missing`, `license.invalid_signature`, `license.wrong_product`, `license.not_covering_build`, `activation.missing`, `activation.other_machine`, `activation.expired`, `license.not_configured`. The UI shows a sentence for each one.

- A token with `checked_at` more than 1 day in the future counts as invalid. That case comes only from clock tampering.
- `checked_at` 31 days old is still Pro (grace). 61 days old is Free, with nothing lost; the next successful check restores Pro.

## Online check (desktop app only, `reqwest`)

Base URL and public key come from `crates/valet-core/license/` files committed by Vlad: `pubkey.txt` (minisign public key, base64 line) and `api.txt` (HTTPS base URL). An empty or missing file means licensing is not configured: no network calls, Pro can't be activated, and Trial and Free work normally.

| Call | When | Body | Answers |
|---|---|---|---|
| `POST {api}/v1/activate` | the user imports a license | `{license_id, machine, app_version}` | 200 `{token}`; 404 `unknown`; 409 `too_many` (`max_activations` other machines); 410 `revoked` |
| `POST {api}/v1/validate` | at startup and every 24 h, when `now - checked_at ≥ 30 days` | same | 200 `{token}`; 404, 409, 410 as above |
| `POST {api}/v1/deactivate` | the user removes the license from Settings | `{license_id, machine}` | 204 |

- `token` in a 200 answer is the envelope object `{payload, signature}` (not a string). The app stores it as JSON in `activation.valet-token`.
- Validate for a machine that isn't active behaves like activate: it takes a free slot or answers 409. 404 always means the license id is unknown. Deactivate answers 204 even when the machine wasn't active.
- Timeouts: 10 s. A network error keeps the current token (grace up to 60 days).
- 404 and 410 remove the token: Free or Trial applies, and the license file stays so the user sees why. 409 on validate behaves the same.
- Nothing else is sent: no paths, file names, rule contents or counts.

## License service (`services/license/`)

- Node ≥ 20, built-in modules only (`node:http`, `node:crypto`, `node:test`). It signs with Ed25519 through `crypto.sign(null, blake2b512(data), key)` in minisign's prehashed format.
- Activation storage: one small interface (`get`, `put`). The repo ships a JSON-file implementation for tests and local runs. The hosting decision (Vlad) adds the real one.
- `node services/license/issue.mjs --email a@b.c [--days 365]` issues a license file by hand (for test purchases and support).
- `node services/license/reset.mjs --license <id>` clears the active machines of a license (support: the old PC is gone and can't deactivate). History is kept.
- The merchant webhook (Paddle, Polar or Stripe, Vlad's decision) is not part of this contract yet.
- The secret key is read from the `VALET_LICENSE_SECRET_KEY` environment variable on the server. It is never committed. Tests use a key generated at test time. Committed fixtures are signed with a key labelled TEST whose public half is never shipped.

## App commands (Tauri, `commands/license.rs` unless noted)

Errors are the usual `"code: detail"` strings or `{code, …}` objects that `parseCommandError` reads. Every command that changes the plan returns the new `PlanView`.

`PlanView` (snake_case):

```
{
  plan: {kind:"free"} | {kind:"trial", ends_at} | {kind:"pro", license_id, email, updates_until},
  reasons: string[],                // contract codes above
  limits: {own_rules: number|null, folders: number|null, history_days: number|null,
           undo_latest_only: boolean, export_import: boolean},
  licensing_configured: boolean,    // pubkey.txt and api.txt both set
  checkout_available: boolean,      // checkout.txt set
  choice_pending: boolean,          // show the "pick what stays active" dialog
  license: {license_id, email, updates_until} | null,   // also when not Pro, to explain why
  trial_ends_at: number | null,
  last_check_at: number | null,     // checked_at of the stored token
  packaged: boolean                 // Microsoft Store (MSIX) build
}
```

| Command | Args | Does | Errors |
|---|---|---|---|
| `get_plan_status` | — | resolves the plan, applies a pending downgrade or upgrade (below) | — |
| `import_license_file` | — | native open dialog (`.valet-license`), then like `import_license_text`; `null` on cancel | as below |
| `import_license_text` | `text` | verifies, activates online, stores both files | `license.not_configured`, `license.invalid_signature`, `license.wrong_product`, `license.not_covering_build`, `license.network`, `license.unknown`, `license.too_many`, `license.revoked` |
| `check_license_now` | — | validate call, even before 30 days | `license.network`, then as above |
| `remove_license` | — | deactivate (best effort; a network error doesn't block), then moves both files to `license-removed/<unix>-…` (never deleted) | — |
| `apply_plan_choice` | `rule_ids: string[]`, `folders: string[]` | Free only: these own rules (≤ 3) enabled, the other own rules plan-disabled; these folders (≤ 2) moved to the front of `inbox_paths`; clears `choice_pending` | `plan.limit`, `plan.notFree` |
| `export_rules_file` | — | Pro/Trial: native save dialog, writes the export; refuses an existing path. Returns the written path, or `null` on cancel | `plan.proOnly`, `export.exists`, `export.io` |
| `import_rules_file` | — | Pro/Trial: native open dialog, imports as new rules (auto-apply waits for a preview, as for any new rule). Returns the imported `Rule[]`, or `null` on cancel | `plan.proOnly`, `import.*` |
| `open_external` (`platform.rs`) | `target: "checkout" \| "startup_settings"` | opens the checkout URL (`checkout.txt`) or `ms-settings:startupapps`; nothing else can be opened | `open.unavailable` |

Changes to existing commands:
- `upsert_rule`: `check_rule_change` (→ `plan.limit`); the UI can't add or change `presetId` or `planDisabled` (the stored values are kept).
- `save_config`: `check_folders` (→ `plan.limit`).
- `get_config`: adds `inactive_paths` (Free, beyond the first 2).
- `list_batches`: the response becomes `{batches: BatchInfo[], hidden_older: boolean, latest_apply_batch_id: string | null}`. Free hides batches older than 7 days. `latest_apply_batch_id` is the newest batch of kind `apply` (even if already undone), from the whole journal, not only the visible part.
- `undo_batch`: Free refuses anything but `latest_apply_batch_id` (`plan.undoLatestOnly`). Undoing it once leaves it the latest apply batch, so older batches stay locked: no chain of undos on Free.
- `check_for_update`: adds `pub_date` and `covered_by_license: boolean | null` (null when not Pro).
- `configure_auto_start`, `check_for_update`, `install_update`: in the packaged build they return `autostart.packaged` / `update.packaged`.
- `AppSettings` gains `auto_update_check: boolean` (default true). Off: no automatic checks; "Check now" still works.

Plan transitions run at startup, hourly, and after any command above that changes the plan: Free with more than 3 enabled own rules or more than 2 folders → `apply_free_limits`, `choice_pending = true`; Trial or Pro → `restore_after_upgrade`, `choice_pending = false`. Rules are written with `db.upsert_rule` (keeps `version`, so approvals survive the round trip).

`checkout.txt` sits next to `pubkey.txt` and `api.txt`: the HTTPS checkout URL. Empty means buying isn't open yet.
