// Merchant webhooks (docs/LICENSING.md, "License service"): Paddle Billing or Stripe.
//
// Order of work for every delivery:
//   1. signature on the raw body (HMAC-SHA256, constant-time compare, 5-minute tolerance,
//      any of several signatures in the header may match: secret rotation);
//   2. JSON parse and event interpretation (purchase, refund, dispute, dispute
//      information, or ignore);
//   3. state change, serialized for all webhook deliveries in this process.
//
// Idempotency: the store maps every merchant order id to the license id before the
// email is sent. A redelivered purchase whose email was not sent yet sends the same
// license again (same id, same dates, signed again); once `emailed_at` is set, a
// redelivery changes nothing. A failed email answers 502 so the merchant redelivers.
// The email address is never stored: it is read from the event (Stripe) or from the
// Paddle customer API on every delivery that needs it.
//
// Disputes (Vlad, 2026-10-29): a final chargeback (Paddle adjustment `action = chargeback`)
// or a Stripe dispute (`charge.dispute.created`, not an inquiry) revokes the license and
// marks it `disputed`. Warnings and inquiries change nothing. A reversed or won dispute
// is recorded in the license history and logged; it does not reactivate the license
// (only `undispute.mjs` does, by hand).

import crypto from 'node:crypto';
import { issueLicense, resignLicense, PRODUCT } from './license.mjs';
import { registerLicense, updateRecord, addHistory } from './store.mjs';
import { sendLicenseEmail, MailError } from './mail.mjs';
import { paddleCustomerEmail, stripeChargePaymentIntent, MerchantApiError, EMAIL, STRIPE_CHARGE } from './merchant.mjs';

export { PADDLE_API, PADDLE_SANDBOX_API } from './merchant.mjs';
export const TOLERANCE_SECONDS = 300;
export const MERCHANTS = ['paddle', 'stripe'];

/** Event types that appear in logs by name; anything else is logged as "other". */
export const KNOWN_EVENT_TYPES = new Set([
  'transaction.completed',
  'adjustment.created',
  'adjustment.updated',
  'checkout.session.completed',
  'checkout.session.async_payment_succeeded',
  'charge.refunded',
  'charge.dispute.created',
  'charge.dispute.updated',
  'charge.dispute.closed',
]);

// ---------------------------------------------------------------------------
// Signatures

const HEX64 = /^[0-9a-f]{64}$/;

function parseHeader(header, separator) {
  const fields = new Map();
  for (const part of String(header).split(separator)) {
    const i = part.indexOf('=');
    if (i <= 0) continue;
    const k = part.slice(0, i).trim();
    const v = part.slice(i + 1).trim();
    if (!fields.has(k)) fields.set(k, []);
    fields.get(k).push(v);
  }
  return fields;
}

function checkSignatures({ timestamp, candidates, signedPrefix, body, secrets, now }) {
  if (typeof timestamp !== 'string' || !/^\d{1,12}$/.test(timestamp)) return false;
  if (Math.abs(now - Number(timestamp)) > TOLERANCE_SECONDS) return false;
  const given = candidates.filter((s) => HEX64.test(s)).map((s) => Buffer.from(s, 'hex'));
  if (given.length === 0) return false;
  const signed = Buffer.concat([Buffer.from(signedPrefix, 'utf8'), body]);
  let ok = false;
  for (const secret of secrets) {
    const expected = crypto.createHmac('sha256', secret).update(signed).digest();
    for (const g of given) {
      // No early exit: the time taken doesn't depend on which candidate matched.
      if (crypto.timingSafeEqual(expected, g)) ok = true;
    }
  }
  return ok;
}

/**
 * Paddle Billing: `Paddle-Signature: ts=<unix>;h1=<hex>[;h1=<hex>…]`,
 * HMAC-SHA256 over `<ts>:<raw body>` keyed with the notification destination secret.
 */
