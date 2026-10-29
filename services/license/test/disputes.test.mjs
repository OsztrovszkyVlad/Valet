// Chargebacks and disputes through the webhook (Vlad, 2026-10-29): a final chargeback or
// dispute revokes and marks `disputed`; warnings and inquiries change nothing; reversed or
// won disputes are history entries only.
import { describe, test } from 'node:test';
import assert from 'node:assert/strict';
import http from 'node:http';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { generateKeyPair } from '../lib/signing.mjs';
import { createJsonFileStore } from '../lib/store.mjs';
import { createHandler } from '../lib/handler.mjs';
import { RESEND_URL } from '../lib/mail.mjs';
import { interpretPaddle, interpretStripe } from '../lib/webhook.mjs';

const key = generateKeyPair();
const NOW = 1_793_268_950;
// Test-only secrets.
const PADDLE_SECRET = 'test-paddle-notification-secret';
const STRIPE_SECRET = 'test-stripe-endpoint-secret';
const STRIPE_API_KEY = 'test-stripe-restricted-key';
const PRICE = 'pri_01jbx2valetpro0000000000a';
const TXN = 'txn_01jbx3dq8w5h2k9m4n7p0r3t6v';
const PI = 'pi_3QBx7mE5valet0000000001';
const CHARGE = 'ch_3QBx7mE5valet0000000001';
const machine = (n) => n.toString(16).padStart(32, '0');

const fixture = (name) => JSON.parse(fs.readFileSync(new URL(`./fixtures/${name}.json`, import.meta.url), 'utf8'));
const hmac = (secret, text) => crypto.createHmac('sha256', secret).update(text).digest('hex');

function mockFetch() {
  const calls = [];
  const fn = async (url, init = {}) => {
    calls.push({ url, init });
    if (url === RESEND_URL) return new Response('{"id":"x"}', { status: 200 });
    if (/^https:\/\/sandbox-api\.paddle\.com\/customers\/ctm_[a-z0-9]+$/.test(url)) {
      return new Response(JSON.stringify({ data: { email: 'buyer@example.com' } }), { status: 200 });
    }
    if (url === `https://api.stripe.com/v1/charges/${CHARGE}`) {
      return new Response(JSON.stringify({ id: CHARGE, object: 'charge', payment_intent: PI }), { status: 200 });
    }
    throw new Error(`unexpected fetch ${url}`);
  };
  fn.calls = calls;
  return fn;
}

async function setup({ merchant = 'paddle', stripeApiKey = STRIPE_API_KEY } = {}) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'valet-license-disputes-'));
  const file = path.join(dir, 'store.json');
  const store = createJsonFileStore(file);
  const logs = [];
  const clock = { now: NOW };
  const fetch = mockFetch();
  const webhook = {
    merchant,
    secrets: [merchant === 'paddle' ? PADDLE_SECRET : STRIPE_SECRET],
    mail: { apiKey: 'test-resend-key', from: 'Valet <licente@valetapp.ro>' },
    fetch,
    ...(merchant === 'paddle' ? { paddle: { apiKey: 'test-paddle-api-key', priceId: PRICE, sandbox: true } } : {}),
    ...(merchant === 'stripe' && stripeApiKey ? { stripe: { apiKey: stripeApiKey } } : {}),
  };
  const server = http.createServer(createHandler({ store, key, now: () => clock.now, log: (e) => logs.push(e), webhook }));
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  const base = `http://127.0.0.1:${server.address().port}`;
  const post = async (route, body, headers) => {
    const res = await globalThis.fetch(`${base}${route}`, { method: 'POST', headers: { 'Content-Type': 'application/json', ...headers }, body });
    const text = await res.text();
    return { status: res.status, body: text ? JSON.parse(text) : undefined };
  };
  return {
    clock,
    logs,
    fetch,
    data: () => JSON.parse(fs.readFileSync(file, 'utf8')),
    paddle: (event) => {
      const body = JSON.stringify(event);
      return post('/v1/webhook/paddle', body, { 'Paddle-Signature': `ts=${clock.now};h1=${hmac(PADDLE_SECRET, `${clock.now}:${body}`)}` });
    },
    stripe: (event) => {
      const body = JSON.stringify(event);
      return post('/v1/webhook/stripe', body, { 'Stripe-Signature': `t=${clock.now},v1=${hmac(STRIPE_SECRET, `${clock.now}.${body}`)}` });
    },
    validate: (id, m = machine(1)) => post('/v1/validate', JSON.stringify({ license_id: id, machine: m, app_version: '0.2.0' })),
    close: () => new Promise((resolve) => server.close(resolve)),
  };
}

