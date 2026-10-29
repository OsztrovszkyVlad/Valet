#!/usr/bin/env node
// License service: activation API, merchant webhook, health check, daily store backup.
//   node server.mjs --port 8080 --store /data/store.json [--host 0.0.0.0] [--run-as 1000:1000]
//
// Environment: see lib/config.mjs and README.md. The server refuses to start when a
// variable needed for the configured merchant is missing, and names the variables
// (never their values). Plain HTTP: the host (Fly.io) terminates TLS in front of it.
//
// --run-as uid:gid: when started as root (the Fly volume is mounted owned by root), the
// server makes the store directory owned by uid:gid, then drops to that user before it
// opens the store or the port. Ignored when not started as root.

import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import { parseArgs } from 'node:util';
import { loadServerConfig } from './lib/config.mjs';
import { createJsonFileStore } from './lib/store.mjs';
import { createHandler } from './lib/handler.mjs';
import { createRateLimiter } from './lib/ratelimit.mjs';
import { startBackups } from './lib/backup.mjs';

function fail(message) {
  process.stderr.write(`server: ${message}\n`);
  process.exit(2);
}

const log = (entry) => console.log(JSON.stringify({ time: new Date().toISOString(), ...entry }));

let args;
try {
  args = parseArgs({
    options: {
      port: { type: 'string', default: process.env.PORT ?? '8787' },
      host: { type: 'string', default: '127.0.0.1' },
      store: { type: 'string' },
      'run-as': { type: 'string' },
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

let config;
try {
  config = loadServerConfig(process.env);
} catch (err) {
  fail(err.message);
}

function chownTree(target, uid, gid) {
  fs.lchownSync(target, uid, gid);
  if (!fs.lstatSync(target).isDirectory()) return;
  for (const entry of fs.readdirSync(target, { withFileTypes: true })) {
    const p = path.join(target, entry.name);
    if (entry.isDirectory()) chownTree(p, uid, gid);
    else fs.lchownSync(p, uid, gid);
  }
}

if (args['run-as'] !== undefined) {
  const m = /^(\d{1,10}):(\d{1,10})$/.exec(args['run-as']);
  if (!m) fail('--run-as must be uid:gid (numbers)');
  const uid = Number(m[1]);
  const gid = Number(m[2]);
  if (typeof process.getuid === 'function' && process.getuid() === 0) {
    const dir = path.dirname(path.resolve(storePath));
    try {
      fs.mkdirSync(dir, { recursive: true });
      chownTree(dir, uid, gid);
      process.setgroups([]);
      process.setgid(gid);
      process.setuid(uid);
    } catch (err) {
      fail(`could not switch to ${uid}:${gid} (${err.code ?? err.name})`);
    }
  }
}

const store = createJsonFileStore(storePath);
const handler = createHandler({
  store,
  key: config.key,
  log,
  webhook: config.webhook,
  rateLimiter: createRateLimiter(config.rateLimit),
  trustFly: config.trustFly,
});
const backups = startBackups({ storeFile: store.file, log });

const server = http.createServer(handler);
server.headersTimeout = 10_000;
server.requestTimeout = 15_000;

server.listen(port, args.host, () => {
  const addr = server.address();
  process.stderr.write(
    `server: listening on http://${addr.address}:${addr.port}, store ${store.file}, merchant ${config.merchant}` +
      `${config.webhook?.paddle?.sandbox ? ' (sandbox)' : ''}, rate limit ${config.rateLimit.perMinute}/min` +
      `${config.trustFly ? ', client address from Fly-Client-IP' : ''}\n`,
  );
});

for (const signal of ['SIGINT', 'SIGTERM']) {
  process.on(signal, () => {
    backups.stop();
    server.close(() => process.exit(0));
    // Open keep-alive connections would hold close() back; Fly sends SIGKILL after kill_timeout anyway.
    server.closeIdleConnections?.();
  });
}
