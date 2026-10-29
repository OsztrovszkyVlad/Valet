# Valet license service

Issues signed Valet Pro license files, answers the three activation calls of the desktop app, and turns merchant webhooks (Paddle or Stripe) into licenses sent by email. The contract is `docs/LICENSING.md`, section "License service".

Node 22 (20 or newer works), ES modules, built-in modules only (`fetch` is built in). No npm dependencies, no framework. Hosting: Fly.io, one machine, JSON store on a volume.

Suggested API base for `crates/valet-core/license/api.txt` (Vlad commits it): `https://license.valetapp.ro`.

## Files

| Path | What it does |
|---|---|
| `lib/signing.mjs` | Minisign signatures in the prehashed `ED` format (BLAKE2b-512, Ed25519), the public key line, verification, loading the key from the environment. |
| `lib/license.mjs` | License payload, activation token payload, the `{payload, signature}` envelope. |
| `lib/store.mjs` | Storage interface (`get`, `put`, `getOrder`, `putOrder`), JSON-file and in-memory implementations, support reset. |
| `lib/handler.mjs` | `createHandler({store, key, webhook, rateLimiter, …})` returns a plain `(req, res)` function for `node:http`. |
| `lib/webhook.mjs` | Paddle and Stripe signature checks, event interpretation, purchase and refund processing. |
| `lib/mail.mjs` | The license email (ro + en) and the Resend call. |
| `lib/ratelimit.mjs` | Token bucket per client address for the activation routes. |
| `lib/backup.mjs` | Daily dated copy of the store, last 30 kept. |
| `lib/config.mjs` | Reads and checks the server environment. |
| `lib/support.mjs` | Merchant order id → license id, record summary for the CLIs. |
| `server.mjs` | The server (local and on Fly). |
| `issue.mjs` | Issues a license by hand and registers it in the store. |
| `lookup.mjs` | Support: shows the license behind a merchant order id. |
| `reset.mjs` | Support: clears the active machines of a license (`--order` or `--license`). |
| `revoke.mjs` | Marks a license as revoked (`--order` or `--license`). |
| `keygen.mjs` | Prints a new key pair to stdout. |
| `Dockerfile`, `.dockerignore`, `fly.toml` | Fly.io image and app config. |
| `test/` | `node --test` suites; `test/fixtures/` holds webhook payloads in the documented shapes. |
| `all-tests.mjs` | Entry point so that `node --test services/license` runs the tests on Node 21+. |

The signing module is named `signing.mjs`, not `minisign.mjs`: the repository's edit hook protects every path containing "minisign" (it is there for key files).

## Environment

`server.mjs` refuses to start when a variable needed for the configured merchant is missing or malformed. The error lists the variable names, never the values.

| Variable | Needed | Content |
|---|---|---|
| `VALET_LICENSE_SECRET_KEY` | always | Ed25519 private key, PKCS#8 DER, base64. Secret. |
| `VALET_LICENSE_KEY_ID` | always | 8-byte minisign key id, 16 lowercase hex characters. Not secret. |
| `VALET_MERCHANT` | always | `paddle`, `stripe`, or `none` (activation API only, both webhook routes 404; local runs). |
| `VALET_WEBHOOK_SECRET` | paddle, stripe | Paddle notification destination secret, or Stripe endpoint signing secret. Secret. Two values separated by a comma (`new,old`) while rotating it. |
| `VALET_RESEND_API_KEY` | paddle, stripe | Resend API key with sending access. Secret. |
| `VALET_MAIL_FROM` | paddle, stripe | Sender, e.g. `Valet <licente@valetapp.ro>`, on a domain verified in Resend. |
| `VALET_MAIL_REPLY_TO` | optional | Reply-To address. |
| `VALET_PADDLE_API_KEY` | paddle | Paddle API key with read access to customers. Secret. |
| `VALET_PADDLE_PRICE_ID` | paddle | `pri_…` of the Valet Pro price. Other prices are ignored. |
| `VALET_PADDLE_SANDBOX` | optional | `1`: call `https://sandbox-api.paddle.com` instead of `https://api.paddle.com`. |
| `VALET_TRUST_FLY` | optional | `1`: rate limit by the `Fly-Client-IP` header (set in `fly.toml`). Without it the socket address is used. Set it only behind Fly's proxy. |
| `VALET_RATE_LIMIT_PER_MIN` | optional | Activation requests per minute per client, also the burst size. Default 30. |
| `VALET_LICENSE_STORE` | optional | Store path when `--store` is not given. |
| `PORT` | optional | Port when `--port` is not given. |

