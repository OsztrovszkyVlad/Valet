// Activation storage.
//
// Interface (all methods return promises, so a hosted database can implement it):
//   get(licenseId) -> record | null
//   put(licenseId, record) -> void      (insert or replace the record for that id)
//   getOrder(orderKey) -> order | null   (merchant order map, used by the webhook)
//   putOrder(orderKey, order) -> void
//   update(licenseId, fn) -> fn's result (optional: read-modify-write of one record under
//                                         the store's write lock; see updateRecord)
//
// There is no delete. Revocation is `revoked: true`; deactivation sets `active: false`
// on the machine entry and keeps its history.
//
// Record:
//   {
//     license_id, max_activations, issued_at, updates_until,
//     revoked: false, revoked_at?,
//     revoked_reason?: "refund" | "dispute" | "reissued",
//     disputed?: true,                   (chargeback or dispute; cleared only by undispute.mjs)
//     resets?: [{ at, cleared }],
//     machines: { "<32 hex>": { first_seen, last_seen, active, deactivated_at } },
//     emailed_at?: number,              (set once Resend accepted the first license email)
//     history?: [{ at, action, … }]     (revoke, dispute, dispute_reversed, dispute_lost,
//                                         undispute, resend, reissued, issued)
//   }
// The email is not stored here; the license file and the merchant hold it.
//
// Order keys are "<merchant>:<merchant id>" (paddle:txn_…, stripe:cs_…, stripe:pi_…).
// Order: { license_id, created_at, order_keys?, customer_id?, history? } once a license
// was issued for it, or { license_id: null, refunded_at | disputed_at } when a refund or
// dispute arrived before the purchase. `order_keys` lists every key of the same purchase
// (Stripe: the session and the payment intent); `customer_id` is the Paddle `ctm_…` id
// (never the email); `history` holds { at, action: "reissue", from, to } entries.
// All of these fields are additive: entries written by older versions lack them.

import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';

const LOCK_WAIT_MS = 10_000;
const LOCK_STALE_MS = 30_000;

/** Builds the store record for a freshly issued license payload. */
export function newLicenseRecord(payload) {
  return {
    license_id: payload.license_id,
    max_activations: payload.max_activations,
    issued_at: payload.issued_at,
    updates_until: payload.updates_until,
    revoked: false,
    machines: {},
  };
}

/** Registers an issued license. Refuses to replace an existing record. */
export async function registerLicense(store, payload) {
  if (await store.get(payload.license_id)) {
    throw new Error('license id already registered');
  }
  await store.put(payload.license_id, newLicenseRecord(payload));
}

/** Appends an entry to the record's history (created on first use). */
export function addHistory(record, entry) {
  record.history = [...(record.history ?? []), entry];
}

/**
 * Read-modify-write of one license record. `fn(record | null)` may change the record in
 * place; it is written back only when its JSON changed. Returns what `fn` returns.
 * Uses `store.update` when the store has it (the JSON store does this under its file
 * lock, so a support CLI and the server don't overwrite each other's change), else
 * `get` + `put`. `fn` must not call the network: it may run while the file is locked.
 */
export async function updateRecord(store, id, fn) {
  if (typeof store.update === 'function') return store.update(id, fn);
  const record = await store.get(id);
  const before = JSON.stringify(record);
  const result = await fn(record);
  if (record !== null && JSON.stringify(record) !== before) await store.put(id, record);
  return result;
}

/** Number of machines currently holding an activation. */
export function activeMachineCount(record) {
  return Object.values(record.machines ?? {}).filter((m) => m.active).length;
}

/**
 * Support reset: marks every active machine of the record as deactivated at `now`.
 * Machine entries (first/last seen) stay as history. Returns the number cleared.
 */
export function resetActiveMachines(record, now) {
  let cleared = 0;
  for (const entry of Object.values(record.machines ?? {})) {
    if (entry.active) {
      entry.active = false;
      entry.deactivated_at = now;
      cleared += 1;
    }
  }
  if (cleared > 0) {
    record.resets = [...(record.resets ?? []), { at: now, cleared }];
  }
  return cleared;
}

/** In-memory store, for tests. */
export function createMemoryStore() {
  const map = new Map();
  const orders = new Map();
  return {
    async get(id) {
      return map.has(id) ? structuredClone(map.get(id)) : null;
    },
    async put(id, record) {
      map.set(id, structuredClone(record));
    },
    async getOrder(key) {
      return orders.has(key) ? structuredClone(orders.get(key)) : null;
    },
    async putOrder(key, order) {
      orders.set(key, structuredClone(order));
    },
    async update(id, fn) {
      const record = map.has(id) ? structuredClone(map.get(id)) : null;
      const before = JSON.stringify(record);
      const result = await fn(record);
      if (record !== null && JSON.stringify(record) !== before) map.set(id, structuredClone(record));
      return result;
    },
  };
}

