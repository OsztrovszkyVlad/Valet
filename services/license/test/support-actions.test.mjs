// resend.mjs, reissue.mjs, undispute.mjs, reset on a disputed license, and stores written
// by the previous version (Vlad, 2026-10-29). Merchant APIs and Resend are replaced.
import { describe, test } from 'node:test';
import assert from 'node:assert/strict';
import http from 'node:http';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { execFileSync, spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { loadSecretKeyFromEnv, publicKeyLine } from '../lib/signing.mjs';
import { issueLicense, openEnvelope } from '../lib/license.mjs';
import { createJsonFileStore, newLicenseRecord } from '../lib/store.mjs';
import { createHandler } from '../lib/handler.mjs';
import { RESEND_URL } from '../lib/mail.mjs';
import { resendLicense, reissueLicense, undisputeLicense, ActionRefused, ActionFailed } from '../lib/actions.mjs';

const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const NOW = 1_793_268_950;
const DAY = 86_400;
// Test-only secrets.
const PADDLE_SECRET = 'test-paddle-notification-secret';
const STRIPE_SECRET = 'test-stripe-endpoint-secret';
const PRICE = 'pri_01jbx2valetpro0000000000a';
const TXN = 'txn_01jbx3dq8w5h2k9m4n7p0r3t6v';
const CTM = 'ctm_01jbx3cz1a4s7d0f3g6h9j2k5l';
const CS = 'cs_test_a1B2c3D4e5F6g7H8i9J0k1L2m3N4o5P6q7R8s9T0u1V2w3X4y5Z6';
const PI = 'pi_3QBx7mE5valet0000000001';
const BUYER = 'buyer@example.com';
const machine = (n) => n.toString(16).padStart(32, '0');

const keyEnv = (() => {
  const out = execFileSync(process.execPath, [path.join(root, 'keygen.mjs')], { encoding: 'utf8' });
  return Object.fromEntries(
    out.split('\n').filter((l) => l.startsWith('VALET_LICENSE_')).map((l) => [l.slice(0, l.indexOf('=')), l.slice(l.indexOf('=') + 1)]),
  );
})();
const key = loadSecretKeyFromEnv(keyEnv);
const pub = publicKeyLine(key);
const mail = { apiKey: 'test-resend-key', from: 'Valet <licente@valetapp.ro>' };
const paddle = { apiKey: 'test-paddle-api-key', priceId: PRICE, sandbox: true };
const stripe = { apiKey: 'test-stripe-restricted-key' };
const cliEnv = {
  ...keyEnv,
  VALET_RESEND_API_KEY: mail.apiKey,
  VALET_MAIL_FROM: mail.from,
  VALET_PADDLE_API_KEY: paddle.apiKey,
  VALET_PADDLE_SANDBOX: '1',
  VALET_STRIPE_API_KEY: stripe.apiKey,
};

const fixture = (name) => JSON.parse(fs.readFileSync(new URL(`./fixtures/${name}.json`, import.meta.url), 'utf8'));
const hmac = (secret, text) => crypto.createHmac('sha256', secret).update(text).digest('hex');

/** Paddle customer + transaction API, Stripe checkout session API, Resend. */
function merchantFetch({ resendFailures = 0 } = {}) {
  const calls = [];
  const state = { email: BUYER, resendFailures };
  const json = (body, status = 200) => new Response(JSON.stringify(body), { status });
  const fn = async (url, init = {}) => {
    calls.push({ url, init });
    if (url === RESEND_URL) {
      if (state.resendFailures > 0) {
        state.resendFailures -= 1;
        return json({ name: 'internal_server_error' }, 500);
      }
      return json({ id: 'msg' });
    }
    let m = /^https:\/\/sandbox-api\.paddle\.com\/customers\/(ctm_[a-z0-9]+)$/.exec(url);
    if (m) return json({ data: { id: m[1], email: state.email } });
    m = /^https:\/\/sandbox-api\.paddle\.com\/transactions\/(txn_[a-z0-9]+)$/.exec(url);
    if (m) return json({ data: { id: m[1], status: 'completed', customer_id: CTM } });
    m = /^https:\/\/api\.stripe\.com\/v1\/checkout\/sessions\/(cs_[A-Za-z0-9_]+)$/.exec(url);
    if (m) return json({ id: m[1], object: 'checkout.session', customer_details: { email: state.email }, payment_intent: PI, payment_status: 'paid' });
    throw new Error(`unexpected fetch ${url}`);
  };
  fn.calls = calls;
  fn.state = state;
  fn.resendCalls = () => calls.filter((c) => c.url === RESEND_URL);
  return fn;
}

const attachment = (call) => JSON.parse(call.init.body).attachments[0].content;
const sentTo = (call) => JSON.parse(call.init.body).to;
const sentPayload = (call) => openEnvelope(JSON.parse(Buffer.from(attachment(call), 'base64').toString('utf8')), pub);

/** A store, a server with the merchant's webhook, and helpers. */
async function setup({ merchant = 'paddle', storeData } = {}) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'valet-license-actions-'));
  const file = path.join(dir, 'store.json');
  if (storeData) fs.writeFileSync(file, JSON.stringify(storeData, null, 2));
  const store = createJsonFileStore(file);
  const clock = { now: NOW };
  const fetch = merchantFetch();
  const webhook = {
    merchant,
    secrets: [merchant === 'paddle' ? PADDLE_SECRET : STRIPE_SECRET],
    mail,
    fetch,
    ...(merchant === 'paddle' ? { paddle } : { stripe }),
  };
  const server = http.createServer(createHandler({ store, key, now: () => clock.now, log: () => {}, webhook }));
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  const base = `http://127.0.0.1:${server.address().port}`;
  const post = async (route, body, headers = {}) => {
    const res = await globalThis.fetch(`${base}${route}`, { method: 'POST', headers: { 'Content-Type': 'application/json', ...headers }, body });
    await res.text();
    return res.status;
  };
  const t = {
    file,
    store,
    clock,
    fetch,
    data: () => JSON.parse(fs.readFileSync(file, 'utf8')),
    now: () => clock.now,
    paddle: (event) => {
      const body = JSON.stringify(event);
      return post('/v1/webhook/paddle', body, { 'Paddle-Signature': `ts=${clock.now};h1=${hmac(PADDLE_SECRET, `${clock.now}:${body}`)}` });
    },
    stripe: (event) => {
      const body = JSON.stringify(event);
      return post('/v1/webhook/stripe', body, { 'Stripe-Signature': `t=${clock.now},v1=${hmac(STRIPE_SECRET, `${clock.now}.${body}`)}` });
    },
    validate: (id, m = machine(1)) => post('/v1/validate', JSON.stringify({ license_id: id, machine: m, app_version: '0.2.0' })),
    activate: (id, m = machine(1)) => post('/v1/activate', JSON.stringify({ license_id: id, machine: m, app_version: '0.2.0' })),
    action: (fn, extra) => fn({ store, key, fetch, mail, paddle, stripe, now: t.now, ...extra }),
    cli: (script, args, env = {}) =>
      spawnSync(process.execPath, [path.join(root, script), ...args, '--store', file], { encoding: 'utf8', env: { PATH: process.env.PATH, ...env } }),
    close: () => new Promise((resolve) => server.close(resolve)),
  };
  return t;
}