function paddleAdjustment(action, status = 'approved', type = 'adjustment.created') {
  const adj = fixture('paddle-adjustment-created');
  adj.event_type = type;
  adj.data.action = action;
  adj.data.status = status;
  return adj;
}

/** Stripe dispute event in the documented shape (fields from knowledge, see README checklist). */
function stripeDispute({ type = 'charge.dispute.created', status = 'needs_response', pi = PI, charge = CHARGE, previousStatus } = {}) {
  return {
    id: 'evt_1QDx7mE5valet00000000009',
    object: 'event',
    api_version: '2025-09-30.clover',
    created: NOW,
    livemode: false,
    type,
    data: {
      object: {
        id: 'du_1QDx7mE5valet0000000001',
        object: 'dispute',
        amount: 2900,
        currency: 'eur',
        charge,
        payment_intent: pi,
        reason: 'fraudulent',
        status,
        is_charge_refundable: false,
        livemode: false,
        metadata: {},
      },
      ...(previousStatus ? { previous_attributes: { status: previousStatus } } : {}),
    },
  };
}

describe('dispute interpretation', () => {
  test('paddle adjustment actions', () => {
    const opts = { priceId: PRICE };
    assert.deepEqual(interpretPaddle(paddleAdjustment('chargeback'), opts), { kind: 'dispute', type: 'adjustment.created', orderKeys: [`paddle:${TXN}`] });
    assert.equal(interpretPaddle(paddleAdjustment('chargeback', 'pending_approval'), opts).kind, 'dispute', 'Paddle creates chargebacks itself');
    assert.equal(interpretPaddle(paddleAdjustment('chargeback', 'rejected'), opts).kind, 'ignore');
    assert.deepEqual(interpretPaddle(paddleAdjustment('chargeback_warning'), opts), { kind: 'ignore', type: 'adjustment.created', reason: 'chargeback_warning' });
    assert.deepEqual(interpretPaddle(paddleAdjustment('chargeback_reverse'), opts), {
      kind: 'dispute_info',
      type: 'adjustment.created',
      action: 'dispute_reversed',
      orderKeys: [`paddle:${TXN}`],
    });
    const bad = paddleAdjustment('chargeback');
    delete bad.data.transaction_id;
    assert.throws(() => interpretPaddle(bad, opts), /transaction id/);
  });

  test('stripe dispute events', () => {
    assert.deepEqual(interpretStripe(stripeDispute()), { kind: 'dispute', type: 'charge.dispute.created', orderKeys: [`stripe:${PI}`] });
    assert.deepEqual(interpretStripe(stripeDispute({ pi: null })), { kind: 'dispute', type: 'charge.dispute.created', chargeId: CHARGE });
    for (const status of ['warning_needs_response', 'warning_under_review', 'warning_closed']) {
      assert.equal(interpretStripe(stripeDispute({ status })).reason, 'dispute_inquiry', status);
    }
    assert.equal(interpretStripe(stripeDispute({ type: 'charge.dispute.updated', status: 'under_review', previousStatus: 'needs_response' })).kind, 'ignore');
    assert.equal(interpretStripe(stripeDispute({ type: 'charge.dispute.updated', status: 'needs_response', previousStatus: 'warning_under_review' })).kind, 'dispute', 'inquiry escalated');
    assert.equal(interpretStripe(stripeDispute({ type: 'charge.dispute.closed', status: 'won' })).action, 'dispute_reversed');
    assert.equal(interpretStripe(stripeDispute({ type: 'charge.dispute.closed', status: 'lost' })).action, 'dispute_lost');
    assert.equal(interpretStripe(stripeDispute({ type: 'charge.dispute.closed', status: 'warning_closed' })).kind, 'ignore');
    assert.throws(() => interpretStripe(stripeDispute({ pi: null, charge: null })), /dispute payment/);
  });
});

