#!/usr/bin/env node
// Test helper: the license service's real handler, store and signing code
// (services/license/lib) behind node:http, like services/license/server.mjs, plus a
// movable clock. server.mjs always signs with the real time; the "offline 61 days, then
// a successful validate restores Pro" check needs the service clock in the app's future.
//
//   node scripts/license-test-server.mjs --store <file.json> --clock-file <file> [--port 0]
//
// The clock file holds an offset in seconds added to the real time. It is read on every
// request, so a test can move the clock while the server runs. Missing file: offset 0.
// Needs VALET_LICENSE_SECRET_KEY and VALET_LICENSE_KEY_ID (a TEST key). Prints
// "server: listening on http://127.0.0.1:<port>, ..." to stderr, as server.mjs does.
// Not for production.

import fs from 'node:fs';
import http from 'node:http';
import { parseArgs } from 'node:util';
import { loadSecretKeyFromEnv } from '../services/license/lib/signing.mjs';
import { createJsonFileStore } from '../services/license/lib/store.mjs';
import { createHandler } from '../services/license/lib/handler.mjs';

const { values: args } = parseArgs({
  options: {
    port: { type: 'string', default: '0' },
    store: { type: 'string' },
    'clock-file': { type: 'string' },
  },
  strict: true,
});
if (!args.store || !args['clock-file']) {
  process.stderr.write('license-test-server: --store and --clock-file are required\n');
  process.exit(2);
}

function offset() {
  try {
    const n = Number(fs.readFileSync(args['clock-file'], 'utf8').trim());
    return Number.isSafeInteger(n) ? n : 0;
  } catch {
    return 0;
  }
}

const store = createJsonFileStore(args.store);
const key = loadSecretKeyFromEnv();
const now = () => Math.floor(Date.now() / 1000) + offset();
const server = http.createServer(createHandler({ store, key, now, log: () => {} }));
server.listen(Number(args.port), '127.0.0.1', () => {
  const addr = server.address();
  process.stderr.write(`server: listening on http://${addr.address}:${addr.port}, store ${store.file}\n`);
});
for (const signal of ['SIGINT', 'SIGTERM']) {
  process.on(signal, () => server.close(() => process.exit(0)));
}