const paddleChargeback = () => {
  const adj = fixture('paddle-adjustment-created');
  adj.data.action = 'chargeback';
  adj.data.status = 'approved';
  return adj;
};
const paddleRefund = () => {
  const adj = fixture('paddle-adjustment-created');
  adj.data.status = 'approved';
  return adj;
};
const stripeDispute = () => ({
  id: 'evt_1QDx7mE5valet00000000009',
  object: 'event',
  type: 'charge.dispute.created',
  data: { object: { id: 'du_1QDx7mE5valet0000000001', object: 'dispute', charge: 'ch_3QBx7mE5valet0000000001', payment_intent: PI, status: 'needs_response' } },
});

async function paddlePurchase(t) {
  assert.equal(await t.paddle(fixture('paddle-transaction-completed')), 200);
  const [id] = Object.keys(t.data().licenses);
  return id;
}

describe('reset and undispute on a disputed license', () => {
  test('reset refuses a disputed license and leaves it revoked', async () => {
    const t = await setup();
    try {
      const id = await paddlePurchase(t);
      assert.equal(await t.activate(id), 200);
      await t.paddle(paddleChargeback());
      const before = t.data().licenses[id];
      for (const args of [['--order', TXN], ['--license', id]]) {
        const r = t.cli('reset.mjs', args);
        assert.equal(r.status, 2, r.stderr);
        assert.match(r.stderr, /disputed .*stays revoked; reset refused, nothing changed/);
      }
      assert.deepEqual(t.data().licenses[id], before, 'record unchanged, machine still listed as active');
      assert.equal(await t.validate(id), 410);
    } finally {
      await t.close();
    }
  });

  test('reset on a refunded license clears machines and says it stays revoked', async () => {
    const t = await setup();
    try {
      const id = await paddlePurchase(t);
      await t.activate(id);
      await t.paddle(paddleRefund());
      const r = t.cli('reset.mjs', ['--order', TXN]);
      assert.equal(r.status, 0, r.stderr);
      assert.match(r.stderr, /1 active machine\(s\) cleared/);
      assert.match(r.stderr, /stays revoked \(refund\)/);
      assert.equal(t.data().licenses[id].revoked, true);
    } finally {
      await t.close();
    }
  });

  test('undispute without --confirm refuses; with --confirm restores and records the history', async () => {
    const t = await setup();
    try {
      const id = await paddlePurchase(t);
      t.clock.now += 100;
      await t.paddle(paddleChargeback());
      const before = t.data().licenses[id];

      const refused = t.cli('undispute.mjs', ['--order', TXN]);
      assert.equal(refused.status, 2);
      assert.match(refused.stderr, /is disputed; undispute clears the dispute flag and the license will be active again\. Run again with --confirm/);
      assert.deepEqual(t.data().licenses[id], before);
      assert.equal(await t.validate(id), 410);

      assert.equal(t.cli('undispute.mjs', ['--order', TXN, '--confirm', '--by', 'me@example.com']).status, 2, '--by is a name, not an email');

      const ok = t.cli('undispute.mjs', ['--order', TXN, '--confirm', '--by', 'vlad']);
      assert.equal(ok.status, 0, ok.stderr);
      assert.match(ok.stderr, /no longer disputed and active again/);
      const rec = t.data().licenses[id];
      assert.equal(rec.disputed, false);
      assert.equal(rec.revoked, false);
      assert.equal(rec.revoked_reason, undefined);
      assert.deepEqual(rec.history.map((h) => h.action), ['dispute', 'undispute']);
      assert.equal(rec.history[1].by, 'vlad');
      assert.equal(typeof rec.history[1].at, 'number');
      assert.equal(t.fetch.resendCalls().length, 1, 'no email from undispute');
      assert.equal(await t.validate(id), 200, 'active again');

      const again = t.cli('undispute.mjs', ['--license', id, '--confirm']);
      assert.equal(again.status, 0);
      assert.match(again.stderr, /not disputed; nothing changed/);
    } finally {
      await t.close();
    }
  });

  test('undispute keeps a refunded license revoked', async () => {
    const t = await setup();
    try {
      const id = await paddlePurchase(t);
      await t.paddle(paddleRefund());
      await t.paddle(paddleChargeback());
      const r = await t.action(undisputeLicense, { licenseId: id, confirm: true });
      assert.deepEqual(r, { changed: true, revoked: true, revokedReason: 'refund' });
      assert.equal(t.data().licenses[id].disputed, false);
      assert.equal(await t.validate(id), 410);
    } finally {
      await t.close();
    }
  });
});