Stripe needs no price variable: Checkout events don't carry line items, so the service issues a license only for sessions with `metadata.product = "valet-pro"` (see "Merchant setup").

Creating the production signing key (Vlad):

1. `node services/license/keygen.mjs`. It prints to the terminal and writes nothing to disk.
2. Put `VALET_LICENSE_SECRET_KEY` and `VALET_LICENSE_KEY_ID` into Fly secrets (below). Do not save the secret in a file, a chat, a ticket or the repository.
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

The three activation calls are `POST` with `Content-Type: application/json` and a body of at most 4096 bytes. Responses are JSON with `Cache-Control: no-store`. Webhooks and `/healthz` are described further down.

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
| 405 | `{error:"method_not_allowed"}`, `Allow: POST` | any method other than POST on the three paths and the webhook route |
| 413 | `{error:"too_large"}` | body over 4096 bytes |
| 415 | `{error:"unsupported_media_type"}` | content type other than `application/json` |
| 429 | `{error:"rate_limited"}`, `Retry-After: <seconds>` | more than `VALET_RATE_LIMIT_PER_MIN` calls from one client address (the app treats it like a network error and keeps its token) |
| 500 | `{error:"internal"}` | store failure |

### Behaviour where the contract is silent

- **Activate on a machine that already holds an activation** returns a fresh token and does not use a slot.
- **Validate on a machine not in the active set** behaves like activate: it takes a free slot and returns a token, or answers 409 `too_many` when `max_activations` other machines are active. 404 always means "this license id is not known", so the message the app shows stays accurate. This happens only after a server-side change (support reset, restored store); the app's rule "404, 409 and 410 on validate remove the token" stays correct.
- **Deactivate** is idempotent: 204 whether the machine was active or not, also for a revoked license. It answers 404 `unknown` only when the license id is not in the store. The app removes its local token and license whatever the answer.
- **Revoked license**: activate and validate answer 410. Deactivate still answers 204.
- Activations are counted per license from the store record (`max_activations` is copied from the license at issue time, 3 by default). Concurrent requests for one license are serialized inside one process.
- 404 for an unknown path uses `not_found`, not `unknown`. The app removes its token on any 404, so a wrong `api.txt` URL would remove tokens; the app keeps the license file and the next correct check restores Pro.

### Store records

The JSON file is `{"version": 1, "licenses": {"<license_id>": record}, "orders": {"<order key>": order}}` (`orders` appears with the first webhook purchase; older files without it load as before):

```json
{
  "license_id": "…",
  "max_activations": 3,
  "issued_at": 1793000000,
  "updates_until": 1824536000,
  "revoked": false,
  "emailed_at": 1793000002,
  "machines": {
    "<machine>": { "first_seen": 1793000100, "last_seen": 1795000000, "active": true, "deactivated_at": null }
  }
}
```

Orders map merchant ids to licenses: keys `paddle:txn_…`, `stripe:cs_…`, `stripe:pi_…`; value `{"license_id": "…", "created_at": …}`, or `{"license_id": null, "refunded_at": …}` when a refund arrived before any purchase event for that order.

