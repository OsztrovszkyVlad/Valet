// Support actions behind resend.mjs, reissue.mjs and undispute.mjs (Vlad, 2026-10-29).
// The CLIs only parse arguments and print; everything else is here so it can be tested
// with a replaced `fetch`.
//
// The store holds no email. resend and reissue read it from the merchant each time:
// Paddle through the customer id stored on the order (or, for orders recorded before
// that, through the transaction), Stripe through the checkout session.
//
// Errors: ActionRefused (nothing changed; exit 2) and ActionFailed (a merchant or Resend
// call failed; the message says what state the store is in; exit 1). Messages never
// contain an email address or a key.

import { issueLicense, resignLicense } from './license.mjs';
import { registerLicense, updateRecord, addHistory } from './store.mjs';
import { sendLicenseEmail, MailError } from './mail.mjs';
import { paddleCustomerEmail, paddleTransactionCustomer, stripeSession, MerchantApiError } from './merchant.mjs';
import { resolveOrder, merchantOf } from './support.mjs';

export class ActionRefused extends Error {
  constructor(message) {
    super(message);
    this.name = 'ActionRefused';
  }
}

export class ActionFailed extends Error {
  constructor(message) {
    super(message);
    this.name = 'ActionFailed';
  }
}

const day = (s) => new Date(s * 1000).toISOString().slice(0, 10);

async function orderAndRecord(store, orderId) {
  let resolved;
  try {
    resolved = await resolveOrder(store, orderId);
  } catch (err) {
    throw new ActionRefused(err.message);
  }
  const record = await store.get(resolved.entry.license_id);
  if (!record) throw new ActionRefused('the order points to a license that is not in the store');
  return { ...resolved, record };
}

/** Refuses revoked and disputed licenses, naming the reason. */
function refuseUnusable(record, what) {
  if (record.disputed) {
    throw new ActionRefused(`license ${record.license_id} is disputed (chargeback); ${what} refused. After the dispute is settled in the buyer's favour: undispute.mjs --confirm`);
  }
  if (record.revoked) {
    throw new ActionRefused(`license ${record.license_id} is revoked${record.revoked_reason ? ` (${record.revoked_reason})` : ''}; ${what} refused`);
  }
}

/**
 * The buyer's email from the merchant, plus every order key of the purchase that exists
 * in the store (Stripe: the session read also names the payment intent).
 */
async function buyerEmail({ store, key, entry, fetch, paddle, stripe }) {
  const keys = [...new Set([key, ...(entry.order_keys ?? [])])];
  try {
    if (merchantOf(key) === 'paddle') {
      const customerId = entry.customer_id ?? (await paddleTransactionCustomer(fetch, paddle, key.slice('paddle:'.length)));
      return { email: await paddleCustomerEmail(fetch, paddle, customerId), keys };
    }
    const cs = keys.find((k) => k.startsWith('stripe:cs_'));
    if (!cs) {
      throw new ActionRefused(
        'this order entry was recorded without its checkout session id; run the command again with the cs_… id ' +
          '(Stripe dashboard → the payment → Checkout session)',
      );
    }
    const session = await stripeSession(fetch, stripe, cs.slice('stripe:'.length));
    const piKey = session.paymentIntent ? `stripe:${session.paymentIntent}` : null;
    if (piKey && !keys.includes(piKey) && (await store.getOrder(piKey))?.license_id === entry.license_id) keys.push(piKey);
    return { email: session.email, keys };
  } catch (err) {
    if (err instanceof MerchantApiError) throw new ActionFailed(`merchant API: ${err.message}; nothing changed`);
    throw err;
  }
}

async function send({ fetch, mail }, email, issued) {
  await sendLicenseEmail({ fetch, ...mail }, { to: email, payload: issued.payload, envelope: issued.envelope });
}

/**
 * Sends the license of an order again: same license id and dates, signed again for the
 * email the merchant has now. Byte-identical to the original file when that email is
 * unchanged; if the buyer changed it at the merchant, the file differs only in `email`.
 * @returns {Promise<{licenseId: string, firstEmail: boolean}>}
 */
