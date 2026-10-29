// In-process token bucket per client, for the activation routes.
//
// The client address is never stored as such: the map key is an HMAC of it with a key
// generated at process start, so the map holds no IP addresses and nothing survives a
// restart. Nothing here is logged.
//
// Memory stays bounded: at most once a minute (or when the map reaches `maxEntries`),
// buckets that have refilled completely are removed (a full bucket behaves exactly like
// a missing one). If the map is still at `maxEntries` after that, the buckets that were
// used least recently are removed.

import crypto from 'node:crypto';

const SWEEP_EVERY_MS = 60_000;

/**
 * @param {object} [options]
 * @param {number} [options.capacity] requests allowed in a burst (default 30)
 * @param {number} [options.perMinute] refill rate (default 30 per minute)
 * @param {number} [options.maxEntries] bound on the number of buckets kept (default 10000)
 * @param {() => number} [options.nowMs] clock in milliseconds
 */
export function createRateLimiter({ capacity = 30, perMinute = 30, maxEntries = 10_000, nowMs = Date.now } = {}) {
  if (!(capacity > 0) || !(perMinute > 0) || !(maxEntries > 0)) {
    throw new Error('rate limiter: capacity, perMinute and maxEntries must be positive');
  }
  const refillPerMs = perMinute / 60_000;
  const salt = crypto.randomBytes(32);
  const buckets = new Map();
  let lastSweep = nowMs();

  const keyOf = (client) => crypto.createHmac('sha256', salt).update(String(client)).digest('base64url');

  const level = (bucket, t) => Math.min(capacity, bucket.tokens + (t - bucket.at) * refillPerMs);

  function sweep(t) {
    lastSweep = t;
    for (const [k, b] of buckets) {
      if (level(b, t) >= capacity) buckets.delete(k);
    }
    // Map iteration follows insertion order; take() re-inserts on every use, so the
    // first keys are the least recently used.
    for (const k of buckets.keys()) {
      if (buckets.size < maxEntries) break;
      buckets.delete(k);
    }
  }

  return {
    /** Takes one token. Returns { ok: true } or { ok: false, retryAfter } (whole seconds). */
    take(client) {
      const t = nowMs();
      if (t - lastSweep >= SWEEP_EVERY_MS || buckets.size >= maxEntries) sweep(t);
      const k = keyOf(client);
      const existing = buckets.get(k);
      const tokens = existing ? level(existing, t) : capacity;
      if (tokens < 1) {
        // Keep the bucket where it is: a client that keeps getting 429 is not "recent use".
        return { ok: false, retryAfter: Math.max(1, Math.ceil((1 - tokens) / refillPerMs / 1000)) };
      }
      buckets.delete(k);
      buckets.set(k, { tokens: tokens - 1, at: t });
      return { ok: true };
    },
    /** Number of buckets held (tests). */
    size() {
      return buckets.size;
    },
  };
}