describe('resend', () => {
  test('paddle: email read through the stored customer id, same license, byte-identical file', async () => {
    const t = await setup();
    try {
      const id = await paddlePurchase(t);
      assert.equal(t.data().orders[`paddle:${TXN}`].customer_id, CTM, 'customer id stored with the order');
      const original = t.fetch.resendCalls()[0];
      t.clock.now += 30 * DAY;
      const r = await t.action(resendLicense, { orderId: TXN });
      assert.deepEqual(r, { licenseId: id, firstEmail: false });
      const [, again] = t.fetch.resendCalls();
      assert.deepEqual(sentTo(again), [BUYER]);
      assert.equal(attachment(again), attachment(original), 'same bytes');
      assert.equal(sentPayload(again).license_id, id);
      assert.ok(t.fetch.calls.some((c) => c.url === `https://sandbox-api.paddle.com/customers/${CTM}`));
      assert.ok(!t.fetch.calls.some((c) => c.url.includes('/transactions/')), 'no transaction lookup when the customer id is stored');
      const rec = t.data().licenses[id];
      assert.equal(rec.emailed_at, NOW, 'first email date kept');
      assert.deepEqual(rec.history, [{ at: NOW + 30 * DAY, action: 'resend' }]);
      assert.ok(!fs.readFileSync(t.file, 'utf8').includes(BUYER), 'the store holds no email');
    } finally {
      await t.close();
    }
  });

  test('paddle: a changed email at the merchant changes only the email in the file', async () => {
    const t = await setup();
    try {
      const id = await paddlePurchase(t);
      t.fetch.state.email = 'new-address@example.com';
      await t.action(resendLicense, { orderId: TXN });
      const [first, second] = t.fetch.resendCalls().map(sentPayload);
      assert.deepEqual(sentTo(t.fetch.resendCalls()[1]), ['new-address@example.com']);
      assert.deepEqual({ ...second, email: BUYER }, first);
      assert.equal(second.license_id, id);
    } finally {
      await t.close();
    }
  });

  test('stripe: email read from the checkout session, also when support gives the payment intent', async () => {
    const t = await setup({ merchant: 'stripe' });
    try {
      assert.equal(await t.stripe(fixture('stripe-checkout-session-completed')), 200);
      const [id] = Object.keys(t.data().licenses);
      assert.deepEqual(t.data().orders[`stripe:${PI}`].order_keys, [`stripe:${CS}`, `stripe:${PI}`]);
      const original = t.fetch.resendCalls()[0];
      for (const orderId of [CS, PI]) {
        const r = await t.action(resendLicense, { orderId });
        assert.equal(r.licenseId, id);
        const last = t.fetch.resendCalls().at(-1);
        assert.equal(attachment(last), attachment(original), `same bytes via ${orderId.slice(0, 3)}`);
      }
      const sessionCalls = t.fetch.calls.filter((c) => c.url.startsWith('https://api.stripe.com/'));
      assert.equal(sessionCalls.length, 2);
      assert.equal(sessionCalls[0].url, `https://api.stripe.com/v1/checkout/sessions/${CS}`);
      assert.equal(sessionCalls[0].init.headers.Authorization, `Bearer ${stripe.apiKey}`);
    } finally {
      await t.close();
    }
  });

  test('refused for revoked and disputed licenses; a Resend failure changes nothing', async () => {
    const t = await setup();
    try {
      const id = await paddlePurchase(t);
      t.fetch.state.resendFailures = 1;
      await assert.rejects(t.action(resendLicense, { orderId: TXN }), (err) => err instanceof ActionFailed && /nothing changed/.test(err.message));
      assert.equal(t.data().licenses[id].history, undefined);

      await t.paddle(paddleRefund());
      const sent = t.fetch.resendCalls().length;
      await assert.rejects(t.action(resendLicense, { orderId: TXN }), (err) => err instanceof ActionRefused && /revoked \(refund\)/.test(err.message));
      await t.paddle(paddleChargeback());
      await assert.rejects(t.action(resendLicense, { orderId: TXN }), (err) => err instanceof ActionRefused && /disputed/.test(err.message));
      const cli = t.cli('resend.mjs', ['--order', TXN], cliEnv);
      assert.equal(cli.status, 2);
      assert.match(cli.stderr, /disputed \(chargeback\); resend refused/);
      assert.equal(t.fetch.resendCalls().length, sent, 'nothing sent');
    } finally {
      await t.close();
    }
  });

  test('a webhook email that failed is completed by resend (sets emailed_at)', async () => {
    const t = await setup();
    try {
      t.fetch.state.resendFailures = 1;
      assert.equal(await t.paddle(fixture('paddle-transaction-completed')), 502);
      const [id] = Object.keys(t.data().licenses);
      const r = await t.action(resendLicense, { orderId: TXN });
      assert.deepEqual(r, { licenseId: id, firstEmail: true });
      assert.equal(t.data().licenses[id].emailed_at, NOW);
    } finally {
      await t.close();
    }
  });

  test('resend.mjs names missing variables and never prints values', async () => {
    const t = await setup();
    try {
      await paddlePurchase(t);
      const r = t.cli('resend.mjs', ['--order', TXN], { ...keyEnv, VALET_MAIL_FROM: mail.from });
      assert.equal(r.status, 2);
      assert.match(r.stderr, /missing: VALET_RESEND_API_KEY, VALET_PADDLE_API_KEY/);
      assert.ok(!r.stderr.includes(keyEnv.VALET_LICENSE_SECRET_KEY.slice(0, 16)));
      const s = t.cli('resend.mjs', ['--order', CS], { ...cliEnv, VALET_STRIPE_API_KEY: '' });
      assert.equal(s.status, 2);
      assert.match(s.stderr, /missing: VALET_STRIPE_API_KEY/);
    } finally {
      await t.close();
    }
  });
});

