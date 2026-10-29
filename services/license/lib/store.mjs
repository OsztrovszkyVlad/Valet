// Activation storage.
//
// Interface (both methods return promises, so a hosted database can implement it):
//   get(licenseId) -> record | null
//   put(licenseId, record) -> void      (insert or replace the record for that id)
//
// There is no delete. Revocation is `revoked: true`; deactivation sets `active: false`
// on the machine entry and keeps its history.
//
// Record:
//   {
//     license_id, max_activations, issued_at, updates_until,
//     revoked: false, revoked_at?,
//     resets?: [{ at, cleared }],
//     machines: { "<32 hex>": { first_seen, last_seen, active, deactivated_at } }
//   }
// The email is not stored here; the license file and the merchant hold it.

import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';

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
  return {
    async get(id) {
      return map.has(id) ? structuredClone(map.get(id)) : null;
    },
    async put(id, record) {
      map.set(id, structuredClone(record));
    },
  };
}

/**
 * JSON-file store for tests and local runs. The file holds
 * { "version": 1, "licenses": { "<id>": record } }.
 * Writes go to a temp file in the same directory, are fsynced, then renamed over the
 * original. Writes inside one process are serialized. Several processes writing the
 * same file at the same moment can lose an update; the hosted store must not.
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
    return data;
  }

  async function writeAtomic(data) {
    await fs.mkdir(path.dirname(file), { recursive: true });
    const tmp = `${file}.${process.pid}.${crypto.randomBytes(6).toString('hex')}.tmp`;
    const handle = await fs.open(tmp, 'wx', 0o600);
    try {
      await handle.writeFile(`${JSON.stringify(data, null, 2)}\n`, 'utf8');
      await handle.sync();
    } finally {
      await handle.close();
    }
    await fs.rename(tmp, file);
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
      return serialized(async () => {
        const data = await load();
        data.licenses[id] = record;
        await writeAtomic(data);
      });
    },
  };
}
