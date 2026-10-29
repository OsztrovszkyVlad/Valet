import { describe, test } from 'node:test';
import assert from 'node:assert/strict';
import http from 'node:http';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { generateKeyPair, publicKeyLine } from '../lib/signing.mjs';
import { openEnvelope } from '../lib/license.mjs';
import { createJsonFileStore } from '../lib/store.mjs';
import { createHandler } from '../lib/handler.mjs';
import { RESEND_URL, ATTACHMENT_NAME } from '../lib/mail.mjs';
import { verifyPaddleSignature, verifyStripeSignature, interpretPaddle, interpretStripe } from '../lib/webhook.mjs';

const key = generateKeyPair();
const pub = publicKeyLine(key);
const NOW = 1_793_268_950;
// Test-only secrets. Real ones live only in the host's secret store.
const PADDLE_SECRET = 'test-paddle-notification-secret';
const STRIPE_SECRET = 'test-stripe-endpoint-secret';
const RESEND_KEY = 'test-resend-key';
const PADDLE_API_KEY = 'test-paddle-api-key';
const PRICE = 'pri_01jbx2valetpro0000000000a';
const BUYER = 'buyer@example.com';
const machine = (n) => n.toString(16).padStart(32, '0');

const fixture = (name) => JSON.parse(fs.readFileSync(new URL(`./fixtures/${name}.json`, import.meta.url), 'utf8'));
const hmac = (secret, text) => crypto.createHmac('sha256', secret).update(text).digest('hex');
const paddleHeader = (body, { secret = PADDLE_SECRET, ts = NOW, extra = [] } = {}) =>
  [`ts=${ts}`, ...extra.map((h) => `h1=${h}`), `h1=${hmac(secret, `${ts}:${body}`)}`].join(';');
const stripeHeader = (body, { secret = STRIPE_SECRET, t = NOW, extra = [] } = {}) =>
  [`t=${t}`, ...extra.map((h) => `v1=${h}`), `v1=${hmac(secret, `${t}.${body}`)}`, 'v0=0000'].join(',');

/** fetch stand-in for Resend and the Paddle customer API. */
function mockFetch({ resendFailures = 0, paddleStatus = 200, hold = null } = {}) {
  const calls = [];
  let failures = resendFailures;
  const fn = async (url, init = {}) => {
    calls.push({ url, init });
    if (url === RESEND_URL) {
      if (hold) await hold;
      if (failures > 0) {
        failures -= 1;
        return new Response(JSON.stringify({ name: 'internal_server_error' }), { status: 500 });
      }
      return new Response(JSON.stringify({ id: '4ef9a417-02e9-4d39-ad75-9611e0fcc33c' }), { status: 200 });
    }
    const m = /^https:\/\/(sandbox-)?api\.paddle\.com\/customers\/(ctm_[a-z0-9]+)$/.exec(url);
    if (m) {
      if (paddleStatus !== 200) return new Response('{}', { status: paddleStatus });
      return new Response(JSON.stringify({ data: { id: m[2], email: BUYER, name: 'Ion Popescu', status: 'active' }, meta: { request_id: 'r' } }), { status: 200 });
    }
    throw new Error(`unexpected fetch ${url}`);
  };
  fn.calls = calls;
  fn.resendCalls = () => calls.filter((c) => c.url === RESEND_URL);
  fn.paddleCalls = () => calls.filter((c) => c.url !== RESEND_URL);
  return fn;
}

