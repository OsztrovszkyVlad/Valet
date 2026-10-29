// Support helpers for the CLIs. Support identifies a license by the merchant order id
// printed on the buyer's receipt, never by the license id: the license id alone is enough
// to activate, and support runs on public GitHub Issues.
//
//   txn_…        Paddle transaction id       -> store key paddle:txn_…
//   cs_… / pi_…  Stripe checkout session or payment intent -> store key stripe:cs_… / stripe:pi_…

import { UUID_V4 } from './license.mjs';

const PADDLE_TXN = /^txn_[a-z0-9]{1,64}$/;
const STRIPE_ORDER = /^(cs|pi)_[A-Za-z0-9_]{1,250}$/;

/** Store key for a merchant order id, or null when the id has an unknown shape. */
export function orderKeyFor(orderId) {
  if (typeof orderId !== 'string') return null;
  const id = orderId.trim();
  if (PADDLE_TXN.test(id)) return `paddle:${id}`;
  if (STRIPE_ORDER.test(id)) return `stripe:${id}`;
  return null;
}

/**
 * Resolves `--license` or `--order` to a license id. Throws an Error with a message for
 * the terminal (no email, nothing secret) when it can't.
 */
export async function resolveLicenseId(store, { license, order }) {
  if ((license === undefined) === (order === undefined)) {
    throw new Error('give exactly one of --license <uuid> or --order <txn_…|cs_…|pi_…>');
  }
  if (license !== undefined) {
    if (!UUID_V4.test(license)) throw new Error('--license must be a lowercase UUID v4');
    return license;
  }
  const key = orderKeyFor(order);
  if (!key) throw new Error('--order must be a Paddle transaction id (txn_…) or a Stripe checkout session (cs_…) or payment intent (pi_…)');
  const entry = await store.getOrder(key);
  if (!entry) throw new Error('order not found in the store (no webhook delivery recorded for it)');
  if (entry.license_id === null) {
    throw new Error('order has no license: a refund was recorded before any purchase event, or it is not a Valet Pro order');
  }
  return entry.license_id;
}

const day = (s) => (typeof s === 'number' ? new Date(s * 1000).toISOString().slice(0, 10) : '-');

/** Human-readable summary of a store record. No machine ids, no email (the store has none). */
export function describeRecord(record) {
  const machines = Object.values(record.machines ?? {});
  const active = machines.filter((m) => m.active);
  const lines = [
    `license_id: ${record.license_id}`,
    `issued: ${day(record.issued_at)}`,
    `updates_until: ${day(record.updates_until)}`,
    `revoked: ${record.revoked ? `yes, ${day(record.revoked_at)}${record.revoked_reason ? ` (${record.revoked_reason})` : ''}` : 'no'}`,
    `emailed: ${record.emailed_at ? day(record.emailed_at) : 'no'}`,
    `active_machines: ${active.length} of ${record.max_activations}`,
  ];
  machines
    .slice()
    .sort((a, b) => (b.last_seen ?? 0) - (a.last_seen ?? 0))
    .forEach((m, i) => {
      lines.push(`machine ${i + 1}: ${m.active ? 'active' : 'inactive'}, first seen ${day(m.first_seen)}, last seen ${day(m.last_seen)}`);
    });
  lines.push(`resets: ${(record.resets ?? []).length}`);
  return lines.join('\n');
}