describe('reissue', () => {
  test('revokes the old license (reissued), maps the order to a new one with the same updates_until, emails it', async () => {
    const t = await setup();
    try {
      const oldId = await paddlePurchase(t);
      assert.equal(await t.activate(oldId), 200);
      const oldRec = t.data().licenses[oldId];

      const preview = t.cli('reissue.mjs', ['--order', TXN]);
      assert.equal(preview.status, 2);
      assert.match(preview.stderr, new RegExp(`would revoke license ${oldId} \\(reason reissued\\).*Nothing changed; run again with --confirm`));
      await assert.rejects(t.action(reissueLicense, { orderId: TXN }), ActionRefused, 'no confirm in the library either');
      assert.deepEqual(Object.keys(t.data().licenses), [oldId]);

      t.clock.now += 40 * DAY;
      const r = await t.action(reissueLicense, { orderId: TXN, confirm: true });
      assert.equal(r.oldId, oldId);
      assert.equal(r.emailed, true);
      assert.notEqual(r.newId, oldId);

      const data = t.data();
      const old = data.licenses[oldId];
      assert.equal(old.revoked, true);
      assert.equal(old.revoked_reason, 'reissued');
      assert.equal(old.disputed, undefined);
      assert.deepEqual(old.history, [{ at: NOW + 40 * DAY, action: 'reissued', to: r.newId }]);
      const neu = data.licenses[r.newId];
      assert.equal(neu.updates_until, oldRec.updates_until, 'no update time lost or gained');
      assert.equal(neu.issued_at, NOW + 40 * DAY);
      assert.equal(neu.max_activations, 3);
      assert.equal(neu.emailed_at, NOW + 40 * DAY);
      assert.deepEqual(neu.history, [{ at: NOW + 40 * DAY, action: 'issued', reissue_of: oldId }]);
      const order = data.orders[`paddle:${TXN}`];
      assert.equal(order.license_id, r.newId);
      assert.equal(order.customer_id, CTM);
      assert.deepEqual(order.history, [{ at: NOW + 40 * DAY, action: 'reissue', from: oldId, to: r.newId }]);

      const mailCall = t.fetch.resendCalls().at(-1);
      assert.deepEqual(sentTo(mailCall), [BUYER]);
      const payload = sentPayload(mailCall);
      assert.equal(payload.license_id, r.newId);
      assert.equal(payload.updates_until, oldRec.updates_until);

      assert.equal(await t.validate(oldId), 410, 'old id revoked');
      assert.equal(await t.activate(r.newId), 200, 'new id activates');

      const lookup = t.cli('lookup.mjs', ['--order', TXN]);
      assert.equal(lookup.status, 0, lookup.stderr);
      assert.match(lookup.stdout, new RegExp(`^license_id: ${r.newId}$`, 'm'));
      assert.match(lookup.stdout, new RegExp(`^history: \\d{4}-\\d{2}-\\d{2} issued replaces ${oldId}$`, 'm'));
      assert.match(lookup.stdout, new RegExp(`^order: \\d{4}-\\d{2}-\\d{2} reissued, previous license ${oldId}$`, 'm'));
      assert.ok(!lookup.stdout.includes('@'));
    } finally {
      await t.close();
    }
  });

  test('stripe: both order keys move to the new license', async () => {
    const t = await setup({ merchant: 'stripe' });
    try {
      await t.stripe(fixture('stripe-checkout-session-completed'));
      const r = await t.action(reissueLicense, { orderId: PI, confirm: true });
      const data = t.data();
      assert.equal(data.orders[`stripe:${CS}`].license_id, r.newId);
      assert.equal(data.orders[`stripe:${PI}`].license_id, r.newId);
      // A dispute on the payment intent now reaches the new license.
      await t.stripe(stripeDispute());
      assert.equal(t.data().licenses[r.newId].disputed, true);
      assert.equal(t.data().licenses[r.oldId].revoked_reason, 'reissued');
    } finally {
      await t.close();
    }
  });

  test('refused for disputed and revoked licenses', async () => {
    const t = await setup();
    try {
      await paddlePurchase(t);
      await t.paddle(paddleChargeback());
      await assert.rejects(t.action(reissueLicense, { orderId: TXN, confirm: true }), (err) => err instanceof ActionRefused && /disputed/.test(err.message));
      const cli = t.cli('reissue.mjs', ['--order', TXN, '--confirm'], cliEnv);
      assert.equal(cli.status, 2);
      assert.match(cli.stderr, /reissue refused/);
      assert.equal(Object.keys(t.data().licenses).length, 1);
      assert.equal(t.fetch.calls.filter((c) => c.url.includes('/customers/')).length, 1, 'only the purchase lookup');
    } finally {
      await t.close();
    }
  });

  test('a failed email leaves the new license mapped; resend sends it', async () => {
    const t = await setup();
    try {
      const oldId = await paddlePurchase(t);
      t.fetch.state.resendFailures = 1;
      const r = await t.action(reissueLicense, { orderId: TXN, confirm: true });
      assert.equal(r.emailed, false);
      assert.match(r.error, /resend answered 500/);
      const data = t.data();
      assert.equal(data.orders[`paddle:${TXN}`].license_id, r.newId);
      assert.equal(data.licenses[r.newId].emailed_at, undefined);
      assert.equal(data.licenses[oldId].revoked, true);

      const again = await t.action(resendLicense, { orderId: TXN });
      assert.deepEqual(again, { licenseId: r.newId, firstEmail: true });
      assert.equal(sentPayload(t.fetch.resendCalls().at(-1)).license_id, r.newId);
      // The webhook's own redelivery path re-signs a reissued license too (updates_until is not issued_at + whole days).
      assert.equal(await t.paddle(fixture('paddle-transaction-completed')), 200);
    } finally {
      await t.close();
    }
  });
});