describe('paddle chargeback', () => {
  test('revokes and marks disputed; validate answers 410; warning and repeat change nothing', async () => {
    const t = await setup();
    try {
      await t.paddle(fixture('paddle-transaction-completed'));
      const [id] = Object.keys(t.data().licenses);
      assert.equal((await t.validate(id)).status, 200);

      assert.equal((await t.paddle(paddleAdjustment('chargeback_warning'))).status, 200);
      assert.equal(t.data().licenses[id].revoked, false, 'a warning is not a chargeback');

      t.clock.now += 3600;
      assert.equal((await t.paddle(paddleAdjustment('chargeback'))).status, 200);
      const rec = t.data().licenses[id];
      assert.equal(rec.revoked, true);
      assert.equal(rec.disputed, true);
      assert.equal(rec.revoked_reason, 'dispute');
      assert.equal(rec.revoked_at, NOW + 3600);
      assert.deepEqual(rec.history, [{ at: NOW + 3600, action: 'dispute' }]);
      const v = await t.validate(id);
      assert.equal(v.status, 410);
      assert.deepEqual(v.body, { error: 'revoked' });

      assert.equal((await t.paddle(paddleAdjustment('chargeback', 'approved', 'adjustment.updated'))).status, 200);
      assert.equal(t.data().licenses[id].history.length, 1, 'repeat is a duplicate');
      const outcomes = t.logs.filter((e) => e.route === '/v1/webhook/paddle').map((e) => e.outcome);
      assert.deepEqual(outcomes, ['issued', 'ignored_chargeback_warning', 'disputed', 'duplicate']);
    } finally {
      await t.close();
    }
  });

  test('chargeback_reverse is logged in the history and does not reactivate', async () => {
    const t = await setup();
    try {
      await t.paddle(fixture('paddle-transaction-completed'));
      const [id] = Object.keys(t.data().licenses);
      await t.paddle(paddleAdjustment('chargeback'));
      t.clock.now += 86_400;
      assert.equal((await t.paddle(paddleAdjustment('chargeback_reverse'))).status, 200);
      const rec = t.data().licenses[id];
      assert.equal(rec.revoked, true);
      assert.equal(rec.disputed, true);
      assert.deepEqual(rec.history.map((h) => h.action), ['dispute', 'dispute_reversed']);
      assert.equal((await t.validate(id)).status, 410);
      assert.equal(t.logs.at(-2).outcome, 'dispute_reversed');
    } finally {
      await t.close();
    }
  });

  test('chargeback after a refund keeps the refund reason and adds disputed', async () => {
    const t = await setup();
    try {
      await t.paddle(fixture('paddle-transaction-completed'));
      const [id] = Object.keys(t.data().licenses);
      await t.paddle(paddleAdjustment('refund'));
      t.clock.now += 60;
      await t.paddle(paddleAdjustment('chargeback'));
      const rec = t.data().licenses[id];
      assert.equal(rec.revoked_reason, 'refund');
      assert.equal(rec.revoked_at, NOW);
      assert.equal(rec.disputed, true);
      assert.deepEqual(rec.history.map((h) => h.action), ['revoke', 'dispute']);
    } finally {
      await t.close();
    }
  });

  test('chargeback before any purchase event: the purchase then issues nothing', async () => {
    const t = await setup();
    try {
      await t.paddle(paddleAdjustment('chargeback'));
      await t.paddle(fixture('paddle-transaction-completed'));
      const data = t.data();
      assert.deepEqual(data.licenses, {});
      assert.equal(data.orders[`paddle:${TXN}`].license_id, null);
      assert.equal(data.orders[`paddle:${TXN}`].disputed_at, NOW);
      assert.equal(t.fetch.calls.length, 0);
    } finally {
      await t.close();
    }
  });
});