async function setup({ merchant = 'paddle', fetch = mockFetch(), sandbox = true, maxConcurrentWebhooks } = {}) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'valet-license-webhook-'));
  const store = createJsonFileStore(path.join(dir, 'store.json'));
  const logs = [];
  const clock = { now: NOW };
  const webhook = {
    merchant,
    secrets: [merchant === 'paddle' ? PADDLE_SECRET : STRIPE_SECRET],
    mail: { apiKey: RESEND_KEY, from: 'Valet <licente@valetapp.ro>' },
    fetch,
    ...(merchant === 'paddle' ? { paddle: { apiKey: PADDLE_API_KEY, priceId: PRICE, sandbox } } : {}),
  };
  const server = http.createServer(
    createHandler({ store, key, now: () => clock.now, log: (e) => logs.push(e), webhook, ...(maxConcurrentWebhooks ? { maxConcurrentWebhooks } : {}) }),
  );
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  const base = `http://127.0.0.1:${server.address().port}`;
  const raw = (route, body, headers) => fetchJson(`${base}${route}`, { method: 'POST', headers, body });
  return {
    store,
    file: path.join(dir, 'store.json'),
    logs,
    clock,
    fetch,
    base,
    raw,
    paddle: (event, opts) => {
      const body = JSON.stringify(event);
      return raw('/v1/webhook/paddle', body, { 'Content-Type': 'application/json', 'Paddle-Signature': paddleHeader(body, { ts: clock.now, ...opts }) });
    },
    stripe: (event, opts) => {
      const body = JSON.stringify(event);
      return raw('/v1/webhook/stripe', body, { 'Content-Type': 'application/json; charset=utf-8', 'Stripe-Signature': stripeHeader(body, { t: clock.now, ...opts }) });
    },
    activate: (id, m = machine(1)) =>
      raw('/v1/activate', JSON.stringify({ license_id: id, machine: m, app_version: '0.2.0' }), { 'Content-Type': 'application/json' }),
    validate: (id, m = machine(1)) =>
      raw('/v1/validate', JSON.stringify({ license_id: id, machine: m, app_version: '0.2.0' }), { 'Content-Type': 'application/json' }),
    data: () => JSON.parse(fs.readFileSync(path.join(dir, 'store.json'), 'utf8')),
    close: () => new Promise((resolve) => server.close(resolve)),
  };
}

async function fetchJson(url, init) {
  const res = await globalThis.fetch(url, init);
  const text = await res.text();
  let body;
  try {
    body = text ? JSON.parse(text) : undefined;
  } catch {
    body = text;
  }
  return { status: res.status, body, headers: res.headers };
}

function sentEmail(call) {
  const body = JSON.parse(call.init.body);
  assert.equal(body.attachments.length, 1);
  const envelope = JSON.parse(Buffer.from(body.attachments[0].content, 'base64').toString('utf8'));
  return { body, envelope, payload: openEnvelope(envelope, pub) };
}

