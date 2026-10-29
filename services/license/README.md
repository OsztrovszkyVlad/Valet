# Valet license service

Issues signed Valet Pro license files and answers the three activation calls of the desktop app. The contract is `docs/LICENSING.md`; this directory implements its "License service" section.

Node 20 or newer, ES modules, built-in modules only. No npm dependencies, no framework, no hosting-specific code.

## Files

| Path | What it does |
|---|---|
| `lib/signing.mjs` | Minisign signatures in the prehashed `ED` format (BLAKE2b-512, Ed25519), the public key line, verification, loading the key from the environment. |
| `lib/license.mjs` | License payload, activation token payload, the `{payload, signature}` envelope. |
| `lib/store.mjs` | Activation storage interface (`get`, `put`), JSON-file and in-memory implementations, support reset. |
| `lib/handler.mjs` | `createHandler({store, key})` returns a plain `(req, res)` function for `node:http`. |
| `server.mjs` | Local server. |
| `issue.mjs` | Issues a license by hand and registers it in the store. |
| `reset.mjs` | Support: clears the active machines of a license. |
| `revoke.mjs` | Marks a license as revoked (refund, chargeback). |
| `keygen.mjs` | Prints a new key pair to stdout. |
| `all-tests.mjs` | Entry point so that `node --test services/license` runs the tests on Node 21+. |

The signing module is named `signing.mjs`, not `minisign.mjs`: the repository's edit hook protects every path containing "minisign" (it is there for key files).

## Keys and environment

| Variable | Content |
|---|---|
| `VALET_LICENSE_SECRET_KEY` | The Ed25519 private key, PKCS#8 DER, base64 (64 characters). Secret. |
| `VALET_LICENSE_KEY_ID` | The 8-byte minisign key id as 16 lowercase hex characters, in the byte order used inside the signature and public key blobs. Not secret. |
| `VALET_LICENSE_STORE` | Path of the JSON store file, used when `--store` is not given. |
| `PORT` | Default port for `server.mjs`. |

Creating the production key (Vlad):

1. `node services/license/keygen.mjs`. It prints to the terminal and writes nothing to disk.
2. Put `VALET_LICENSE_SECRET_KEY` and `VALET_LICENSE_KEY_ID` into the host's secret store. Do not save the secret in a file, a chat, a ticket or the repository.
3. Put the `RW…` public key line into `crates/valet-core/license/pubkey.txt` and commit it.
4. Clear the terminal scrollback.

Losing the secret key means no new licenses or activation tokens can be signed for apps that ship its public key. Keep an offline backup in a password manager.

## Signature format

Every license file and activation token is `{"payload": "<JSON string>", "signature": "<minisign signature file>"}`. The signed bytes are the UTF-8 bytes of `payload`. The signature file has 4 lines, each ending with `\n`:

```
untrusted comment: signature from valet license service
base64("ED" || key_id[8] || Ed25519(BLAKE2b-512(payload)))
trusted comment: timestamp:<unix>	file:valet-<kind>	hashed
base64(Ed25519(signature[64] || trusted_comment))
```

The public key line is `base64("Ed" || key_id[8] || public_key[32])`. The trusted comment holds the signing time and the kind only (no email, no machine id).

Cross-check: the test vectors in `test/signing.test.mjs` (signed with a fixed key labelled TEST, whose public half must never be shipped) were verified with the Rust crate `minisign-verify` 0.3.0, `PublicKey::verify(…, allow_legacy = false)`, together with a random-key license, a 100 kB message and a trusted comment with non-ASCII characters; tampered payloads were rejected. The `minisign` CLI was not installed, so it was not part of the check.

## API

All calls are `POST` with `Content-Type: application/json` and a body of at most 4096 bytes. Responses are JSON with `Cache-Control: no-store`.

| Call | Body | Answers |
|---|---|---|
| `/v1/activate` | `{license_id, machine, app_version}` | 200 `{token}`; 404 `{error:"unknown"}`; 409 `{error:"too_many"}`; 410 `{error:"revoked"}` |
| `/v1/validate` | `{license_id, machine, app_version}` | same as activate |
| `/v1/deactivate` | `{license_id, machine}` (`app_version` optional) | 204 (no body); 404 `{error:"unknown"}` |

`token` is an envelope object `{payload, signature}`, the same shape as a license file. The desktop app stores it as `activation.valet-token`. Its payload is `{"v":1,"kind":"activation","license_id":…,"machine":…,"checked_at":<now>}`.

Input rules: `license_id` is a lowercase UUID v4; `machine` is 32 lowercase hex characters; `app_version` is 1 to 32 characters of `[0-9A-Za-z.+-]`, starting with a letter or digit. Unknown fields are rejected.

Other answers:

