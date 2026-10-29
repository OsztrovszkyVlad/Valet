// HTTP handler for the license service (docs/LICENSING.md, "Online check" and
// "License service"). A plain (req, res) function for node:http.
//
//   POST /v1/activate    {license_id, machine, app_version}  200 {token} | 404 | 409 | 410
//   POST /v1/validate    {license_id, machine, app_version}  200 {token} | 404 | 409 | 410
//   POST /v1/deactivate  {license_id, machine}               204 | 404
//   POST /v1/webhook/paddle | /v1/webhook/stripe             only the configured merchant; the other is 404
//   GET  /healthz                                            200 "ok"
//
// `token` is an envelope object { payload, signature }, the same shape as a license file.
// The three activation routes are rate limited per client address when a limiter is
// given (429 with Retry-After). Webhooks are not rate limited (the signature is checked
// first) but at most `maxConcurrentWebhooks` are handled at once (503 with Retry-After).
// Logs carry route, status, duration, and for webhooks the event type and outcome:
// no license ids, machine ids, merchant ids, emails or IPs.

import { activeMachineCount, updateRecord } from './store.mjs';
import { issueActivationToken, nowSeconds, UUID_V4, MACHINE_ID } from './license.mjs';
import { createWebhookProcessor, MERCHANTS } from './webhook.mjs';

export const MAX_BODY_BYTES = 4096;
export const MAX_WEBHOOK_BODY_BYTES = 64 * 1024;
const APP_VERSION = /^[0-9A-Za-z][0-9A-Za-z.+-]{0,31}$/;

class HttpError extends Error {
  constructor(status, code, headers = {}) {
    super(code);
    this.status = status;
    this.code = code;
    this.headers = headers;
  }
}

function defaultLog(entry) {
  console.log(JSON.stringify({ time: new Date().toISOString(), ...entry }));
}

function send(res, status, body, headers = {}) {
  const base = { 'Cache-Control': 'no-store', ...headers };
  if (body === undefined) {
    res.writeHead(status, base);
    res.end();
    return;
  }
  const text = JSON.stringify(body);
  res.writeHead(status, {
    ...base,
    'Content-Type': 'application/json; charset=utf-8',
    'Content-Length': Buffer.byteLength(text),
  });
  res.end(text);
}

function readBody(req, limit) {
  return new Promise((resolve, reject) => {
    const declared = req.headers['content-length'];
    if (declared !== undefined && Number(declared) > limit) {
      req.resume();
      reject(new HttpError(413, 'too_large', { Connection: 'close' }));
      return;
    }
    const chunks = [];
    let size = 0;
    let settled = false;
    req.on('data', (chunk) => {
      if (settled) return;
      size += chunk.length;
      if (size > limit) {
        settled = true;
        reject(new HttpError(413, 'too_large', { Connection: 'close' }));
        return;
      }
      chunks.push(chunk);
    });
    req.on('end', () => {
      if (settled) return;
      settled = true;
      resolve(Buffer.concat(chunks));
    });
    req.on('error', (err) => {
      if (settled) return;
      settled = true;
      reject(err);
    });
  });
}

function isJsonContentType(value) {
  if (typeof value !== 'string') return false;
  return value.split(';')[0].trim().toLowerCase() === 'application/json';
}

function parseJsonObject(buffer) {
  let text;
  try {
    text = new TextDecoder('utf-8', { fatal: true }).decode(buffer);
  } catch {
    throw new HttpError(400, 'bad_request');
  }
  let value;
  try {
    value = JSON.parse(text);
  } catch {
    throw new HttpError(400, 'bad_request');
  }
  if (value === null || typeof value !== 'object' || Array.isArray(value)) {
    throw new HttpError(400, 'bad_request');
  }
  return value;
}