describe('webhook signatures', () => {
  const body = Buffer.from('{"event_type":"transaction.completed"}');
  const s = body.toString();

  test('paddle: valid, invalid, old, future, rotated', () => {
    const v = (header, secrets = [PADDLE_SECRET], now = NOW) => verifyPaddleSignature({ header, body, secrets, now });
    assert.equal(v(paddleHeader(s)), true);
    assert.equal(v(paddleHeader(s, { secret: 'other' })), false);
    assert.equal(v(paddleHeader(s).replace(/h1=([0-9a-f])/, (m, c) => `h1=${c === '0' ? '1' : '0'}`)), false);
    assert.equal(v(paddleHeader(s, { ts: NOW - 301 })), false, 'older than 5 minutes');
    assert.equal(v(paddleHeader(s, { ts: NOW - 299 })), true);
    assert.equal(v(paddleHeader(s, { ts: NOW + 301 })), false, 'more than 5 minutes ahead');
    // Merchant-side rotation: two h1 values, only one made with our secret.
    assert.equal(v(paddleHeader(s, { extra: [hmac('old-secret', `${NOW}:${s}`)] })), true);
    // Our-side rotation: two configured secrets, the header made with the old one.
    assert.equal(v(paddleHeader(s, { secret: 'old-secret' }), [PADDLE_SECRET, 'old-secret']), true);
    // The body is part of the signature.
    assert.equal(verifyPaddleSignature({ header: paddleHeader(s), body: Buffer.from(`${s} `), secrets: [PADDLE_SECRET], now: NOW }), false);
    for (const bad of [undefined, '', 'h1=abc', `ts=${NOW}`, `ts=${NOW};ts=${NOW};h1=${hmac(PADDLE_SECRET, `${NOW}:${s}`)}`, `ts=abc;h1=${'0'.repeat(64)}`]) {
      assert.equal(v(bad), false, String(bad));
    }
  });

  test('stripe: valid, invalid, old, future, rotated, v0 ignored', () => {
    const v = (header, secrets = [STRIPE_SECRET], now = NOW) => verifyStripeSignature({ header, body, secrets, now });
    assert.equal(v(stripeHeader(s)), true);
    assert.equal(v(stripeHeader(s, { secret: 'other' })), false);
    assert.equal(v(stripeHeader(s, { t: NOW - 301 })), false);
    assert.equal(v(stripeHeader(s, { t: NOW + 301 })), false);
    assert.equal(v(stripeHeader(s, { extra: [hmac('old-secret', `${NOW}.${s}`)] })), true);
    assert.equal(v(stripeHeader(s, { secret: 'old-secret' }), [STRIPE_SECRET, 'old-secret']), true);
    assert.equal(v(`t=${NOW},v0=${hmac(STRIPE_SECRET, `${NOW}.${s}`)}`), false, 'v0 alone is not accepted');
    // A Paddle-style header is not a Stripe header.
    assert.equal(v(paddleHeader(s, { secret: STRIPE_SECRET })), false);
  });

  test('interpretation of the documented shapes', () => {
    assert.deepEqual(interpretPaddle(fixture('paddle-transaction-completed'), { priceId: PRICE }), {
      kind: 'purchase',
      type: 'transaction.completed',
      orderKeys: ['paddle:txn_01jbx3dq8w5h2k9m4n7p0r3t6v'],
      customerId: 'ctm_01jbx3cz1a4s7d0f3g6h9j2k5l',
    });
    const adj = fixture('paddle-adjustment-created');
    assert.equal(interpretPaddle(adj, { priceId: PRICE }).kind, 'ignore', 'pending_approval');
    adj.data.status = 'approved';
    assert.deepEqual(interpretPaddle(adj, { priceId: PRICE }).orderKeys, ['paddle:txn_01jbx3dq8w5h2k9m4n7p0r3t6v']);
    adj.data.action = 'credit';
    assert.equal(interpretPaddle(adj, { priceId: PRICE }).kind, 'ignore');
    const cs = interpretStripe(fixture('stripe-checkout-session-completed'));
    assert.deepEqual(cs.orderKeys, ['stripe:cs_test_a1B2c3D4e5F6g7H8i9J0k1L2m3N4o5P6q7R8s9T0u1V2w3X4y5Z6', 'stripe:pi_3QBx7mE5valet0000000001']);
    assert.equal(cs.email, BUYER);
    assert.deepEqual(interpretStripe(fixture('stripe-charge-refunded')).orderKeys, ['stripe:pi_3QBx7mE5valet0000000001']);
  });
});

