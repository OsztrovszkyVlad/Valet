# Licensing and plan limits (contract)

This is the contract between `valet-core` (verification and limits), the desktop app (commands, online check), the license service (`services/license/`) and the UI. Product rules come from `docs/PLAN.md` 3.1 and 3.2. Change this file first, then the code.

## Plans

| | Free | Trial (14 days) and Pro |
|---|---|---|
| Presets (all 8) | yes, not counted | yes |
| Own rules enabled at once | 3 | unlimited |
| Watched folders active at once | 2 | unlimited |
| History shown | batches from the last 7 days | all |
| Undo | the most recent batch only | any batch in history |
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
- Watched folders: the first 2 in `inbox_paths` order stay active. The rest stay in the config and are reported as `inactive_paths`. The watcher and planning skip them, as for unavailable folders. Approvals are kept.
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

- Timeouts: 10 s. A network error keeps the current token (grace up to 60 days).
- 404 and 410 remove the token: Free or Trial applies, and the license file stays so the user sees why. 409 on validate behaves the same.
- Nothing else is sent: no paths, file names, rule contents or counts.

## License service (`services/license/`)

- Node ≥ 20, built-in modules only (`node:http`, `node:crypto`, `node:test`). It signs with Ed25519 through `crypto.sign(null, blake2b512(data), key)` in minisign's prehashed format.
- Activation storage: one small interface (`get`, `put`). The repo ships a JSON-file implementation for tests and local runs. The hosting decision (Vlad) adds the real one.
- `node services/license/issue.mjs --email a@b.c [--days 365]` issues a license file by hand (for test purchases and support).
- The merchant webhook (Paddle, Polar or Stripe, Vlad's decision) is not part of this contract yet.
- The secret key is read from the `VALET_LICENSE_SECRET_KEY` environment variable on the server. It is never committed. Tests use a key generated at test time. Committed fixtures are signed with a key labelled TEST whose public half is never shipped.