describe('stores written by the previous version', () => {
  /** The format before this change: no customer_id, order_keys or history; refund reason only. */
  function oldStore() {
    const a = issueLicense({ email: BUYER, issuedAt: NOW - 10 * DAY }, key);
    const b = issueLicense({ email: BUYER, issuedAt: NOW - 5 * DAY }, key);
    const c = issueLicense({ email: BUYER, issuedAt: NOW - 3 * DAY }, key);
    const rec = (p, extra) => ({ ...newLicenseRecord(p), emailed_at: p.issued_at, ...extra });
    return {
      envelopes: { a: a.envelope, b: b.envelope },
      ids: { a: a.payload.license_id, b: b.payload.license_id, c: c.payload.license_id },
      data: {
        version: 1,
        licenses: {
          [a.payload.license_id]: rec(a.payload, {
            machines: { [machine(1)]: { first_seen: NOW - 9 * DAY, last_seen: NOW - DAY, active: true, deactivated_at: null } },
            resets: [{ at: NOW - 8 * DAY, cleared: 1 }],
          }),
          [b.payload.license_id]: rec(b.payload),
          [c.payload.license_id]: rec(c.payload, { revoked: true, revoked_at: NOW - DAY, revoked_reason: 'refund' }),
        },
        orders: {
          [`paddle:${TXN}`]: { license_id: a.payload.license_id, created_at: NOW - 10 * DAY },
          [`stripe:${CS}`]: { license_id: b.payload.license_id, created_at: NOW - 5 * DAY },
          [`stripe:${PI}`]: { license_id: b.payload.license_id, created_at: NOW - 5 * DAY },
          'paddle:txn_01refundonly0000000000000': { license_id: null, refunded_at: NOW - 2 * DAY },
        },
      },
    };
  }
  const mailed = (envelope) => Buffer.from(`${JSON.stringify(envelope, null, 2)}\n`, 'utf8').toString('base64');

  test('lookup, activation and resend work; Paddle orders without a customer id use the transaction', async () => {
    const old = oldStore();
    const t = await setup({ storeData: old.data });
    try {
      const lookup = t.cli('lookup.mjs', ['--order', TXN]);
      assert.equal(lookup.status, 0, lookup.stderr);
      assert.match(lookup.stdout, /^revoked: no$/m);
      assert.match(lookup.stdout, /^disputed: no$/m);
      assert.match(lookup.stdout, /^resets: 1$/m);
      assert.ok(!/^history:/m.test(lookup.stdout));
      assert.equal(await t.validate(old.ids.a), 200);
      assert.equal(await t.validate(old.ids.c), 410);

      await t.action(resendLicense, { orderId: TXN });
      const urls = t.fetch.calls.map((c) => c.url);
      assert.deepEqual(urls.slice(0, 2), [`https://sandbox-api.paddle.com/transactions/${TXN}`, `https://sandbox-api.paddle.com/customers/${CTM}`]);
      assert.equal(attachment(t.fetch.resendCalls()[0]), mailed(old.envelopes.a), 'byte-identical to the file issued by the old version');

      const after = t.data();
      assert.deepEqual(after.licenses[old.ids.a].resets, [{ at: NOW - 8 * DAY, cleared: 1 }], 'old fields kept');
      assert.deepEqual(after.orders['paddle:txn_01refundonly0000000000000'], { license_id: null, refunded_at: NOW - 2 * DAY });
    } finally {
      await t.close();
    }
  });

  test('stripe: an old payment-intent entry asks for the session id; the session id works and reissue moves both keys', async () => {
    const old = oldStore();
    const t = await setup({ merchant: 'stripe', storeData: old.data });
    try {
      await assert.rejects(t.action(resendLicense, { orderId: PI }), (err) => err instanceof ActionRefused && /cs_… id/.test(err.message));
      await t.action(resendLicense, { orderId: CS });
      assert.equal(attachment(t.fetch.resendCalls()[0]), mailed(old.envelopes.b));

      const r = await t.action(reissueLicense, { orderId: CS, confirm: true });
      const data = t.data();
      assert.equal(data.orders[`stripe:${CS}`].license_id, r.newId);
      assert.equal(data.orders[`stripe:${PI}`].license_id, r.newId, 'payment intent from the session read');
      assert.deepEqual(data.orders[`stripe:${PI}`].order_keys, [`stripe:${CS}`, `stripe:${PI}`]);
    } finally {
      await t.close();
    }
  });

  test('a dispute on an old entry revokes and marks disputed', async () => {
    const old = oldStore();
    const t = await setup({ merchant: 'stripe', storeData: old.data });
    try {
      assert.equal(await t.stripe(stripeDispute()), 200);
      const rec = t.data().licenses[old.ids.b];
      assert.equal(rec.disputed, true);
      assert.equal(rec.revoked_reason, 'dispute');
      assert.equal(await t.validate(old.ids.b), 410);
    } finally {
      await t.close();
    }
  });
});

