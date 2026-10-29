import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawn, spawnSync, execFileSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const server = path.join(root, 'server.mjs');

function keyEnv() {
  const out = execFileSync(process.execPath, [path.join(root, 'keygen.mjs')], { encoding: 'utf8' });
  return Object.fromEntries(
    out.split('\n').filter((l) => l.startsWith('VALET_LICENSE_')).map((l) => [l.slice(0, l.indexOf('=')), l.slice(l.indexOf('=') + 1)]),
  );
}

const runSync = (args, env) =>
  spawnSync(process.execPath, [server, ...args], { encoding: 'utf8', env: { PATH: process.env.PATH, ...env }, timeout: 10_000 });

const storeArgs = () => ['--port', '0', '--store', path.join(fs.mkdtempSync(path.join(os.tmpdir(), 'valet-license-srv-')), 'store.json')];

test('server refuses to start without the base environment and names what is missing', () => {
  const r = runSync(storeArgs(), {});
  assert.equal(r.status, 2);
  for (const name of ['VALET_LICENSE_SECRET_KEY', 'VALET_LICENSE_KEY_ID', 'VALET_MERCHANT']) assert.match(r.stderr, new RegExp(name));
});

test('server refuses to start without the variables of the configured merchant, never printing values', () => {
  const key = keyEnv();
  const paddle = runSync(storeArgs(), { ...key, VALET_MERCHANT: 'paddle', VALET_WEBHOOK_SECRET: 'test-value-123', VALET_PADDLE_SANDBOX: '1' });
  assert.equal(paddle.status, 2);
  for (const name of ['VALET_RESEND_API_KEY', 'VALET_MAIL_FROM', 'VALET_PADDLE_API_KEY', 'VALET_PADDLE_PRICE_ID']) assert.match(paddle.stderr, new RegExp(name));
  assert.doesNotMatch(paddle.stderr, /VALET_WEBHOOK_SECRET/, 'set variables are not listed');
  for (const v of ['test-value-123', key.VALET_LICENSE_SECRET_KEY.slice(0, 16)]) assert.ok(!paddle.stderr.includes(v));

  const stripe = runSync(storeArgs(), { ...key, VALET_MERCHANT: 'stripe' });
  assert.equal(stripe.status, 2);
  for (const name of ['VALET_WEBHOOK_SECRET', 'VALET_RESEND_API_KEY', 'VALET_MAIL_FROM']) assert.match(stripe.stderr, new RegExp(name));
  assert.doesNotMatch(stripe.stderr, /PADDLE/);

  const wrong = runSync(storeArgs(), { ...key, VALET_MERCHANT: 'polar' });
  assert.equal(wrong.status, 2);
  assert.match(wrong.stderr, /invalid: VALET_MERCHANT/);
  assert.ok(!wrong.stderr.includes('polar'));

  const badKey = runSync(storeArgs(), { ...key, VALET_MERCHANT: 'none', VALET_LICENSE_SECRET_KEY: 'AAAAsecretish' });
  assert.equal(badKey.status, 2);
  assert.ok(!badKey.stderr.includes('AAAAsecretish'));

  const badPrice = runSync(storeArgs(), {
    ...key,
    VALET_MERCHANT: 'paddle',
    VALET_WEBHOOK_SECRET: 'a',
    VALET_RESEND_API_KEY: 'b',
    VALET_MAIL_FROM: 'Valet <licente@valetapp.ro>',
    VALET_PADDLE_API_KEY: 'c',
    VALET_PADDLE_PRICE_ID: 'pro_wrongkind',
  });
  assert.equal(badPrice.status, 2);
  assert.match(badPrice.stderr, /invalid: VALET_PADDLE_PRICE_ID/);
});

async function startServer(extraArgs, env) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'valet-license-srv-'));
  const store = path.join(dir, 'store.json');
  fs.writeFileSync(store, `${JSON.stringify({ version: 1, licenses: {} })}\n`);
  const child = spawn(process.execPath, [server, '--port', '0', '--store', store, ...extraArgs], {
    env: { PATH: process.env.PATH, ...env },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  let stderr = '';
  let stdout = '';
  child.stdout.on('data', (d) => {
    stdout += d;
  });
  const port = await new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error(`no listen line: ${stderr}`)), 10_000);
    child.stderr.on('data', (d) => {
      stderr += d;
      const m = /listening on http:\/\/[^:]+:(\d+)/.exec(stderr);
      if (m) {
        clearTimeout(timer);
        resolve(Number(m[1]));
      }
    });
    child.on('exit', (code) => reject(new Error(`exited ${code}: ${stderr}`)));
  });
  const stop = () =>
    new Promise((resolve) => {
      child.on('exit', (code, signal) => resolve({ code, signal }));
      child.kill('SIGTERM');
    });
  return { dir, port, stop, stderr: () => stderr, stdout: () => stdout };
}

test('server starts with a complete environment: /healthz, the merchant route only, a backup of the store', async () => {
  const s = await startServer([], {
    ...keyEnv(),
    VALET_MERCHANT: 'stripe',
    VALET_WEBHOOK_SECRET: 'test-stripe-endpoint-secret',
    VALET_RESEND_API_KEY: 'test-resend-key',
    VALET_MAIL_FROM: 'Valet <licente@valetapp.ro>',
  });
  try {
    const base = `http://127.0.0.1:${s.port}`;
    const health = await fetch(`${base}/healthz`);
    assert.equal(health.status, 200);
    assert.equal(await health.text(), 'ok');
    assert.equal((await fetch(`${base}/v1/webhook/paddle`, { method: 'POST', body: '{}' })).status, 404);
    assert.equal((await fetch(`${base}/v1/webhook/stripe`, { method: 'POST', body: '{}' })).status, 400);
    assert.match(s.stderr(), /merchant stripe, rate limit 30\/min/);
    assert.ok(!s.stderr().includes('test-stripe-endpoint-secret'));
    const today = new Date().toISOString().slice(0, 10);
    // The first backup runs right after start; wait for its log line.
    for (let i = 0; i < 100 && !s.stdout().includes('"backup"'); i++) await new Promise((r) => setTimeout(r, 20));
    assert.ok(fs.existsSync(path.join(s.dir, 'backups', `store-${today}.json`)));
  } finally {
    const exit = await s.stop();
    assert.equal(exit.code, 0);
  }
});

test('--run-as is validated; as root the store directory is handed to that user', async () => {
  const env = { ...keyEnv(), VALET_MERCHANT: 'none' };
  assert.equal(runSync([...storeArgs(), '--run-as', 'node'], env).status, 2);
  if (typeof process.getuid !== 'function' || process.getuid() !== 0) return;
  const s = await startServer(['--run-as', '65534:65534'], env);
  try {
    assert.equal(fs.statSync(s.dir).uid, 65534);
    assert.equal(fs.statSync(path.join(s.dir, 'store.json')).uid, 65534);
    assert.equal((await fetch(`http://127.0.0.1:${s.port}/healthz`)).status, 200);
  } finally {
    await s.stop();
  }
});
