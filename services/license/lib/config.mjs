// Server configuration from the environment. Errors name the variables, never their values.
//
// Always:              VALET_LICENSE_SECRET_KEY, VALET_LICENSE_KEY_ID, VALET_MERCHANT
// paddle and stripe:   VALET_WEBHOOK_SECRET, VALET_RESEND_API_KEY, VALET_MAIL_FROM
// paddle:              VALET_PADDLE_API_KEY, VALET_PADDLE_PRICE_ID
// Optional:            VALET_PADDLE_SANDBOX=1, VALET_MAIL_REPLY_TO, VALET_TRUST_FLY=1,
//                      VALET_RATE_LIMIT_PER_MIN (default 30),
//                      VALET_STRIPE_API_KEY (stripe: disputes that carry only a charge id)
//
// resend.mjs and reissue.mjs (loadMailerConfig): the signing key, VALET_RESEND_API_KEY,
// VALET_MAIL_FROM, and the merchant of the order: VALET_PADDLE_API_KEY (+ optional
// VALET_PADDLE_SANDBOX) or VALET_STRIPE_API_KEY.
//
// VALET_MERCHANT=none runs the activation API without any webhook (local runs only).
// VALET_WEBHOOK_SECRET may hold two secrets separated by a comma while rotating on our
// side ("new,old"); the merchant's own rotation (two signatures in one header) works with one.

import { loadSecretKeyFromEnv } from './signing.mjs';

export const MERCHANT_CHOICES = ['paddle', 'stripe', 'none'];

export class ConfigError extends Error {
  constructor(missing, invalid) {
    const parts = [];
    if (missing.length) parts.push(`missing: ${missing.join(', ')}`);
    if (invalid.length) parts.push(`invalid: ${invalid.join(', ')}`);
    super(`environment not complete (${parts.join('; ')})`);
    this.name = 'ConfigError';
    this.missing = missing;
    this.invalid = invalid;
  }
}

const set = (v) => typeof v === 'string' && v.trim() !== '';
const MAIL_FROM = /^(?:[^<>]*<)?[^\s@<>]+@[^\s@<>]+\.[^\s@<>]+>?$/;

function checker(env) {
  const missing = [];
  const invalid = [];
  const need = (name) => {
    if (!set(env[name])) missing.push(name);
    return set(env[name]) ? env[name].trim() : undefined;
  };
  const optional = (name) => (set(env[name]) ? env[name].trim() : undefined);
  return { missing, invalid, need, optional };
}

function mailSettings({ need, optional, invalid }) {
  const apiKey = need('VALET_RESEND_API_KEY');
  const from = need('VALET_MAIL_FROM');
  if (from && !MAIL_FROM.test(from)) invalid.push('VALET_MAIL_FROM (Name <address@domain>)');
  const replyTo = optional('VALET_MAIL_REPLY_TO');
  return { apiKey, from, ...(replyTo ? { replyTo } : {}) };
}

function loadKey(env) {
  try {
    return loadSecretKeyFromEnv(env);
  } catch (err) {
    // The messages of loadSecretKeyFromEnv name the variable and never include the value.
    throw new ConfigError([], [err.message]);
  }
}

export function loadServerConfig(env = process.env) {
  const c = checker(env);
  const { missing, invalid, need } = c;

  need('VALET_LICENSE_SECRET_KEY');
  need('VALET_LICENSE_KEY_ID');
  const merchant = need('VALET_MERCHANT');
  if (merchant !== undefined && !MERCHANT_CHOICES.includes(merchant)) invalid.push('VALET_MERCHANT (paddle|stripe|none)');

  let webhook = null;
  if (merchant === 'paddle' || merchant === 'stripe') {
    const secretText = need('VALET_WEBHOOK_SECRET');
    const mail = mailSettings(c);
    const secrets = secretText ? secretText.split(',').map((s) => s.trim()).filter(Boolean) : [];
    if (secretText && (secrets.length === 0 || secrets.length > 2)) invalid.push('VALET_WEBHOOK_SECRET (one secret, or two separated by a comma)');
    webhook = { merchant, secrets, mail };
    if (merchant === 'stripe') {
      const apiKey = c.optional('VALET_STRIPE_API_KEY');
      if (apiKey) webhook.stripe = { apiKey };
    }
    if (merchant === 'paddle') {
      const apiKey = need('VALET_PADDLE_API_KEY');
      const priceId = need('VALET_PADDLE_PRICE_ID');
      if (priceId && !/^pri_[a-z0-9]{1,64}$/.test(priceId)) invalid.push('VALET_PADDLE_PRICE_ID (pri_…)');
      webhook.paddle = { apiKey, priceId, sandbox: env.VALET_PADDLE_SANDBOX === '1' };
    }
  }

  let perMinute = 30;
  if (env.VALET_RATE_LIMIT_PER_MIN !== undefined) {
    perMinute = Number(env.VALET_RATE_LIMIT_PER_MIN);
    if (!Number.isSafeInteger(perMinute) || perMinute <= 0) invalid.push('VALET_RATE_LIMIT_PER_MIN (positive integer)');
  }

  if (missing.length || invalid.length) throw new ConfigError(missing, invalid);

  const key = loadKey(env);

  return {
    key,
    merchant,
    webhook,
    trustFly: env.VALET_TRUST_FLY === '1',
    rateLimit: { capacity: perMinute, perMinute },
  };
}

/**
 * Settings for resend.mjs and reissue.mjs: signing key, Resend, and the API key of the
 * merchant that sold the order (`paddle` or `stripe`, from the order id's prefix).
 */
export function loadMailerConfig(env, merchant) {
  const c = checker(env);
  c.need('VALET_LICENSE_SECRET_KEY');
  c.need('VALET_LICENSE_KEY_ID');
  const mail = mailSettings(c);
  const out = { mail };
  if (merchant === 'paddle') {
    out.paddle = { apiKey: c.need('VALET_PADDLE_API_KEY'), sandbox: env.VALET_PADDLE_SANDBOX === '1' };
  } else if (merchant === 'stripe') {
    out.stripe = { apiKey: c.need('VALET_STRIPE_API_KEY') };
  } else {
    throw new Error('merchant must be paddle or stripe');
  }
  if (c.missing.length || c.invalid.length) throw new ConfigError(c.missing, c.invalid);
  out.key = loadKey(env);
  return out;
}
