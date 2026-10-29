#!/usr/bin/env node
// Issues a license file by hand (test purchases, support) and registers it in the store.
//   node services/license/issue.mjs --email a@b.c [--days 365] [--store path.json]
// Needs VALET_LICENSE_SECRET_KEY and VALET_LICENSE_KEY_ID. The store path comes from
// --store or VALET_LICENSE_STORE. Prints the .valet-license JSON to stdout.

import { parseArgs } from 'node:util';
import { loadSecretKeyFromEnv } from './lib/signing.mjs';
import { issueLicense, DEFAULT_DAYS } from './lib/license.mjs';
import { createJsonFileStore, registerLicense } from './lib/store.mjs';

function fail(message) {
  process.stderr.write(`issue: ${message}\n`);
  process.exit(2);
}

let args;
try {
  args = parseArgs({
    options: {
      email: { type: 'string' },
      days: { type: 'string', default: String(DEFAULT_DAYS) },
      store: { type: 'string' },
    },
    strict: true,
  }).values;
} catch (err) {
  fail(err.message);
}

if (!args.email) fail('--email is required');
const days = Number(args.days);
if (!Number.isSafeInteger(days) || days <= 0) fail('--days must be a positive integer');
const storePath = args.store ?? process.env.VALET_LICENSE_STORE;
if (!storePath) fail('--store or VALET_LICENSE_STORE is required (the license must be registered)');

let key;
try {
  key = loadSecretKeyFromEnv();
} catch (err) {
  fail(err.message);
}

let issued;
try {
  issued = issueLicense({ email: args.email, days }, key);
} catch (err) {
  fail(err.message);
}

const store = createJsonFileStore(storePath);
await registerLicense(store, issued.payload);
process.stdout.write(`${JSON.stringify(issued.envelope, null, 2)}\n`);
process.stderr.write(
  `issue: registered license ${issued.payload.license_id} in ${store.file}, ` +
    `updates until ${new Date(issued.payload.updates_until * 1000).toISOString().slice(0, 10)}\n`,
);
