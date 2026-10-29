// Daily backup of the JSON store on the same volume (docs/LICENSING.md, "Hosting").
//
//   <store dir>/backups/store-YYYY-MM-DD.json   (UTC date of the copy)
//
// - One copy per day. An existing file for the day is never overwritten: the copy is
//   written to a temp file, fsynced, then hard-linked to its final name (link fails if
//   the name exists), and the temp name is unlinked.
// - A store file that doesn't parse is not copied (the older copies stay the good ones).
// - Only the newest `keep` (30) copies are kept. Older ones are unlinked, and only files
//   inside backups/ whose name matches store-YYYY-MM-DD.json exactly. These are server
//   copies of the store, not user files; the never-delete rule of the app is about files
//   on the user's machine.
// - The copies are on the same volume as the store: they cover a bad write or a bad
//   deploy, not the loss of the volume. Fly's volume snapshots and a copy pulled by Vlad
//   (README, "Backups") cover that.

import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';

export const BACKUP_KEEP = 30;
export const BACKUP_NAME = /^store-\d{4}-\d{2}-\d{2}\.json$/;
export const DAY_MS = 86_400_000;

export const backupDirFor = (storeFile) => path.join(path.dirname(path.resolve(storeFile)), 'backups');

/**
 * Makes today's copy if there is none, then prunes. Returns
 * { result: 'created' | 'exists' | 'no_store' | 'invalid_store', name, removed: number }.
 */
export async function backupStore(storeFile, { now = new Date(), keep = BACKUP_KEEP } = {}) {
  const dir = backupDirFor(storeFile);
  const name = `store-${now.toISOString().slice(0, 10)}.json`;
  const target = path.join(dir, name);
  let result;

  let text = null;
  try {
    text = await fs.readFile(path.resolve(storeFile), 'utf8');
  } catch (err) {
    if (err.code !== 'ENOENT') throw err;
  }
  if (text === null) {
    result = 'no_store';
  } else {
    let valid = true;
    try {
      const data = JSON.parse(text);
      valid = data?.version === 1 && typeof data.licenses === 'object' && data.licenses !== null;
    } catch {
      valid = false;
    }
    if (!valid) {
      result = 'invalid_store';
    } else {
      await fs.mkdir(dir, { recursive: true, mode: 0o700 });
      const tmp = path.join(dir, `.${name}.${crypto.randomBytes(6).toString('hex')}.tmp`);
      const handle = await fs.open(tmp, 'wx', 0o600);
      try {
        await handle.writeFile(text, 'utf8');
        await handle.sync();
      } finally {
        await handle.close();
      }
      try {
        await fs.link(tmp, target);
        result = 'created';
      } catch (err) {
        if (err.code !== 'EEXIST') throw err;
        result = 'exists';
      } finally {
        await fs.unlink(tmp);
      }
    }
  }

  const removed = await pruneBackups(dir, keep);
  return { result, name, removed };
}

/** Unlinks all but the newest `keep` files named store-YYYY-MM-DD.json in `dir`. */
export async function pruneBackups(dir, keep = BACKUP_KEEP) {
  let names;
  try {
    names = await fs.readdir(dir);
  } catch (err) {
    if (err.code === 'ENOENT') return 0;
    throw err;
  }
  // ISO dates sort lexically.
  const dated = names.filter((n) => BACKUP_NAME.test(n)).sort().reverse();
  let removed = 0;
  for (const n of dated.slice(keep)) {
    await fs.unlink(path.join(dir, n));
    removed += 1;
  }
  return removed;
}

/**
 * Runs a backup now and every `intervalMs` (24 h). Errors are logged by class name and
 * never stop the server. Returns a function that stops the timer.
 */
export function startBackups({ storeFile, log, intervalMs = DAY_MS, keep = BACKUP_KEEP }) {
  const run = async () => {
    try {
      const { result, removed } = await backupStore(storeFile, { keep });
      log({ backup: result, removed });
    } catch (err) {
      log({ backup: 'failed', error: err?.name ?? 'Error', ...(err?.code ? { code: String(err.code) } : {}) });
    }
  };
  const first = run();
  const timer = setInterval(run, intervalMs);
  timer.unref();
  return { first, stop: () => clearInterval(timer) };
}