Nothing is ever removed. Deactivation sets `active: false` and `deactivated_at`. A reset does the same for every active machine and appends `{at, cleared}` to `resets`. Revocation sets `revoked: true`, `revoked_at` and, from a webhook, `revoked_reason: "refund"`. `emailed_at` is set once Resend accepted the license email. The email address is not stored on the server, nor is any manual email → license list kept anywhere; the license file and the merchant have the address.

Writes go to a temp file in the same directory, are fsynced, then renamed over the store file. Each write holds `store.json.lock` (created exclusively) while it re-reads the file, changes one entry and renames, so the server and a support CLI run at the same time don't lose each other's entries; a lock older than 30 s (a crashed writer) is taken over. When a CLI runs as root (`fly ssh console`), the file it writes gets the owner of `/data`, so the server can still read it. A store file that can't be parsed is never overwritten.

## Merchant webhook

`POST /v1/webhook/paddle` or `POST /v1/webhook/stripe`. Only the route of `VALET_MERCHANT` exists; the other answers 404 `not_found`.

Order of work for every delivery:

1. Read the raw body, at most 64 KB (413 above that).
2. Check the signature on the raw bytes, before parsing: HMAC-SHA256, hex, constant-time compare, timestamp within 5 minutes of the server clock (both directions). Any of several `h1=` / `v1=` values may match (the merchant's secret rotation); `VALET_WEBHOOK_SECRET` may hold two secrets (ours). Failure: 400 `bad_signature`, nothing changes.
3. Parse the JSON (400 `bad_request`) and interpret the event. A signed event of a handled type with a missing or malformed field answers 400 `bad_event`, so it shows as failed in the merchant's dashboard.
4. Purchase, refund, or ignore. Deliveries are processed one at a time; at most 4 are accepted at once (503 `busy`, `Retry-After: 10` above that; the merchant retries).

| | Paddle Billing | Stripe |
|---|---|---|
| Signature header | `Paddle-Signature: ts=<unix>;h1=<hex>[;h1=…]`, HMAC over `<ts>:<body>` | `Stripe-Signature: t=<unix>,v1=<hex>[,v1=…][,v0=…]`, HMAC over `<t>.<body>`; `v0` ignored |
| Purchase | `transaction.completed`, `data.status` = `completed`, an item with `price.id` (or `price_id`) = `VALET_PADDLE_PRICE_ID` | `checkout.session.completed` or `checkout.session.async_payment_succeeded`, `payment_status` = `paid`, `metadata.product` = `valet-pro` |
| Buyer email | `GET {api}/customers/{data.customer_id}`, `Authorization: Bearer VALET_PADDLE_API_KEY`, field `data.email` | `data.object.customer_details.email` |
| Order keys | `paddle:<data.id>` (`txn_…`) | `stripe:<session id>` (`cs_…`) and `stripe:<payment_intent>` (`pi_…`) |
| Refund | `adjustment.created` / `adjustment.updated` with `data.action` = `refund` and `data.status` = `approved`; found by `data.transaction_id` | `charge.refunded` with `amount_refunded` > 0; found by `data.object.payment_intent` |
| Ignored (200) | other event types, other prices, adjustments that aren't approved refunds | other event types, unpaid sessions, sessions without `metadata.product = "valet-pro"` |

Purchase:

1. If an order key is already mapped and the license has `emailed_at`: 200, nothing else (duplicate delivery). If it is mapped to a revoked license or to a refund-only entry: 200, nothing sent.
2. Read the buyer email (Paddle: customer API; a failure answers 502 `merchant_api`, nothing is issued).
3. Not mapped yet: issue the license (365 days, 3 activations), register it, map every order key to it. Mapped already (an earlier email failed): sign the same license again (same id and dates; the signature is deterministic, so the file is byte-identical) and add any missing order key.
4. Send the email through Resend: the `.valet-license` file attached as `valet-pro.valet-license` (base64), a plain-text body in Romanian then English saying where to import it (Valet → Setări → Valet Pro → Am o licență), the activation limit, the updates date, and that support needs the order number from the receipt, never the license file. If Resend fails or times out (8 s): 502 `email_failed`; the merchant redelivers and step 3 sends the same license.
5. Set `emailed_at`, answer 200.

Refund: the mapped license gets `revoked: true`; activate and validate answer 410 from then on. A refund for an unknown order records a refund-only entry, so a purchase event that arrives later for the same order issues nothing. Repeated refunds answer 200 and change nothing.

Paddle chargebacks (`action = chargeback`) and Stripe disputes are not handled: the contract names refunds only (open decision for Vlad).

### Merchant setup

Paddle (sandbox first: sandbox-vendors.paddle.com; then the same in the live account with new secrets):

1. Catalog → Products: "Valet Pro" with a one-time price. Its `pri_…` id → `VALET_PADDLE_PRICE_ID`.
2. Developer tools → Notifications → New destination: URL `https://license.valetapp.ro/v1/webhook/paddle`, type webhook, events `transaction.completed`, `adjustment.created`, `adjustment.updated`. Its secret key → `VALET_WEBHOOK_SECRET`.
3. Developer tools → Authentication → API key with read permission on customers → `VALET_PADDLE_API_KEY`.
4. Sandbox: `VALET_PADDLE_SANDBOX=1`. Live: remove it (`fly secrets unset VALET_PADDLE_SANDBOX`).

Stripe (test mode first, then live with new secrets):

1. Product "Valet Pro" with a one-time price; a Payment Link for it (or Checkout Sessions created in code).
2. Required: metadata `product` = `valet-pro` on the Payment Link (copied to the Checkout Sessions it creates) or on each Checkout Session. Without it the service ignores the purchase.
3. Developers → Webhooks → Add endpoint `https://license.valetapp.ro/v1/webhook/stripe`, events `checkout.session.completed`, `checkout.session.async_payment_succeeded`, `charge.refunded`. Its signing secret → `VALET_WEBHOOK_SECRET`.
4. Optional: success URL `https://valetapp.ro/multumim?order={CHECKOUT_SESSION_ID}` so the thank-you page can show the buyer the `cs_…` order id that support asks for.

Resend:

1. Domains → Add `valetapp.ro` (or a subdomain), add the DNS records Resend shows (DKIM TXT, SPF TXT and MX for its sending subdomain), wait for "Verified". A DMARC record on the domain helps delivery.
2. API Keys → create a key with "Sending access" limited to that domain → `VALET_RESEND_API_KEY`.
3. `VALET_MAIL_FROM="Valet <licente@valetapp.ro>"`.

### Verify in sandbox (the gate before selling)

The webhook docs (developer.paddle.com, docs.stripe.com) could not be reached from the environment where this was written; the shapes come from knowledge of the documented APIs and are mirrored in `test/fixtures/`. Selling opens only after Vlad has run this list against the chosen merchant's sandbox and every item holds (docs/LICENSING.md, decision 10). A wrong assumption shows up as 400 `bad_signature` / `bad_event` in the merchant's delivery log, or as an `ignored_*` outcome in `fly logs`.

Paddle:

- [ ] A sandbox checkout of Valet Pro: Paddle's notification log shows `transaction.completed` delivered with 200; `fly logs` shows `"event":"transaction.completed","outcome":"issued"`.
- [ ] Header `Paddle-Signature` is `ts=…;h1=…` and the destination's secret key, used as is, verifies it (no `bad_signature`).
- [ ] Payload: `event_type`; `data.id` is `txn_` + lowercase letters and digits; `data.status` = `completed`; `data.customer_id` is `ctm_…`; `data.items[].price.id` (or `data.items[].price_id`) equals `VALET_PADDLE_PRICE_ID`.
- [ ] `GET https://sandbox-api.paddle.com/customers/{ctm_…}` with the API key answers 200 with `data.email` (an `outcome":"customer_lookup_failed` log means permission or base URL is wrong).
- [ ] The email arrives from `VALET_MAIL_FROM`, with `valet-pro.valet-license` attached; importing it in Valet (Setări → Valet Pro → Am o licență) activates Pro with `api.txt` pointing at this server.
- [ ] "Replay" of the same notification in Paddle: 200, `outcome":"duplicate"`, no second email.
- [ ] A full refund of that transaction: `adjustment.created` (usually `status: pending_approval`, ignored), then `adjustment.updated` with `action: refund`, `status: approved`, `transaction_id: txn_…` → `outcome":"revoked"`. "Verifică licența acum" in Valet then shows the license as revoked.
- [ ] Note what a partial refund looks like (`type: partial`); it revokes too.
- [ ] The buyer's receipt or invoice email shows the `txn_…` id (support needs it). If it shows only an invoice number, tell the PM: `lookup.mjs` needs another key.

Stripe:

- [ ] A test-mode payment through the Payment Link (card 4242 4242 4242 4242): `checkout.session.completed` delivered with 200, `outcome":"issued"`. (`stripe trigger` events carry no `metadata.product` and are ignored by design.)
- [ ] Header `Stripe-Signature` is `t=…,v1=…` and the endpoint's `whsec_…` secret verifies it.
- [ ] Payload: `type`; `data.object.id` is `cs_…`; `payment_status` = `paid`; `metadata.product` = `valet-pro` (copied from the Payment Link, also with Managed Payments); `customer_details.email` set; `payment_intent` is a `pi_…` string, not null.
- [ ] Email and import as for Paddle.
- [ ] "Resend" of the event from the dashboard: 200, `duplicate`, no second email.
- [ ] Refund in the dashboard: `charge.refunded` with `data.object.payment_intent` equal to the session's `pi_…` and `amount_refunded` > 0 → `revoked`.
- [ ] If a delayed payment method is enabled: `checkout.session.completed` with `payment_status: unpaid` is ignored and `checkout.session.async_payment_succeeded` issues.
- [ ] Which id the buyer sees (receipt email, thank-you page). Stripe receipts show a receipt number, not `cs_…`/`pi_…`; use the success-URL step above or tell the PM.

Both:

- [ ] Resend failure path: set a wrong `VALET_RESEND_API_KEY` (`fly secrets set`), buy once: 502 `email_failed` in the merchant log; restore the key; the merchant's retry (or a manual resend) sends the email; `lookup.mjs --order …` shows one license with `emailed`.
- [ ] `fly logs` contains no email address, license id, order id or IP address.

## Rate limiting

In the process, per client address: a token bucket of `VALET_RATE_LIMIT_PER_MIN` (default 30) requests, refilled at that rate per minute, for `/v1/activate`, `/v1/validate` and `/v1/deactivate` together. Over the limit: 429 `rate_limited` with `Retry-After` in seconds. The address comes from `Fly-Client-IP` when `VALET_TRUST_FLY=1`, else from the socket. Webhooks are not rate limited (signature first, and at most 4 in flight). `/healthz` is not limited.

Addresses are never logged or written anywhere. The bucket map is keyed by an HMAC of the address with a key generated at process start, holds at most 10,000 entries, and drops refilled buckets once a minute (then the least recently used ones if still full). A restart empties it.

## Health check

`GET /healthz` (and `HEAD`) → 200, body `ok`, no data. Fly checks it every 30 s.

## Backups

At start and every 24 h the server copies the store to `/data/backups/store-YYYY-MM-DD.json` (UTC date). A day's file is never overwritten: the copy is written to a temp file, fsynced and hard-linked to its name, which fails if it exists. A store that doesn't parse is not copied. The newest 30 dated copies are kept; older ones are deleted, only inside `backups/` and only names matching `store-YYYY-MM-DD.json` exactly. These are server copies of the store; the app's never-delete rule is about user files.

The copies sit on the same volume: they cover a bad write or a bad deploy, not the loss of the volume. Fly also takes daily volume snapshots (`fly volumes snapshots list <volume id>`). Pull a copy to your own machine every week or two:

```sh
fly ssh sftp get /data/backups/store-2026-10-29.json ./store-2026-10-29.json
```

The file holds license ids and machine hashes (no emails); keep it out of the repository and out of public places.

Restoring a copy (only after a bad write; licenses issued after the copy's date are lost from the server and their buyers' activations would answer 404). Run it as the `node` user so the server can read the result; the rename is atomic and the server reads the file on every request:

```sh
fly ssh console
su node -s /bin/sh -c 'cp /data/backups/store-2026-10-29.json /data/store.json.restored && mv /data/store.json.restored /data/store.json'
exit
```

## Deploy on Fly.io

One machine with one volume. The JSON store is a file on that volume and is not shared: never scale to 2 machines (`fly scale count 1` is the only valid count) and never create a second volume for this app. Deploys replace the machine in place, so expect a few seconds without service; the app retries later and webhooks are redelivered.

From `services/license/`:

```sh
fly auth login
fly launch --no-deploy --copy-config --name valet-license --region fra
fly volumes create license_data --size 1 --region fra
fly secrets import          # paste NAME=value lines, then Ctrl-D (nothing lands in shell history or on disk)
#   VALET_LICENSE_SECRET_KEY=…
#   VALET_LICENSE_KEY_ID=…
#   VALET_MERCHANT=paddle            (or stripe)
#   VALET_WEBHOOK_SECRET=…
#   VALET_RESEND_API_KEY=…
#   VALET_MAIL_FROM=Valet <licente@valetapp.ro>
#   VALET_PADDLE_API_KEY=…           (paddle only)
#   VALET_PADDLE_PRICE_ID=pri_…      (paddle only)
#   VALET_PADDLE_SANDBOX=1           (paddle sandbox only)
fly deploy --ha=false
fly scale count 1
fly certs add license.valetapp.ro
fly certs show license.valetapp.ro   # shows the DNS records to add
```

DNS at the `valetapp.ro` registrar: `license` CNAME `valet-license.fly.dev` (or the A and AAAA records from `fly ips list`), plus the validation record `fly certs show` asks for. Then:

```sh
curl -i https://license.valetapp.ro/healthz      # 200 ok
fly logs                                          # one JSON line per request
```

Image and config: `Dockerfile` (`node:22-alpine`, copies only `package.json`, the CLIs, `server.mjs` and `lib/`; runs `node server.mjs --host 0.0.0.0 --port 8080 --store /data/store.json --run-as 1000:1000`). The volume is mounted owned by root, so the process starts as root only to hand `/data` to the image's `node` user (uid/gid 1000) and switches to it before opening the store or the port. `fly.toml`: app `valet-license`, region `fra` (EU, large capacity for volumes; activation calls are rare and webhooks come from the merchant's servers, so Bucharest's lower latency doesn't matter), volume `license_data` at `/data`, `internal_port = 8080`, `force_https`, `auto_stop_machines = "off"`, `min_machines_running = 1`, health check on `/healthz`, `VALET_TRUST_FLY=1`.

