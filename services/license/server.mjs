#!/usr/bin/env node
// Local run of the activation API.
//   node services/license/server.mjs --port 8787 --store ./tmp-store.json [--host 127.0.0.1]
// Needs VALET_LICENSE_SECRET_KEY and VALET_LICENSE_KEY_ID. Plain HTTP: in production
// the host terminates TLS in front of it.

import http from 'node:http';
import { parseArgs } from 'node:util';
import { loadSecretKeyFromEnv } from './lib/signing.mjs';
import { createJsonFileStore } from './lib/store.mjs';
import { createHandler } from './lib/handler.mjs';

function fail(message) {
  process.stderr.write(`server: ${message}\n`);
  process.exit(2);
}

let args;
try {
  args = parseArgs({
    options: {
      port: { type: 'string', default: process.env.PORT ?? '8787' },
      host: { type: 'string', default: '127.0.0.1' },
      store: { type: 'string' },
    },
    strict: true,
  }).values;
} catch (err) {
  fail(err.message);
}

const port = Number(args.port);
if (!Number.isInteger(port) || port < 0 || port > 65535) fail('--port must be 0-65535');
const storePath = args.store ?? process.env.VALET_LICENSE_STORE;
if (!storePath) fail('--store or VALET_LICENSE_STORE is required');

let key;
try {
  key = loadSecretKeyFromEnv();
} catch (err) {
  fail(err.message);
}

const store = createJsonFileStore(storePath);
const server = http.createServer(createHandler({ store, key }));
server.headersTimeout = 10_000;
server.requestTimeout = 15_000;

server.listen(port, args.host, () => {
  const addr = server.address();
  process.stderr.write(`server: listening on http://${addr.address}:${addr.port}, store ${store.file}\n`);
});

for (const signal of ['SIGINT', 'SIGTERM']) {
  process.on(signal, () => server.close(() => process.exit(0)));
}