export function verifyPaddleSignature({ header, body, secrets, now }) {
  if (typeof header !== 'string' || header.length > 2048) return false;
  const f = parseHeader(header, ';');
  const ts = f.get('ts');
  if (!ts || ts.length !== 1) return false;
  return checkSignatures({ timestamp: ts[0], candidates: f.get('h1') ?? [], signedPrefix: `${ts[0]}:`, body, secrets, now });
}

/**
 * Stripe: `Stripe-Signature: t=<unix>,v1=<hex>[,v1=<hex>…][,v0=…]`,
 * HMAC-SHA256 over `<t>.<raw body>` keyed with the endpoint secret (`whsec_…`, used as is).
 * Only `v1` is accepted.
 */
export function verifyStripeSignature({ header, body, secrets, now }) {
  if (typeof header !== 'string' || header.length > 2048) return false;
  const f = parseHeader(header, ',');
  const t = f.get('t');
  if (!t || t.length !== 1) return false;
  return checkSignatures({ timestamp: t[0], candidates: f.get('v1') ?? [], signedPrefix: `${t[0]}.`, body, secrets, now });
}

// ---------------------------------------------------------------------------
// Event interpretation:
//   { kind: 'purchase' | 'refund' | 'dispute' | 'dispute_info' | 'ignore', type, orderKeys, … }

export class BadEvent extends Error {
  constructor(message) {
    super(message);
    this.name = 'BadEvent';
  }
}

const PADDLE_ID = /^[a-z]{3}_[a-z0-9]{1,64}$/;
const STRIPE_ID = /^[a-z]{2,8}_[A-Za-z0-9_]{1,250}$/;

const ignore = (type, reason) => ({ kind: 'ignore', type, reason });

export function interpretPaddle(event, { priceId }) {
  const type = event?.event_type;
  const d = event?.data;
  if (type === 'transaction.completed') {
    if (!d || typeof d.id !== 'string' || !PADDLE_ID.test(d.id)) throw new BadEvent('transaction id');
    if (d.status !== undefined && d.status !== 'completed') return ignore(type, 'status');
    const items = Array.isArray(d.items) ? d.items : [];
    const ours = items.some((it) => it?.price?.id === priceId || it?.price_id === priceId);
    if (!ours) return ignore(type, 'other_product');
    if (typeof d.customer_id !== 'string' || !/^ctm_[a-z0-9]{1,64}$/.test(d.customer_id)) {
      throw new BadEvent('customer id');
    }
    return { kind: 'purchase', type, orderKeys: [`paddle:${d.id}`], customerId: d.customer_id };
  }
  if (type === 'adjustment.created' || type === 'adjustment.updated') {
    const action = d?.action;
    let ev;
    if (action === 'refund' && d?.status === 'approved') {
      ev = { kind: 'refund', type };
    } else if (action === 'chargeback') {
      // Paddle creates chargeback adjustments itself; act on any status except a
      // rejected or reversed one (the reversal also arrives as `chargeback_reverse`).
      if (d.status === 'rejected' || d.status === 'reversed') return ignore(type, 'chargeback_status');
      ev = { kind: 'dispute', type };
    } else if (action === 'chargeback_reverse') {
      ev = { kind: 'dispute_info', type, action: 'dispute_reversed' };
    } else if (action === 'chargeback_warning') {
      return ignore(type, 'chargeback_warning');
    } else {
      return ignore(type, 'not_approved_refund');
    }
    if (typeof d.transaction_id !== 'string' || !PADDLE_ID.test(d.transaction_id)) {
      throw new BadEvent('transaction id');
    }
    return { ...ev, orderKeys: [`paddle:${d.transaction_id}`] };
  }
  return ignore(type, 'event_type');
}