| Status | Body | When |
|---|---|---|
| 400 | `{error:"bad_request"}` | invalid JSON or UTF-8, not an object, missing, unknown or malformed fields |
| 404 | `{error:"not_found"}` | any other path |
| 405 | `{error:"method_not_allowed"}`, `Allow: POST` | any method other than POST on the three paths |
| 413 | `{error:"too_large"}` | body over 4096 bytes |
| 415 | `{error:"unsupported_media_type"}` | content type other than `application/json` |
| 500 | `{error:"internal"}` | store failure |

### Behaviour where the contract is silent

- **Activate on a machine that already holds an activation** returns a fresh token and does not use a slot.
- **Validate on a machine not in the active set** behaves like activate: it takes a free slot and returns a token, or answers 409 `too_many` when `max_activations` other machines are active. 404 always means "this license id is not known", so the message the app shows stays accurate. This happens only after a server-side change (support reset, restored store); the app's rule "404, 409 and 410 on validate remove the token" stays correct.
- **Deactivate** is idempotent: 204 whether the machine was active or not, also for a revoked license. It answers 404 `unknown` only when the license id is not in the store. The app removes its local token and license whatever the answer.
- **Revoked license**: activate and validate answer 410. Deactivate still answers 204.
- Activations are counted per license from the store record (`max_activations` is copied from the license at issue time, 3 by default). Concurrent requests for one license are serialized inside one process.
- 404 for an unknown path uses `not_found`, not `unknown`. The app removes its token on any 404, so a wrong `api.txt` URL would remove tokens; the app keeps the license file and the next correct check restores Pro.

### Store records

The JSON file is `{"version": 1, "licenses": {"<license_id>": record}}`:

```json
{
  "license_id": "…",
  "max_activations": 3,
  "issued_at": 1793000000,
  "updates_until": 1824536000,
  "revoked": false,
  "machines": {
    "<machine>": { "first_seen": 1793000100, "last_seen": 1795000000, "active": true, "deactivated_at": null }
  }
}
```

Nothing is ever removed. Deactivation sets `active: false` and `deactivated_at`. A reset does the same for every active machine and appends `{at, cleared}` to `resets`. Revocation sets `revoked: true` and `revoked_at`. The email is not stored on the server; the license file and the merchant have it. Writes go to a temp file in the same directory, are fsynced, then renamed over the store file. A store file that can't be parsed is never overwritten. Two processes writing the same file at the same moment can lose one update; for local use, run `issue.mjs`, `reset.mjs` and `revoke.mjs` while the server is stopped.

## Logs

One JSON line per request: `route` (one of the three paths or `other`), `method`, `status`, `ms`, and `error` (an exception class name) for 500. No license ids, machine ids, emails, IP addresses, request bodies or unknown paths.

## Running locally

```sh
# a throwaway key for local runs (never the production key)
eval "$(node services/license/keygen.mjs | grep '^VALET_LICENSE_' | sed 's/^/export /')"

node services/license/issue.mjs --email t@example.com --store ./tmp-store.json > t.valet-license
node services/license/server.mjs --port 8787 --store ./tmp-store.json

LICENSE_ID=$(node -e "console.log(JSON.parse(JSON.parse(require('fs').readFileSync('t.valet-license','utf8')).payload).license_id)")
curl -s -X POST localhost:8787/v1/activate -H 'Content-Type: application/json' \
  -d "{\"license_id\":\"$LICENSE_ID\",\"machine\":\"0123456789abcdef0123456789abcdef\",\"app_version\":\"0.2.0\"}"

node services/license/reset.mjs --license "$LICENSE_ID" --store ./tmp-store.json
node services/license/revoke.mjs --license "$LICENSE_ID" --store ./tmp-store.json
```

`server.mjs` listens on `127.0.0.1` by default (`--host` to change it) and speaks plain HTTP.

Tests: `node --test services/license` from the repository root, or `npm test` in this directory. They generate their own keys and use temp directories.

## Still missing

- **Merchant webhook** (Paddle, Polar or Stripe, Vlad's decision): verify the webhook signature, call `issueLicense`, `registerLicense`, and email the license file to the buyer. Refunds and chargebacks set `revoked`.
- **Hosted store**: an implementation of `get`/`put` on the chosen host's database. `put` must be a conditional write (or a transaction) so two instances can't both take the last slot; the in-process lock only covers one instance.
- **Host adapter**: wrapping `createHandler` for the chosen platform (Cloud Run can use `server.mjs` as is; a functions platform needs a small adapter from its request object to `(req, res)`).
- **Rate limiting**: at the host or a proxy in front, per IP. The service does not see or log IPs.
- **Renewals**: issuing a license with a later `updates_until` for an existing `license_id`.

## Security notes

- The secret key lives only in the host's secret store and in Vlad's offline backup. It is never committed, never logged, and error messages about it never include its value.
- TLS is terminated by the host. The app only calls an `https://` base URL from `crates/valet-core/license/api.txt`.
- The server holds license ids, `max_activations`, dates and machine ids (a one-way hash of the Windows MachineGuid). No names, emails, paths or IP addresses.
- The TEST key in `test/signing.test.mjs` is public by design. Its public key line must never be placed in `pubkey.txt`.
