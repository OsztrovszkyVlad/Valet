// Merchant API reads. The buyer email is never stored by the service, so every message
// that needs it reads it again from the merchant:
//   Paddle: GET {api}/customers/{ctm_…}               -> data.email
//           GET {api}/transactions/{txn_…}            -> data.customer_id (orders recorded
//                                                        before customer ids were stored)
//   Stripe: GET https://api.stripe.com/v1/checkout/sessions/{cs_…} -> customer_details.email,
//                                                        payment_intent
//           GET https://api.stripe.com/v1/charges/{ch_…}           -> payment_intent (disputes
//                                                        whose object has no payment_intent)
// Errors carry the HTTP status or the error class, never an id, an address or a key.

export const PADDLE_API = 'https://api.paddle.com';
export const PADDLE_SANDBOX_API = 'https://sandbox-api.paddle.com';
export const STRIPE_API = 'https://api.stripe.com';
const FETCH_TIMEOUT_MS = 8_000;

export const EMAIL = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
export const PADDLE_CUSTOMER = /^ctm_[a-z0-9]{1,64}$/;
export const PADDLE_TXN = /^txn_[a-z0-9]{1,64}$/;
export const STRIPE_SESSION = /^cs_[A-Za-z0-9_]{1,250}$/;
export const STRIPE_PAYMENT_INTENT = /^pi_[A-Za-z0-9_]{1,250}$/;
export const STRIPE_CHARGE = /^(ch|py)_[A-Za-z0-9_]{1,250}$/;

export class MerchantApiError extends Error {
  constructor(message) {
    super(message);
    this.name = 'MerchantApiError';
  }
}

async function getJson(fetchFn, url, apiKey, label) {
  let res;
  try {
    res = await fetchFn(url, {
      method: 'GET',
      headers: { Authorization: `Bearer ${apiKey}`, Accept: 'application/json' },
      signal: AbortSignal.timeout(FETCH_TIMEOUT_MS),
    });
  } catch (err) {
    throw new MerchantApiError(`${label} request failed (${err?.name ?? 'Error'})`);
  }
  if (!res.ok) {
    await res.text().catch(() => {});
    throw new MerchantApiError(`${label} answered ${res.status}`);
  }
  try {
    return await res.json();
  } catch {
    throw new MerchantApiError(`${label} answer is not JSON`);
  }
}

const paddleBase = (paddle) => (paddle.sandbox ? PADDLE_SANDBOX_API : PADDLE_API);

/** Paddle customer email. `paddle` = { apiKey, sandbox? }. */
export async function paddleCustomerEmail(fetchFn, paddle, customerId) {
  if (!PADDLE_CUSTOMER.test(customerId)) throw new MerchantApiError('paddle customer id malformed');
  const json = await getJson(fetchFn, `${paddleBase(paddle)}/customers/${encodeURIComponent(customerId)}`, paddle.apiKey, 'paddle');
  const email = json?.data?.email;
  if (typeof email !== 'string' || !EMAIL.test(email)) throw new MerchantApiError('paddle customer has no email');
  return email;
}

/** Paddle transaction -> customer id (for orders recorded without one). */
export async function paddleTransactionCustomer(fetchFn, paddle, transactionId) {
  if (!PADDLE_TXN.test(transactionId)) throw new MerchantApiError('paddle transaction id malformed');
  const json = await getJson(fetchFn, `${paddleBase(paddle)}/transactions/${encodeURIComponent(transactionId)}`, paddle.apiKey, 'paddle');
  const customerId = json?.data?.customer_id;
  if (typeof customerId !== 'string' || !PADDLE_CUSTOMER.test(customerId)) throw new MerchantApiError('paddle transaction has no customer id');
  return customerId;
}

/**
 * Stripe checkout session -> { email, paymentIntent } (paymentIntent may be null).
 * `stripe` = { apiKey }.
 */
export async function stripeSession(fetchFn, stripe, sessionId) {
  if (!STRIPE_SESSION.test(sessionId)) throw new MerchantApiError('stripe session id malformed');
  const json = await getJson(fetchFn, `${STRIPE_API}/v1/checkout/sessions/${encodeURIComponent(sessionId)}`, stripe.apiKey, 'stripe');
  const email = json?.customer_details?.email ?? json?.customer_email;
  if (typeof email !== 'string' || !EMAIL.test(email)) throw new MerchantApiError('stripe session has no customer email');
  const pi = typeof json?.payment_intent === 'string' ? json.payment_intent : json?.payment_intent?.id;
  return { email, paymentIntent: typeof pi === 'string' && STRIPE_PAYMENT_INTENT.test(pi) ? pi : null };
}

/** Stripe charge -> payment intent id. */
export async function stripeChargePaymentIntent(fetchFn, stripe, chargeId) {
  if (!STRIPE_CHARGE.test(chargeId)) throw new MerchantApiError('stripe charge id malformed');
  const json = await getJson(fetchFn, `${STRIPE_API}/v1/charges/${encodeURIComponent(chargeId)}`, stripe.apiKey, 'stripe');
  const pi = typeof json?.payment_intent === 'string' ? json.payment_intent : json?.payment_intent?.id;
  if (typeof pi !== 'string' || !STRIPE_PAYMENT_INTENT.test(pi)) throw new MerchantApiError('stripe charge has no payment intent');
  return pi;
}