export function interpretStripe(event) {
  const type = event?.type;
  const o = event?.data?.object;
  if (type === 'checkout.session.completed' || type === 'checkout.session.async_payment_succeeded') {
    if (!o || typeof o.id !== 'string' || !STRIPE_ID.test(o.id)) throw new BadEvent('session id');
    if (o.payment_status !== 'paid') return ignore(type, 'unpaid');
    if (o.metadata?.product !== PRODUCT) return ignore(type, 'other_product');
    const email = o.customer_details?.email;
    if (typeof email !== 'string' || !EMAIL.test(email)) throw new BadEvent('customer email');
    const orderKeys = [`stripe:${o.id}`];
    if (typeof o.payment_intent === 'string' && STRIPE_ID.test(o.payment_intent)) {
      orderKeys.push(`stripe:${o.payment_intent}`);
    }
    return { kind: 'purchase', type, orderKeys, email };
  }
  if (type === 'charge.refunded') {
    const pi = typeof o?.payment_intent === 'string' ? o.payment_intent : o?.payment_intent?.id;
    if (typeof pi !== 'string' || !STRIPE_ID.test(pi)) return ignore(type, 'no_payment_intent');
    if (!(o.amount_refunded > 0) && o.refunded !== true) return ignore(type, 'nothing_refunded');
    return { kind: 'refund', type, orderKeys: [`stripe:${pi}`] };
  }
  if (type === 'charge.dispute.created' || type === 'charge.dispute.updated' || type === 'charge.dispute.closed') {
    if (!o || typeof o.status !== 'string') throw new BadEvent('dispute status');
    const pi = typeof o.payment_intent === 'string' ? o.payment_intent : o.payment_intent?.id;
    const charge = typeof o.charge === 'string' ? o.charge : o.charge?.id;
    const target = {};
    if (typeof pi === 'string' && STRIPE_ID.test(pi)) target.orderKeys = [`stripe:${pi}`];
    else if (typeof charge === 'string' && STRIPE_CHARGE.test(charge)) target.chargeId = charge;
    else throw new BadEvent('dispute payment');

    if (type === 'charge.dispute.closed') {
      if (o.status === 'won') return { kind: 'dispute_info', type, action: 'dispute_reversed', ...target };
      if (o.status === 'lost') return { kind: 'dispute_info', type, action: 'dispute_lost', ...target };
      return ignore(type, 'dispute_closed_status');
    }
    if (type === 'charge.dispute.updated') {
      // Only the escalation of an inquiry into a dispute; other updates (evidence
      // submitted, …) must not revoke again a license that undispute.mjs restored.
      const before = event?.data?.previous_attributes?.status;
      if (typeof before !== 'string' || !before.startsWith('warning_')) return ignore(type, 'dispute_update');
    }
    // Inquiries (`warning_needs_response`, `warning_under_review`, `warning_closed`) are not chargebacks.
    if (o.status.startsWith('warning_')) return ignore(type, 'dispute_inquiry');
    if (!['needs_response', 'under_review', 'lost'].includes(o.status)) return ignore(type, 'dispute_status');
    return { kind: 'dispute', type, ...target };
  }
  return ignore(type, 'event_type');
}

// ---------------------------------------------------------------------------
// Processor

/**
 * @param {object} o
 * @param {'paddle'|'stripe'} o.merchant
 * @param {string[]} o.secrets webhook secrets (one, or two during a rotation on our side)
 * @param {object} o.store store with get/put/getOrder/putOrder
 * @param {object} o.key signing key
 * @param {(id: string, fn: Function) => Promise} o.withLicenseLock per-license lock shared with the activation API
 * @param {() => number} o.now Unix seconds
 * @param {Function} [o.fetch]
 * @param {{apiKey: string, from: string, replyTo?: string}} o.mail Resend settings
 * @param {{apiKey: string, priceId: string, sandbox?: boolean}} [o.paddle]
 * @param {{apiKey: string}} [o.stripe] Stripe API key (restricted, read): only needed for a
 *   dispute whose object carries a charge id and no payment intent
 * @returns {(input: {body: Buffer, headers: object}) => Promise<{status, body, outcome, type?}>}
 */