describe('paddle webhook', () => {
  test('purchase issues one license, maps the transaction, emails it once with the file attached', async () => {
    const t = await setup();
    try {
      const r = await t.paddle(fixture('paddle-transaction-completed'));
      assert.equal(r.status, 200);
      const data = t.data();
      const ids = Object.keys(data.licenses);
      assert.equal(ids.length, 1);
      const id = ids[0];
      assert.deepEqual(data.orders, { 'paddle:txn_01jbx3dq8w5h2k9m4n7p0r3t6v': { license_id: id, created_at: NOW } });
      assert.equal(data.licenses[id].emailed_at, NOW);
      assert.equal(data.licenses[id].max_activations, 3);
      assert.equal(data.licenses[id].updates_until - data.licenses[id].issued_at, 365 * 86_400);
      assert.ok(!JSON.stringify(data).includes(BUYER), 'the store holds no email');

      const [lookup] = t.fetch.paddleCalls();
      assert.equal(lookup.url, 'https://sandbox-api.paddle.com/customers/ctm_01jbx3cz1a4s7d0f3g6h9j2k5l');
      assert.equal(lookup.init.headers.Authorization, `Bearer ${PADDLE_API_KEY}`);

      const resend = t.fetch.resendCalls();
      assert.equal(resend.length, 1);
      assert.equal(resend[0].init.method, 'POST');
      assert.equal(resend[0].init.headers.Authorization, `Bearer ${RESEND_KEY}`);
      const { body, payload } = sentEmail(resend[0]);
      assert.deepEqual(body.to, [BUYER]);
      assert.equal(body.from, 'Valet <licente@valetapp.ro>');
      assert.equal(body.attachments[0].filename, ATTACHMENT_NAME);
      assert.equal(payload.license_id, id);
      assert.equal(payload.email, BUYER);
      assert.equal(payload.product, 'valet-pro');
      assert.match(body.text, /deschide Valet → Setări → Valet Pro → Am o licență/);
      assert.match(body.text, /open Valet → Settings → Valet Pro → I have a license/);
      assert.ok(!body.text.includes(id), 'the email does not present the license id as a support code');

      assert.equal((await t.activate(id)).status, 200, 'the issued license activates');
    } finally {
      await t.close();
    }
  });

  test('production Paddle API base without the sandbox flag', async () => {
    const t = await setup({ sandbox: false });
    try {
      assert.equal((await t.paddle(fixture('paddle-transaction-completed'))).status, 200);
      assert.match(t.fetch.paddleCalls()[0].url, /^https:\/\/api\.paddle\.com\/customers\//);
    } finally {
      await t.close();
    }
  });

  test('a duplicate delivery issues nothing and sends nothing', async () => {
    const t = await setup();
    try {
      const event = fixture('paddle-transaction-completed');
      assert.equal((await t.paddle(event)).status, 200);
      t.clock.now += 60;
      assert.equal((await t.paddle(event)).status, 200);
      assert.equal(Object.keys(t.data().licenses).length, 1);
      assert.equal(t.fetch.resendCalls().length, 1);
      assert.equal(t.fetch.paddleCalls().length, 1, 'no second customer lookup');
    } finally {
      await t.close();
    }
  });

  test('Resend failure answers 502; the redelivery sends the same license', async () => {
    const t = await setup({ fetch: mockFetch({ resendFailures: 1 }) });
    try {
      const event = fixture('paddle-transaction-completed');
      const first = await t.paddle(event);
      assert.equal(first.status, 502);
      assert.deepEqual(first.body, { error: 'email_failed' });
      const after = t.data();
      const [id] = Object.keys(after.licenses);
      assert.equal(after.licenses[id].emailed_at, undefined);
      assert.equal(after.orders['paddle:txn_01jbx3dq8w5h2k9m4n7p0r3t6v'].license_id, id, 'mapped before the email');

      t.clock.now += 120;
      const second = await t.paddle(event);
      assert.equal(second.status, 200);
      const data = t.data();
      assert.deepEqual(Object.keys(data.licenses), [id], 'no second license');
      assert.equal(data.licenses[id].emailed_at, NOW + 120);
      const resend = t.fetch.resendCalls();
      assert.equal(resend.length, 2);
      const a = sentEmail(resend[0]);
      const b = sentEmail(resend[1]);
      assert.equal(b.payload.license_id, id);
      assert.deepEqual(b.envelope, a.envelope, 'same license file, same signature');

      t.clock.now += 60;
      assert.equal((await t.paddle(event)).status, 200);
      assert.equal(t.fetch.resendCalls().length, 2, 'nothing after the email went out');
    } finally {
      await t.close();
    }
  });

  test('Paddle customer API failure answers 502 and issues nothing', async () => {
    const t = await setup({ fetch: mockFetch({ paddleStatus: 503 }) });
    try {
      const r = await t.paddle(fixture('paddle-transaction-completed'));
      assert.equal(r.status, 502);
      assert.deepEqual(r.body, { error: 'merchant_api' });
      assert.equal(fs.existsSync(t.file), false);
      assert.equal(t.fetch.resendCalls().length, 0);
    } finally {
      await t.close();
    }
  });

  test('approved refund revokes; validate and activate then answer 410', async () => {
    const t = await setup();
    try {
      await t.paddle(fixture('paddle-transaction-completed'));
      const [id] = Object.keys(t.data().licenses);
      assert.equal((await t.activate(id)).status, 200);

      const adj = fixture('paddle-adjustment-created');
      assert.equal((await t.paddle(adj)).status, 200);
      assert.equal(t.data().licenses[id].revoked, false, 'pending_approval does not revoke');

      adj.event_type = 'adjustment.updated';
      adj.data.status = 'approved';
      t.clock.now += 3600;
      assert.equal((await t.paddle(adj)).status, 200);
      const rec = t.data().licenses[id];
      assert.equal(rec.revoked, true);
      assert.equal(rec.revoked_at, NOW + 3600);
      assert.equal(rec.revoked_reason, 'refund');
      const v = await t.validate(id);
      assert.equal(v.status, 410);
      assert.deepEqual(v.body, { error: 'revoked' });
      assert.equal((await t.activate(id, machine(2))).status, 410);
      assert.equal((await t.paddle(adj)).status, 200, 'repeated refund is fine');
      assert.equal(t.fetch.resendCalls().length, 1);
    } finally {
      await t.close();
    }
  });

  test('refund before the purchase event: the purchase then issues nothing', async () => {
    const t = await setup();
    try {
      const adj = fixture('paddle-adjustment-created');
      adj.data.status = 'approved';
      assert.equal((await t.paddle(adj)).status, 200);
      assert.equal((await t.paddle(fixture('paddle-transaction-completed'))).status, 200);
      const data = t.data();
      assert.deepEqual(data.licenses, {});
      assert.equal(data.orders['paddle:txn_01jbx3dq8w5h2k9m4n7p0r3t6v'].license_id, null);
      assert.equal(t.fetch.calls.length, 0);
    } finally {
      await t.close();
    }
  });

  test('another price is ignored', async () => {
    const t = await setup();
    try {
      const event = fixture('paddle-transaction-completed');
      for (const it of event.data.items) {
        it.price_id = 'pri_01other0000000000000000';
        it.price.id = 'pri_01other0000000000000000';
      }
      const r = await t.paddle(event);
      assert.equal(r.status, 200);
      assert.equal(fs.existsSync(t.file), false);
      assert.equal(t.fetch.calls.length, 0);
    } finally {
      await t.close();
    }
  });

  test('unknown event types answer 200 and change nothing', async () => {
    const t = await setup();
    try {
      for (const type of ['customer.created', 'transaction.paid', 'subscription.created']) {
        const r = await t.paddle({ event_id: 'evt_1', event_type: type, occurred_at: '2026-10-29T00:00:00Z', data: { id: 'x' } });
        assert.equal(r.status, 200, type);
      }
      assert.equal(fs.existsSync(t.file), false);
      assert.equal(t.fetch.calls.length, 0);
      assert.ok(t.logs.every((e) => e.event === undefined || e.event === 'other'), 'unknown types are logged as other');
    } finally {
      await t.close();
    }
  });

  test('bad signature answers 400 and changes nothing; malformed signed event answers 400', async () => {
    const t = await setup();
    try {
      const body = JSON.stringify(fixture('paddle-transaction-completed'));
      const r = await t.raw('/v1/webhook/paddle', body, { 'Paddle-Signature': paddleHeader(body, { secret: 'wrong' }) });
      assert.equal(r.status, 400);
      assert.deepEqual(r.body, { error: 'bad_signature' });
      assert.equal((await t.raw('/v1/webhook/paddle', body, {})).status, 400, 'no header');
      const old = await t.raw('/v1/webhook/paddle', body, { 'Paddle-Signature': paddleHeader(body, { ts: NOW - 600 }) });
      assert.equal(old.status, 400);
      const notJson = '{not json';
      assert.deepEqual((await t.raw('/v1/webhook/paddle', notJson, { 'Paddle-Signature': paddleHeader(notJson) })).body, { error: 'bad_request' });
      const event = fixture('paddle-transaction-completed');
      delete event.data.customer_id;
      assert.deepEqual((await t.paddle(event)).body, { error: 'bad_event' });
      assert.equal(fs.existsSync(t.file), false);
      assert.equal(t.fetch.calls.length, 0);
    } finally {
      await t.close();
    }
  });

  test('the disabled merchant answers 404; wrong method 405; body over 64 KB 413', async () => {
    const t = await setup();
    try {
      const event = fixture('stripe-checkout-session-completed');
      const r = await t.stripe(event);
      assert.equal(r.status, 404);
      assert.deepEqual(r.body, { error: 'not_found' });
      assert.equal((await globalThis.fetch(`${t.base}/v1/webhook/paddle`)).status, 405);
      const big = JSON.stringify({ event_type: 'x', pad: 'x'.repeat(70 * 1024) });
      assert.equal((await t.raw('/v1/webhook/paddle', big, { 'Paddle-Signature': paddleHeader(big) })).status, 413);
    } finally {
      await t.close();
    }
  });

  test('concurrent webhook deliveries over the cap answer 503 with Retry-After', async () => {
    let release;
    const hold = new Promise((resolve) => {
      release = resolve;
    });
    const t = await setup({ fetch: mockFetch({ hold }), maxConcurrentWebhooks: 1 });
    try {
      const first = t.paddle(fixture('paddle-transaction-completed'));
      // Wait until the first delivery is inside the Resend call.
      while (t.fetch.resendCalls().length === 0) await new Promise((r) => setTimeout(r, 5));
      const second = await t.paddle({ event_type: 'customer.created', data: {} });
      assert.equal(second.status, 503);
      assert.equal(second.headers.get('retry-after'), '10');
      release();
      assert.equal((await first).status, 200);
      assert.equal((await t.paddle({ event_type: 'customer.created', data: {} })).status, 200);
    } finally {
      await t.close();
    }
  });

  test('webhook logs hold event type and outcome, no ids or emails', async () => {
    const t = await setup();
    try {
      await t.paddle(fixture('paddle-transaction-completed'));
      const [id] = Object.keys(t.data().licenses);
      const text = JSON.stringify(t.logs);
      for (const s of [id, BUYER, 'txn_', 'ctm_', '127.0.0.1', PADDLE_SECRET, RESEND_KEY]) assert.ok(!text.includes(s), s);
      const entry = t.logs.find((e) => e.route === '/v1/webhook/paddle');
      assert.equal(entry.event, 'transaction.completed');
      assert.equal(entry.outcome, 'issued');
      assert.equal(entry.status, 200);
    } finally {
      await t.close();
    }
  });
});

describe('stripe webhook', () => {
  test('paid session with metadata.product issues, maps session and payment intent, emails once', async () => {
    const t = await setup({ merchant: 'stripe' });
    try {
      const r = await t.stripe(fixture('stripe-checkout-session-completed'));
      assert.equal(r.status, 200);
      const data = t.data();
      const [id] = Object.keys(data.licenses);
      assert.deepEqual(Object.keys(data.orders).sort(), [
        'stripe:cs_test_a1B2c3D4e5F6g7H8i9J0k1L2m3N4o5P6q7R8s9T0u1V2w3X4y5Z6',
        'stripe:pi_3QBx7mE5valet0000000001',
      ]);
      for (const o of Object.values(data.orders)) assert.equal(o.license_id, id);
      assert.equal(t.fetch.calls.length, 1, 'only Resend, no merchant API call');
      const { body, payload } = sentEmail(t.fetch.resendCalls()[0]);
      assert.deepEqual(body.to, [BUYER]);
      assert.equal(payload.license_id, id);

      assert.equal((await t.stripe(fixture('stripe-checkout-session-completed'))).status, 200);
      assert.equal(t.fetch.resendCalls().length, 1, 'duplicate sends nothing');
      assert.equal(Object.keys(t.data().licenses).length, 1);
    } finally {
      await t.close();
    }
  });

  test('charge.refunded finds the license through the payment intent and revokes it', async () => {
    const t = await setup({ merchant: 'stripe' });
    try {
      await t.stripe(fixture('stripe-checkout-session-completed'));
      const [id] = Object.keys(t.data().licenses);
      assert.equal((await t.activate(id)).status, 200);
      assert.equal((await t.stripe(fixture('stripe-charge-refunded'))).status, 200);
      assert.equal(t.data().licenses[id].revoked, true);
      assert.equal((await t.validate(id)).status, 410);
    } finally {
      await t.close();
    }
  });

  test('Resend failure then redelivery: one license, email sent', async () => {
    const t = await setup({ merchant: 'stripe', fetch: mockFetch({ resendFailures: 2 }) });
    try {
      const event = fixture('stripe-checkout-session-completed');
      assert.equal((await t.stripe(event)).status, 502);
      assert.equal((await t.stripe(event)).status, 502);
      assert.equal((await t.stripe(event)).status, 200);
      const data = t.data();
      assert.equal(Object.keys(data.licenses).length, 1);
      assert.equal(t.fetch.resendCalls().length, 3);
      assert.equal(Object.values(data.licenses)[0].emailed_at, NOW);
    } finally {
      await t.close();
    }
  });

  test('unpaid session, missing metadata.product and unknown types are ignored', async () => {
    const t = await setup({ merchant: 'stripe' });
    try {
      const unpaid = fixture('stripe-checkout-session-completed');
      unpaid.data.object.payment_status = 'unpaid';
      const other = fixture('stripe-checkout-session-completed');
      other.data.object.metadata = {};
      const unknown = { id: 'evt_x', object: 'event', type: 'payment_intent.created', data: { object: { id: 'pi_x' } } };
      for (const e of [unpaid, other, unknown]) assert.equal((await t.stripe(e)).status, 200);
      assert.equal(fs.existsSync(t.file), false);
      assert.equal(t.fetch.calls.length, 0);
    } finally {
      await t.close();
    }
  });

  test('async payment success issues like a paid session', async () => {
    const t = await setup({ merchant: 'stripe' });
    try {
      const pending = fixture('stripe-checkout-session-completed');
      pending.data.object.payment_status = 'unpaid';
      assert.equal((await t.stripe(pending)).status, 200);
      const later = fixture('stripe-checkout-session-completed');
      later.type = 'checkout.session.async_payment_succeeded';
      assert.equal((await t.stripe(later)).status, 200);
      assert.equal(Object.keys(t.data().licenses).length, 1);
      assert.equal(t.fetch.resendCalls().length, 1);
    } finally {
      await t.close();
    }
  });

  test('the paddle route is 404 when stripe is configured, and both are 404 without a merchant', async () => {
    const t = await setup({ merchant: 'stripe' });
    try {
      assert.equal((await t.paddle(fixture('paddle-transaction-completed'))).status, 404);
    } finally {
      await t.close();
    }
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'valet-license-nomerchant-'));
    const s = http.createServer(createHandler({ store: createJsonFileStore(path.join(dir, 's.json')), key, log: () => {} }));
    await new Promise((resolve) => s.listen(0, '127.0.0.1', resolve));
    try {
      for (const m of ['paddle', 'stripe']) {
        const res = await globalThis.fetch(`http://127.0.0.1:${s.address().port}/v1/webhook/${m}`, { method: 'POST', body: '{}' });
        assert.equal(res.status, 404, m);
      }
    } finally {
      await new Promise((resolve) => s.close(resolve));
    }
  });
});