/**
 * JSON-file store. The file holds
 * { "version": 1, "licenses": { "<id>": record }, "orders": { "<order key>": order } }
 * ("orders" is absent until the first webhook purchase).
 * Writes go to a temp file in the same directory, are fsynced, then renamed over the
 * original. Writes inside one process are serialized; across processes (the server and
 * a support CLI run over `fly ssh console`) each write holds `<file>.lock` (created
 * with O_EXCL) while it re-reads the file, changes one entry and renames. A lock older
 * than 30 s is taken over (a crashed writer). Reads need no lock: rename is atomic.
 * When run as root, the new file gets the owner of its directory, so a CLI run as root
 * doesn't leave a file the server (non-root) can't read.
 * A file that can't be parsed is never overwritten: every call fails until it is fixed.
 */
export function createJsonFileStore(filePath) {
  const file = path.resolve(filePath);
  let chain = Promise.resolve();

  async function load() {
    let text;
    try {
      text = await fs.readFile(file, 'utf8');
    } catch (err) {
      if (err.code === 'ENOENT') return { version: 1, licenses: {} };
      throw err;
    }
    const data = JSON.parse(text);
    if (!data || data.version !== 1 || typeof data.licenses !== 'object' || data.licenses === null) {
      throw new Error(`store file has an unexpected format: ${file}`);
    }
    if (data.orders !== undefined && (typeof data.orders !== 'object' || data.orders === null)) {
      throw new Error(`store file has an unexpected format: ${file}`);
    }
    return data;
  }

  async function writeAtomic(data) {
    const tmp = `${file}.${process.pid}.${crypto.randomBytes(6).toString('hex')}.tmp`;
    const handle = await fs.open(tmp, 'wx', 0o600);
    try {
      if (typeof process.getuid === 'function' && process.getuid() === 0) {
        const owner = await fs.stat(path.dirname(file));
        await handle.chown(owner.uid, owner.gid);
      }
      await handle.writeFile(`${JSON.stringify(data, null, 2)}\n`, 'utf8');
      await handle.sync();
    } finally {
      await handle.close();
    }
    await fs.rename(tmp, file);
  }

  const lockFile = `${file}.lock`;
  async function withFileLock(fn) {
    await fs.mkdir(path.dirname(file), { recursive: true });
    const deadline = Date.now() + LOCK_WAIT_MS;
    for (;;) {
      try {
        const h = await fs.open(lockFile, 'wx', 0o600);
        await h.close();
        break;
      } catch (err) {
        if (err.code !== 'EEXIST') throw err;
      }
      try {
        const st = await fs.stat(lockFile);
        if (Date.now() - st.mtimeMs > LOCK_STALE_MS) {
          await fs.unlink(lockFile).catch(() => {});
          continue;
        }
      } catch (err) {
        if (err.code === 'ENOENT') continue;
        throw err;
      }
      if (Date.now() > deadline) throw new Error(`store is locked by another process: ${lockFile}`);
      await new Promise((resolve) => setTimeout(resolve, 20));
    }
    try {
      return await fn();
    } finally {
      await fs.unlink(lockFile).catch(() => {});
    }
  }

  function serialized(fn) {
    const next = chain.then(fn, fn);
    chain = next.catch(() => {});
    return next;
  }

  return {
    file,
    get(id) {
      return serialized(async () => {
        const data = await load();
        return Object.hasOwn(data.licenses, id) ? data.licenses[id] : null;
      });
    },
    put(id, record) {
      return serialized(() =>
        withFileLock(async () => {
          const data = await load();
          data.licenses[id] = record;
          await writeAtomic(data);
        }),
      );
    },
    update(id, fn) {
      return serialized(() =>
        withFileLock(async () => {
          const data = await load();
          const record = Object.hasOwn(data.licenses, id) ? data.licenses[id] : null;
          const before = JSON.stringify(record);
          const result = await fn(record);
          if (record !== null && JSON.stringify(record) !== before) {
            data.licenses[id] = record;
            await writeAtomic(data);
          }
          return result;
        }),
      );
    },
    getOrder(key) {
      return serialized(async () => {
        const data = await load();
        return data.orders && Object.hasOwn(data.orders, key) ? data.orders[key] : null;
      });
    },
    putOrder(key, order) {
      return serialized(() =>
        withFileLock(async () => {
          const data = await load();
          data.orders ??= {};
          data.orders[key] = order;
          await writeAtomic(data);
        }),
      );
    },
  };
}
