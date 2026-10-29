import { test } from 'node:test';
import assert from 'node:assert/strict';
import { generateKeyPair, publicKeyLine } from '../lib/signing.mjs';
import { buildLicensePayload, issueLicense, openEnvelope, UUID_V4 } from '../lib/license.mjs';

test('license payload has the contract fields and defaults', () => {
  const p = buildLicensePayload({ email: 'a@b.co', issuedAt: 1_800_000_000 });
  assert.deepEqual(Object.keys(p), [
    'v', 'kind', 'license_id', 'product', 'issued_at', 'updates_until', 'max_activations', 'email',
  ]);
  assert.equal(p.v, 1);
  assert.equal(p.kind, 'license');
  assert.match(p.license_id, UUID_V4);
  assert.equal(p.product, 'valet-pro');
  assert.equal(p.updates_until, 1_800_000_000 + 365 * 86_400);
  assert.equal(p.max_activations, 3);
  assert.equal(buildLicensePayload({ email: 'a@b.co', issuedAt: 1, days: 30 }).updates_until, 1 + 30 * 86_400);
});

test('license payload rejects bad input', () => {
  assert.throws(() => buildLicensePayload({ email: 'nope' }), /email/);
  assert.throws(() => buildLicensePayload({ email: 'a@b.co', days: 0 }), /days/);
  assert.throws(() => buildLicensePayload({ email: 'a@b.co', days: 1.5 }), /days/);
  assert.throws(() => buildLicensePayload({ email: 'a@b.co', licenseId: 'x' }), /UUID/);
});

test('envelope payload is a string and its UTF-8 bytes are what is signed', () => {
  const key = generateKeyPair();
  const { envelope } = issueLicense({ email: 'ioana.ștefănescu@exemplu.ro' }, key);
  assert.equal(typeof envelope.payload, 'string');
  assert.equal(envelope.signature.split('\n').filter(Boolean).length, 4);
  const opened = openEnvelope(envelope, publicKeyLine(key));
  assert.equal(opened.email, 'ioana.ștefănescu@exemplu.ro');
  assert.throws(() => openEnvelope({ ...envelope, payload: envelope.payload.replace('ș', 's') }, publicKeyLine(key)));
  assert.throws(() => openEnvelope({ payload: {}, signature: envelope.signature }, publicKeyLine(key)), /envelope/);
});