describe('stripe dispute', () => {
  test('charge.dispute.created revokes through the payment intent; inquiries are ignored', async () => {
    const t = await setup({ merchant: 'stripe' });
    try {
      await t.stripe(fixture('stripe-checkout-session-completed'));
      const [id] = Object.keys(t.data().licenses);
      assert.equal((await t.validate(id)).status, 200);

      assert.equal((await t.stripe(stripeDispute({ status: 'warning_needs_response' }))).status, 200);
      assert.equal(t.data().licenses[id].revoked, false, 'an inquiry is not a chargeback');

      assert.equal((await t.stripe(stripeDispute())).status, 200);
      const rec = t.data().licenses[id];
      assert.equal(rec.revoked, true);
      assert.equal(rec.disputed, true);
      assert.equal(rec.revoked_reason, 'dispute');
      assert.equal((await t.validate(id)).status, 410);
      assert.equal(t.fetch.calls.filter((c) => c.url.startsWith('https://api.stripe.com')).length, 0, 'payment intent on the dispute: no API call');

      assert.equal((await t.stripe(stripeDispute({ type: 'charge.dispute.closed', status: 'won' }))).status, 200);
      const after = t.data().licenses[id];
      assert.equal(after.revoked, true, 'a won dispute does not reactivate');
      assert.deepEqual(after.history.map((h) => h.action), ['dispute', 'dispute_reversed']);
      const types = t.logs.filter((e) => e.route === '/v1/webhook/stripe').map((e) => e.event);
      assert.deepEqual(types, ['checkout.session.completed', 'charge.dispute.created', 'charge.dispute.created', 'charge.dispute.closed']);
    } finally {
      await t.close();
    }
  });

  test('a dispute with only a charge id reads the payment intent from the charge', async () => {
    const t = await setup({ merchant: 'stripe' });
    try {
      await t.stripe(fixture('stripe-checkout-session-completed'));
      const [id] = Object.keys(t.data().licenses);
      assert.equal((await t.stripe(stripeDispute({ pi: null }))).status, 200);
      assert.equal(t.data().licenses[id].disputed, true);
      const call = t.fetch.calls.find((c) => c.url.startsWith('https://api.stripe.com'));
      assert.equal(call.url, `https://api.stripe.com/v1/charges/${CHARGE}`);
      assert.equal(call.init.headers.Authorization, `Bearer ${STRIPE_API_KEY}`);
    } finally {
      await t.close();
    }
  });

  test('without VALET_STRIPE_API_KEY a charge-only dispute answers 502 (Stripe retries)', async () => {
    const t = await setup({ merchant: 'stripe', stripeApiKey: null });
    try {
      await t.stripe(fixture('stripe-checkout-session-completed'));
      const r = await t.stripe(stripeDispute({ pi: null }));
      assert.equal(r.status, 502);
      assert.deepEqual(r.body, { error: 'merchant_api' });
      assert.equal(Object.values(t.data().licenses)[0].revoked, false);
    } finally {
      await t.close();
    }
  });

  test('an inquiry that escalates (charge.dispute.updated from warning_*) revokes', async () => {
    const t = await setup({ merchant: 'stripe' });
    try {
      await t.stripe(fixture('stripe-checkout-session-completed'));
      const [id] = Object.keys(t.data().licenses);
      await t.stripe(stripeDispute({ status: 'warning_needs_response' }));
      await t.stripe(stripeDispute({ type: 'charge.dispute.updated', status: 'warning_under_review', previousStatus: 'warning_needs_response' }));
      assert.equal(t.data().licenses[id].revoked, false);
      await t.stripe(stripeDispute({ type: 'charge.dispute.updated', status: 'needs_response', previousStatus: 'warning_under_review' }));
      assert.equal(t.data().licenses[id].disputed, true);
    } finally {
      await t.close();
    }
  });

  test('logs carry no ids', async () => {
    const t = await setup({ merchant: 'stripe' });
    try {
      await t.stripe(fixture('stripe-checkout-session-completed'));
      await t.stripe(stripeDispute({ pi: null }));
      const text = JSON.stringify(t.logs);
      for (const s of [PI, CHARGE, 'du_', 'cs_test_', STRIPE_API_KEY]) assert.ok(!text.includes(s), s);
    } finally {
      await t.close();
    }
  });
});