export function createWebhookProcessor({ merchant, secrets, store, key, withLicenseLock, now, fetch: fetchFn = fetch, mail, paddle, stripe }) {
  if (!MERCHANTS.includes(merchant)) throw new Error('merchant must be paddle or stripe');
  if (!Array.isArray(secrets) || secrets.length === 0 || secrets.some((s) => typeof s !== 'string' || !s)) {
    throw new Error('webhook secret missing');
  }
  if (typeof store.getOrder !== 'function' || typeof store.putOrder !== 'function') {
    throw new Error('store must implement getOrder and putOrder for the webhook');
  }
  if (!mail?.apiKey || !mail?.from) throw new Error('mail settings missing');
  if (merchant === 'paddle' && (!paddle?.apiKey || !paddle?.priceId)) throw new Error('paddle settings missing');

  let chain = Promise.resolve();
  function serialized(fn) {
    const next = chain.then(fn, fn);
    chain = next.catch(() => {});
    return next;
  }

  async function findOrder(keys) {
    for (const k of keys) {
      const order = await store.getOrder(k);
      if (order) return order;
    }
    return null;
  }

  async function purchase(ev) {
    const t = now();
    const order = await findOrder(ev.orderKeys);
    if (order && order.license_id === null) return { status: 200, outcome: 'refunded_before_purchase' };
    let record = null;
    if (order) {
      record = await store.get(order.license_id);
      if (!record) throw new Error('order points to a missing license');
      if (record.emailed_at) return { status: 200, outcome: 'duplicate' };
      if (record.revoked) return { status: 200, outcome: 'revoked' };
    }

    let email;
    try {
      email = ev.email ?? (await paddleCustomerEmail(fetchFn, paddle, ev.customerId));
    } catch (err) {
      if (err instanceof MerchantApiError) return { status: 502, body: { error: 'merchant_api' }, outcome: 'customer_lookup_failed' };
      throw err;
    }

    // Every key of the purchase, and the Paddle customer id (never the email), so that
    // resend.mjs and reissue.mjs can read the email from the merchant later.
    const orderEntry = (licenseId) => ({
      license_id: licenseId,
      created_at: t,
      order_keys: ev.orderKeys,
      ...(ev.customerId ? { customer_id: ev.customerId } : {}),
    });
    let issued;
    let outcome;
    if (!record) {
      issued = issueLicense({ email, issuedAt: t }, key);
      await registerLicense(store, issued.payload);
      for (const k of ev.orderKeys) await store.putOrder(k, orderEntry(issued.payload.license_id));
      outcome = 'issued';
    } else {
      issued = resignLicense(record, email, key);
      for (const k of ev.orderKeys) {
        if (!(await store.getOrder(k))) await store.putOrder(k, orderEntry(record.license_id));
      }
      outcome = 'resent';
    }

    try {
      await sendLicenseEmail({ fetch: fetchFn, ...mail }, { to: email, payload: issued.payload, envelope: issued.envelope });
    } catch (err) {
      if (err instanceof MailError) return { status: 502, body: { error: 'email_failed' }, outcome: 'email_failed' };
      throw err;
    }

    const id = issued.payload.license_id;
    const sentAt = now();
    await withLicenseLock(id, () =>
      updateRecord(store, id, (r) => {
        if (r) r.emailed_at = sentAt;
      }),
    );
    return { status: 200, outcome };
  }

  /** Charge id -> order keys, for a Stripe dispute that carries no payment intent. */
  async function disputeOrderKeys(ev) {
    if (ev.orderKeys) return ev.orderKeys;
    if (!stripe?.apiKey) throw new MerchantApiError('stripe api key not configured');
    return [`stripe:${await stripeChargePaymentIntent(fetchFn, stripe, ev.chargeId)}`];
  }

  // Refund and dispute: revoke the license behind the order. A dispute also sets
  // `disputed`, also on a license already revoked by a refund.
  async function revoke(ev, orderKeys) {
    const t = now();
    const reason = ev.kind === 'dispute' ? 'dispute' : 'refund';
    const order = await findOrder(orderKeys);
    if (!order) {
      // Refund or dispute before the purchase event (events can arrive out of order), or an
      // order that never got a license (another product). Remember it so a later purchase
      // event issues nothing.
      const entry = reason === 'dispute' ? { license_id: null, disputed_at: t } : { license_id: null, refunded_at: t };
      for (const k of orderKeys) await store.putOrder(k, entry);
      return { status: 200, outcome: `${reason}_without_license` };
    }
    if (order.license_id === null) return { status: 200, outcome: 'duplicate' };
    const changed = await withLicenseLock(order.license_id, () =>
      updateRecord(store, order.license_id, (r) => {
        if (!r) throw new Error('order points to a missing license');
        if (reason === 'dispute' ? r.disputed === true : r.revoked) return false;
        if (!r.revoked) {
          r.revoked = true;
          r.revoked_at = t;
          r.revoked_reason = reason;
        }
        if (reason === 'dispute') r.disputed = true;
        addHistory(r, { at: t, action: reason === 'dispute' ? 'dispute' : 'revoke', ...(reason === 'refund' ? { reason } : {}) });
        return true;
      }),
    );
    return { status: 200, outcome: changed ? (reason === 'dispute' ? 'disputed' : 'revoked') : 'duplicate' };
  }

  // Reversed, won or lost dispute: history entry only. The license stays as it is.
  async function disputeInfo(ev, orderKeys) {
    const t = now();
    const order = await findOrder(orderKeys);
    if (!order || order.license_id === null) return { status: 200, outcome: `${ev.action}_without_license` };
    await withLicenseLock(order.license_id, () =>
      updateRecord(store, order.license_id, (r) => {
        if (!r) throw new Error('order points to a missing license');
        addHistory(r, { at: t, action: ev.action });
      }),
    );
    return { status: 200, outcome: ev.action };
  }

  async function dispute(ev) {
    let orderKeys;
    try {
      orderKeys = await disputeOrderKeys(ev);
    } catch (err) {
      if (err instanceof MerchantApiError) return { status: 502, body: { error: 'merchant_api' }, outcome: 'charge_lookup_failed' };
      throw err;
    }
    return ev.kind === 'dispute' ? revoke(ev, orderKeys) : disputeInfo(ev, orderKeys);
  }

  const verify = merchant === 'paddle' ? verifyPaddleSignature : verifyStripeSignature;
  const headerName = merchant === 'paddle' ? 'paddle-signature' : 'stripe-signature';

  return async function processWebhook({ body, headers }) {
    if (!verify({ header: headers[headerName], body, secrets, now: now() })) {
      return { status: 400, body: { error: 'bad_signature' }, outcome: 'bad_signature' };
    }
    let event;
    try {
      event = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(body));
    } catch {
      return { status: 400, body: { error: 'bad_request' }, outcome: 'bad_request' };
    }
    let ev;
    try {
      ev = merchant === 'paddle' ? interpretPaddle(event, paddle) : interpretStripe(event);
    } catch (err) {
      if (err instanceof BadEvent) return { status: 400, body: { error: 'bad_event' }, outcome: 'bad_event' };
      throw err;
    }
    const type = KNOWN_EVENT_TYPES.has(ev.type) ? ev.type : 'other';
    if (ev.kind === 'ignore') return { status: 200, outcome: `ignored_${ev.reason}`, type };
    const result = await serialized(() => {
      if (ev.kind === 'purchase') return purchase(ev);
      if (ev.kind === 'refund') return revoke(ev, ev.orderKeys);
      return dispute(ev);
    });
    return { ...result, type };
  };
}
