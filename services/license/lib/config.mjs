// Server configuration from the environment. Errors name the variables, never their values.
//
// Always:              VALET_LICENSE_SECRET_KEY, VALET_LICENSE_KEY_ID, VALET_MERCHANT
// paddle and stripe:   VALET_WEBHOOK_SECRET, VALET_RESEND_API_KEY, VALET_MAIL_FROM
// paddle:              VALET_PADDLE_API_KEY, VALET_PADDLE_PRICE_ID
// Optional:            VALET_PADDLE_SANDBOX=1, VALET_MAIL_REPLY_TO, VALET_TRUST_FLY=1,
//                      VALET_RATE_LIMIT_PER_MIN (default 30)
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

export function loadServerConfig(env = process.env) {
  const missing = [];
  const invalid = [];
  const need = (name) => {
    if (!set(env[name])) missing.push(name);
    return set(env[name]) ? env[name].trim() : undefined;
  };

  need('VALET_LICENSE_SECRET_KEY');
  need('VALET_LICENSE_KEY_ID');
  const merchant = need('VALET_MERCHANT');
  if (merchant !== undefined && !MERCHANT_CHOICES.includes(merchant)) invalid.push('VALET_MERCHANT (paddle|stripe|none)');

  let webhook = null;
  if (merchant === 'paddle' || merchant === 'stripe') {
    const secretText = need('VALET_WEBHOOK_SECRET');
    const resendKey = need('VALET_RESEND_API_KEY');
    const from = need('VALET_MAIL_FROM');
    const secrets = secretText ? secretText.split(',').map((s) => s.trim()).filter(Boolean) : [];
    if (secretText && (secrets.length === 0 || secrets.length > 2)) invalid.push('VALET_WEBHOOK_SECRET (one secret, or two separated by a comma)');
    if (from && !/^(?:[^<>]*<)?[^\s@<>]+@[^\s@<>]+\.[^\s@<>]+>?$/.test(from)) invalid.push('VALET_MAIL_FROM (Name <address@domain>)');
    const replyTo = set(env.VALET_MAIL_REPLY_TO) ? env.VALET_MAIL_REPLY_TO.trim() : undefined;
    webhook = { merchant, secrets, mail: { apiKey: resendKey, from, ...(replyTo ? { replyTo } : {}) } };
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

  let key;
  try {
    key = loadSecretKeyFromEnv(env);
  } catch (err) {
    // The messages of loadSecretKeyFromEnv name the variable and never include the value.
    throw new ConfigError([], [err.message]);
  }

  return {
    key,
    merchant,
    webhook,
    trustFly: env.VALET_TRUST_FLY === '1',
    rateLimit: { capacity: perMinute, perMinute },
  };
}
