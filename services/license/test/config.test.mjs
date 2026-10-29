import { test } from 'node:test';
import assert from 'node:assert/strict';
import { loadServerConfig, loadMailerConfig, ConfigError } from '../lib/config.mjs';
import { generateKeyPair } from '../lib/signing.mjs';

test('config: VALET_STRIPE_API_KEY is optional for the server and required by the mailer CLIs for Stripe orders', async () => {
  const k = generateKeyPair();
  const keyEnv = {
    VALET_LICENSE_SECRET_KEY: k.privateKey.export({ format: 'der', type: 'pkcs8' }).toString('base64'),
    VALET_LICENSE_KEY_ID: k.keyId.toString('hex'),
  };
  const base = { ...keyEnv, VALET_MERCHANT: 'stripe', VALET_WEBHOOK_SECRET: 's', VALET_RESEND_API_KEY: 'r', VALET_MAIL_FROM: 'Valet <a@b.ro>' };
  assert.equal(loadServerConfig(base).webhook.stripe, undefined);
  assert.deepEqual(loadServerConfig({ ...base, VALET_STRIPE_API_KEY: 'test-key' }).webhook.stripe, { apiKey: 'test-key' });
  assert.throws(() => loadMailerConfig(base, 'stripe'), (e) => e instanceof ConfigError && e.missing.join() === 'VALET_STRIPE_API_KEY');
  const paddle = loadMailerConfig({ ...base, VALET_PADDLE_API_KEY: 'p', VALET_PADDLE_SANDBOX: '1' }, 'paddle');
  assert.deepEqual(paddle.paddle, { apiKey: 'p', sandbox: true });
  assert.deepEqual(paddle.mail, { apiKey: 'r', from: 'Valet <a@b.ro>' });
});