/** Strict field check. Unknown fields are rejected. */
function parseRequestFields(body, { appVersion }) {
  const allowed = new Set(['license_id', 'machine', 'app_version']);
  for (const k of Object.keys(body)) {
    if (!allowed.has(k)) throw new HttpError(400, 'bad_request');
  }
  const { license_id: licenseId, machine, app_version: version } = body;
  if (typeof licenseId !== 'string' || !UUID_V4.test(licenseId)) {
    throw new HttpError(400, 'bad_request');
  }
  if (typeof machine !== 'string' || !MACHINE_ID.test(machine)) {
    throw new HttpError(400, 'bad_request');
  }
  const required = appVersion === 'required';
  if (version === undefined ? required : typeof version !== 'string' || !APP_VERSION.test(version)) {
    throw new HttpError(400, 'bad_request');
  }
  return { licenseId, machine };
}

/** Client address for rate limiting: Fly's header when trusted, else the socket peer. */
function clientAddress(req, trustFly) {
  const fly = req.headers['fly-client-ip'];
  if (trustFly && typeof fly === 'string' && fly) return fly;
  return req.socket?.remoteAddress ?? 'unknown';
}

/**
 * Creates the request handler.
 * @param {object} options
 * @param {{get: Function, put: Function, update?: Function}} options.store activation storage
 * @param {{keyId: Buffer, privateKey: import('node:crypto').KeyObject}} options.key signing key
 * @param {() => number} [options.now] clock in Unix seconds
 * @param {(entry: object) => void} [options.log] log sink
 * @param {object|null} [options.webhook] merchant settings for createWebhookProcessor
 *   ({merchant, secrets, mail, paddle?, fetch?}); null or absent: both webhook routes are 404
 * @param {{take: Function}|null} [options.rateLimiter] limiter for the activation routes
 * @param {boolean} [options.trustFly] read the client address from Fly-Client-IP
 * @param {number} [options.maxConcurrentWebhooks] webhook deliveries handled at once
 */