describe('CLIs end to end (fetch replaced in the child process)', () => {
  // Preloaded with --import: replaces fetch for the Paddle API and Resend, logs every call.
  const mock = `
    import { appendFileSync } from 'node:fs';
    globalThis.fetch = async (url, init = {}) => {
      appendFileSync(process.env.MOCK_FETCH_LOG, JSON.stringify({ url, body: init.body ?? null }) + '\\n');
      const json = (b, status = 200) => new Response(JSON.stringify(b), { status });
      if (url === 'https://api.resend.com/emails') return json({ id: 'msg' }, Number(process.env.MOCK_RESEND_STATUS ?? 200));
      if (url.startsWith('https://sandbox-api.paddle.com/customers/')) return json({ data: { email: 'buyer@example.com' } });
      throw new Error('unexpected fetch ' + url);
    };`;
  const preload = `data:text/javascript,${encodeURIComponent(mock)}`;

  test('resend.mjs and reissue.mjs --confirm', async () => {
    const t = await setup();
    try {
      const oldId = await paddlePurchase(t);
      const original = attachment(t.fetch.resendCalls()[0]);
      const log = path.join(path.dirname(t.file), 'fetch.log');
      const run = (script, args, extra = {}) =>
        spawnSync(process.execPath, ['--import', preload, path.join(root, script), ...args, '--store', t.file], {
          encoding: 'utf8',
          env: { PATH: process.env.PATH, ...cliEnv, MOCK_FETCH_LOG: log, ...extra },
        });
      const calls = () => fs.readFileSync(log, 'utf8').trim().split('\n').map((l) => JSON.parse(l));

      const r = run('resend.mjs', ['--order', TXN]);
      assert.equal(r.status, 0, r.stderr);
      assert.match(r.stderr, new RegExp(`license ${oldId} sent`));
      assert.ok(!r.stderr.includes(BUYER));
      assert.equal(JSON.parse(calls().at(-1).body).attachments[0].content, original, 'same bytes from the CLI');

      const failed = run('reissue.mjs', ['--order', TXN, '--confirm'], { MOCK_RESEND_STATUS: '500' });
      assert.equal(failed.status, 1);
      assert.match(failed.stderr, new RegExp(`email not sent .*run resend\\.mjs --order ${TXN}`));
      const newId = t.data().orders[`paddle:${TXN}`].license_id;
      assert.notEqual(newId, oldId);

      const retry = run('resend.mjs', ['--order', TXN]);
      assert.equal(retry.status, 0, retry.stderr);
      assert.match(retry.stderr, new RegExp(`license ${newId} sent .*first successful email`));
      assert.equal(await t.validate(oldId), 410);
      assert.equal(await t.activate(newId), 200);
    } finally {
      await t.close();
    }
  });
});