export async function resendLicense({ store, key, orderId, fetch: fetchFn = fetch, mail, paddle, stripe, now }) {
  const { key: orderKey, entry, record } = await orderAndRecord(store, orderId);
  refuseUnusable(record, 'resend');
  const { email } = await buyerEmail({ store, key: orderKey, entry, fetch: fetchFn, paddle, stripe });
  const issued = resignLicense(record, email, key);
  try {
    await send({ fetch: fetchFn, mail }, email, issued);
  } catch (err) {
    if (err instanceof MailError) throw new ActionFailed(`email not sent (${err.message}); nothing changed, run resend.mjs again`);
    throw err;
  }
  const t = now();
  let firstEmail = false;
  await updateRecord(store, record.license_id, (r) => {
    if (!r.emailed_at) {
      r.emailed_at = t;
      firstEmail = true;
    }
    addHistory(r, { at: t, action: 'resend' });
  });
  return { licenseId: record.license_id, firstEmail };
}

/** What reissue would do, for the refusal without --confirm. */
export async function describeReissue({ store, orderId }) {
  const { record } = await orderAndRecord(store, orderId);
  refuseUnusable(record, 'reissue');
  return (
    `would revoke license ${record.license_id} (reason reissued), issue a new license with updates until ` +
    `${day(record.updates_until)} and 3 activations, map the order to it and email it`
  );
}

/**
 * Replaces the license of an order (leaked license): new license id, same updates_until,
 * 3 activations. Order of writes: register the new license, map every order key to it,
 * revoke the old one, send the email. If the email fails the new license stays mapped
 * and resend.mjs sends it.
 * @returns {Promise<{oldId: string, newId: string, emailed: boolean, error?: string}>}
 */
export async function reissueLicense({ store, key, orderId, confirm, fetch: fetchFn = fetch, mail, paddle, stripe, now }) {
  const { key: orderKey, entry, record } = await orderAndRecord(store, orderId);
  refuseUnusable(record, 'reissue');
  if (confirm !== true) throw new ActionRefused(`${await describeReissue({ store, orderId })}. Run again with --confirm.`);

  const { email, keys } = await buyerEmail({ store, key: orderKey, entry, fetch: fetchFn, paddle, stripe });
  const t = now();
  const oldId = record.license_id;
  const issued = issueLicense({ email, issuedAt: t, updatesUntil: record.updates_until }, key);
  const newId = issued.payload.license_id;

  await registerLicense(store, issued.payload);
  await updateRecord(store, newId, (r) => addHistory(r, { at: t, action: 'issued', reissue_of: oldId }));
  for (const k of keys) {
    const e = k === orderKey ? entry : await store.getOrder(k);
    if (!e || e.license_id !== oldId) continue;
    await store.putOrder(k, {
      ...e,
      license_id: newId,
      order_keys: e.order_keys ?? keys,
      history: [...(e.history ?? []), { at: t, action: 'reissue', from: oldId, to: newId }],
    });
  }
  await updateRecord(store, oldId, (r) => {
    if (!r.revoked) {
      r.revoked = true;
      r.revoked_at = t;
      r.revoked_reason = 'reissued';
    }
    addHistory(r, { at: t, action: 'reissued', to: newId });
  });

  try {
    await send({ fetch: fetchFn, mail }, email, issued);
  } catch (err) {
    if (err instanceof MailError) return { oldId, newId, emailed: false, error: err.message };
    throw err;
  }
  const sentAt = now();
  await updateRecord(store, newId, (r) => {
    r.emailed_at = sentAt;
  });
  return { oldId, newId, emailed: true };
}

/**
 * Manual end of a dispute: clears `disputed`, and `revoked` when the dispute was the
 * reason. A license revoked for a refund or a reissue stays revoked. No email.
 * @returns {Promise<{changed: boolean, revoked: boolean, revokedReason?: string}>}
 */
export async function undisputeLicense({ store, licenseId, confirm, now, by }) {
  const record = await store.get(licenseId);
  if (!record) throw new ActionRefused('license not found in the store');
  if (!record.disputed) return { changed: false, revoked: record.revoked === true, revokedReason: record.revoked_reason };
  if (confirm !== true) {
    const after = record.revoked_reason === 'dispute' ? 'the license will be active again' : `the license stays revoked (${record.revoked_reason})`;
    throw new ActionRefused(`license ${licenseId} is disputed; undispute clears the dispute flag and ${after}. Run again with --confirm.`);
  }
  const t = now();
  return updateRecord(store, licenseId, (r) => {
    r.disputed = false;
    if (r.revoked && r.revoked_reason === 'dispute') {
      r.revoked = false;
      delete r.revoked_at;
      delete r.revoked_reason;
    }
    addHistory(r, { at: t, action: 'undispute', ...(by ? { by } : {}) });
    return { changed: true, revoked: r.revoked === true, revokedReason: r.revoked_reason };
  });
}