export function createHandler({
  store,
  key,
  now = nowSeconds,
  log = defaultLog,
  webhook = null,
  rateLimiter = null,
  trustFly = false,
  maxConcurrentWebhooks = 4,
}) {
  if (!store || typeof store.get !== 'function' || typeof store.put !== 'function') {
    throw new Error('store must implement get and put');
  }
  if (!key?.privateKey || !key?.keyId) throw new Error('signing key missing');

  // Serializes read-modify-write per license inside this process.
  const locks = new Map();
  async function withLock(id, fn) {
    const previous = locks.get(id) ?? Promise.resolve();
    const current = previous.then(fn, fn);
    const tail = current.catch(() => {});
    locks.set(id, tail);
    try {
      return await current;
    } finally {
      if (locks.get(id) === tail) locks.delete(id);
    }
  }

  // Activate and validate share the same rules. Validate for a machine that is not
  // in the active set re-activates it when a slot is free, else answers 409.
  async function activate({ licenseId, machine }) {
    return withLock(licenseId, async () => {
      const t = now();
      const refused = await updateRecord(store, licenseId, (record) => {
        if (!record) return [404, { error: 'unknown' }];
        if (record.revoked) return [410, { error: 'revoked' }];
        record.machines ??= {};
        const entry = record.machines[machine];
        if (entry?.active) {
          entry.last_seen = t;
        } else {
          if (activeMachineCount(record) >= record.max_activations) {
            return [409, { error: 'too_many' }];
          }
          record.machines[machine] = {
            first_seen: entry?.first_seen ?? t,
            last_seen: t,
            active: true,
            deactivated_at: entry?.deactivated_at ?? null,
          };
        }
        return null;
      });
      if (refused) return refused;
      const token = issueActivationToken({ licenseId, machine, checkedAt: t }, key);
      return [200, { token }];
    });
  }

  async function deactivate({ licenseId, machine }) {
    return withLock(licenseId, async () => {
      const t = now();
      return updateRecord(store, licenseId, (record) => {
        if (!record) return [404, { error: 'unknown' }];
        const entry = record.machines?.[machine];
        if (entry?.active) {
          entry.active = false;
          entry.last_seen = t;
          entry.deactivated_at = t;
        }
        return [204, undefined];
      });
    });
  }

  const routes = {
    '/v1/activate': { appVersion: 'required', run: activate },
    '/v1/validate': { appVersion: 'required', run: activate },
    '/v1/deactivate': { appVersion: 'optional', run: deactivate },
  };

  const processWebhook = webhook
    ? createWebhookProcessor({ ...webhook, store, key, now, withLicenseLock: withLock })
    : null;
  const webhookPath = webhook ? `/v1/webhook/${webhook.merchant}` : null;
  const knownPaths = new Set([...Object.keys(routes), ...MERCHANTS.map((m) => `/v1/webhook/${m}`), '/healthz']);
  let webhooksInFlight = 0;

  async function handleWebhook(req, res, entry) {
    if (req.method !== 'POST') {
      req.resume();
      throw new HttpError(405, 'method_not_allowed', { Allow: 'POST' });
    }
    if (webhooksInFlight >= maxConcurrentWebhooks) {
      req.resume();
      throw new HttpError(503, 'busy', { 'Retry-After': '10' });
    }
    webhooksInFlight += 1;
    try {
      const body = await readBody(req, MAX_WEBHOOK_BODY_BYTES);
      const result = await processWebhook({ body, headers: req.headers });
      entry.outcome = result.outcome;
      if (result.type) entry.event = result.type;
      entry.status = result.status;
      send(res, result.status, result.body ?? { ok: true });
    } finally {
      webhooksInFlight -= 1;
    }
  }

  return async function handler(req, res) {
    const started = Date.now();
    const pathname = String(req.url ?? '').split('?')[0];
    const route = Object.hasOwn(routes, pathname) ? routes[pathname] : null;
    const entry = { route: knownPaths.has(pathname) ? pathname : 'other', status: 500 };
    try {
      if (pathname === '/healthz') {
        req.resume();
        if (req.method !== 'GET' && req.method !== 'HEAD') {
          throw new HttpError(405, 'method_not_allowed', { Allow: 'GET, HEAD' });
        }
        entry.status = 200;
        res.writeHead(200, { 'Content-Type': 'text/plain; charset=utf-8', 'Cache-Control': 'no-store', 'Content-Length': 2 });
        res.end(req.method === 'HEAD' ? undefined : 'ok');
        return;
      }
      if (webhookPath !== null && pathname === webhookPath) {
        entry.method = req.method;
        await handleWebhook(req, res, entry);
        return;
      }
      if (!route) {
        entry.status = 404;
        req.resume();
        send(res, 404, { error: 'not_found' });
        return;
      }
      entry.method = req.method;
      if (req.method !== 'POST') {
        entry.status = 405;
        req.resume();
        send(res, 405, { error: 'method_not_allowed' }, { Allow: 'POST' });
        return;
      }
      if (rateLimiter) {
        const verdict = rateLimiter.take(clientAddress(req, trustFly));
        if (!verdict.ok) {
          req.resume();
          throw new HttpError(429, 'rate_limited', { 'Retry-After': String(verdict.retryAfter) });
        }
      }
      if (!isJsonContentType(req.headers['content-type'])) {
        req.resume();
        throw new HttpError(415, 'unsupported_media_type');
      }
      const body = parseJsonObject(await readBody(req, MAX_BODY_BYTES));
      const fields = parseRequestFields(body, route);
      const [code, payload] = await route.run(fields);
      entry.status = code;
      send(res, code, payload);
    } catch (err) {
      if (err instanceof HttpError) {
        entry.status = err.status;
        send(res, err.status, { error: err.code }, err.headers);
      } else {
        entry.status = 500;
        entry.error = err?.name ?? 'Error';
        if (!res.headersSent) send(res, 500, { error: 'internal' });
      }
    } finally {
      log({ ...entry, ms: Date.now() - started });
    }
  };
}