Rolling back a bad deploy: `fly releases --image`, then `fly deploy --image <previous image ref>`. The store format is additive (older code keeps the `orders` entries it doesn't read), so an older image runs on a newer store. Changing a secret restarts the machine.

Switching merchant later: set the other merchant's variables and `VALET_MERCHANT`, then pass the sandbox list above for it before selling through it.

## Support

Support runs on public GitHub Issues. The license id alone is enough to activate, so:

- Ask for the order number from the payment receipt (Paddle `txn_…`, Stripe `cs_…` or `pi_…`). Never ask for, and never post, the license code or the license file. If a user posts one, edit it out of the issue (reissuing a leaked license is not automated yet).
- Never post the output of `lookup.mjs` in an issue; summarize it ("2 of 3 activations in use, last seen …").

```sh
fly ssh console -C "node /app/lookup.mjs --order txn_01… --store /data/store.json"
fly ssh console -C "node /app/reset.mjs  --order txn_01… --store /data/store.json"   # old PC gone: frees its slots
fly ssh console -C "node /app/revoke.mjs --order cs_test_… --store /data/store.json" # e.g. a refund made outside the webhook
```

`lookup.mjs` prints the license id, issue date, `updates_until`, revoked flag, whether the email went out, the active machine count and each machine's first/last-seen dates (no machine ids, no email: the store has none). `reset.mjs` and `revoke.mjs` take `--order` or `--license` (not both). A buyer who lost the license email is not handled by a command yet: the server has no email address, and a redelivered purchase sends nothing once `emailed_at` is set (open decision).

## Logs

One JSON line per request: `route` (one of the known paths, or `other`), `method`, `status`, `ms`, `error` (an exception class name) for 500, and for webhooks `event` (a known event type, or `other`) and `outcome` (`issued`, `resent`, `duplicate`, `revoked`, `email_failed`, `customer_lookup_failed`, `ignored_<reason>`, `bad_signature`, …). Backups log `{"backup": "created" | "exists" | "no_store" | "invalid_store" | "failed", "removed": n}`. No license ids, machine ids, order ids, emails, IP addresses, request bodies or unknown paths.

## Running locally

```sh
# a throwaway key for local runs (never the production key)
eval "$(node services/license/keygen.mjs | grep '^VALET_LICENSE_' | sed 's/^/export /')"
export VALET_MERCHANT=none

node services/license/issue.mjs --email t@example.com --store ./tmp-store.json > t.valet-license
node services/license/server.mjs --port 8787 --store ./tmp-store.json

LICENSE_ID=$(node -e "console.log(JSON.parse(JSON.parse(require('fs').readFileSync('t.valet-license','utf8')).payload).license_id)")
curl -s -X POST localhost:8787/v1/activate -H 'Content-Type: application/json' \
  -d "{\"license_id\":\"$LICENSE_ID\",\"machine\":\"0123456789abcdef0123456789abcdef\",\"app_version\":\"0.2.0\"}"

node services/license/reset.mjs --license "$LICENSE_ID" --store ./tmp-store.json
node services/license/revoke.mjs --license "$LICENSE_ID" --store ./tmp-store.json
```

`server.mjs` listens on `127.0.0.1` by default (`--host` to change it) and speaks plain HTTP. It also writes `backups/` next to the store file.

Tests: `node --test` in this directory (or `npm test`), or `node --test services/license` from the repository root. They generate their own keys, use temp directories, and replace `fetch` for Resend and Paddle (no network).

## Still missing

- **Sandbox pass** of the chosen merchant (list above), then `checkout.txt` (Vlad).
- **Renewals**: issuing a license with a later `updates_until` for an existing `license_id`.
- **Chargebacks and disputes**: not revoked automatically (the contract covers refunds).
- **Lost license email and leaked licenses**: no command to send the same license again or to replace a leaked one under the same order.

## Security notes

- The signing key, webhook secret, Resend key and Paddle API key live only in Fly secrets (and the signing key in Vlad's offline backup). They are never committed, never logged; configuration errors name variables, never values.
- Webhook bodies are trusted only after the HMAC check; ids from them are checked against strict patterns before use (the Paddle customer id goes into a URL path).
- TLS is terminated by Fly (`force_https`). The app only calls an `https://` base URL from `crates/valet-core/license/api.txt`.
- The server holds license ids, `max_activations`, dates, machine ids (a one-way hash of the Windows MachineGuid) and merchant order ids. No names, emails, paths or IP addresses. Buyer emails pass through memory for one Resend request.
- The TEST key in `test/signing.test.mjs` is public by design. Its public key line must never be placed in `pubkey.txt`. The secrets in the tests are fixed test strings without the real prefixes.
