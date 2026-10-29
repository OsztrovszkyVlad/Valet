import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  generateKeyPair,
  keyPairFromSeed,
  publicKeyLine,
  publicKeyFile,
  parsePublicKey,
  keyIdDisplay,
  exportSecretKey,
  loadSecretKeyFromEnv,
  sign,
  verify,
} from '../lib/signing.mjs';
import { issueLicense, issueActivationToken, openEnvelope } from '../lib/license.mjs';

// TEST key: seed and key id are public on purpose. Never ship its public half.
const TEST_KEY = keyPairFromSeed(
  Buffer.from('54455354'.repeat(8), 'hex'),
  Buffer.from('544553544b455931', 'hex'),
);
const TEST_PUBKEY = 'RWRURVNUS0VZMdKHQlZnx1Q9hKNAmHWgbluECvO3l6cHCoq6H/7XjDvF';

// Vectors produced by this module and verified once with the Rust crate
// minisign-verify 0.3.0 (PublicKey::verify, allow_legacy = false).
const VECTOR_LICENSE = {
  payload:
    '{"v":1,"kind":"license","license_id":"1b4e28ba-2fa1-4d2e-8b5c-3f6a1c2d9e70","product":"valet-pro","issued_at":1793000000,"updates_until":1824536000,"max_activations":3,"email":"test@example.com"}',
  signature:
    'untrusted comment: signature from valet license service\n' +
    'RURURVNUS0VZMS1cAJYOC/MXGwVt0zRVgoKjmJxxRkiIui/VxbOruSFXIFMkGWyNiscWhXDk+8/QQKwI4aQ2vF9f3y8snQS23Qo=\n' +
    'trusted comment: timestamp:1793000000\tfile:valet-license\thashed\n' +
    '35z9pPzFhytP/YI4WzZd8WsdZ+PShLKlOvF/wEVrnPPKME//ujtRoPavGqMH8k5I3c9fp040OWSTDnR1YdebAw==\n',
};
const VECTOR_ACTIVATION = {
  payload:
    '{"v":1,"kind":"activation","license_id":"1b4e28ba-2fa1-4d2e-8b5c-3f6a1c2d9e70","machine":"0123456789abcdef0123456789abcdef","checked_at":1793000000}',
  signature:
    'untrusted comment: signature from valet license service\n' +
    'RURURVNUS0VZMQyk/dA8o9U80fzlxqs2k4tS1oLYCOiw/DBAYYBd2pJ0EJ55rIrvo/RZoxF1YUJjhHxChifvsLxqMnUlLkxmBQc=\n' +
    'trusted comment: timestamp:1793000000\tfile:valet-activation\thashed\n' +
    'ZTBgTR+iEqIYdA5C/vZ7W66NGToiW7CBO/hmzGwCXV1quH6e4LiZolmAZp4vd3KSb+A3wGaf1ErNnIc5mMB2Dg==\n',
};

test('TEST key public line matches the vector', () => {
  assert.equal(publicKeyLine(TEST_KEY), TEST_PUBKEY);
  assert.equal(keyIdDisplay(TEST_KEY.keyId), '3159454B54534554');
  assert.equal(
    publicKeyFile(TEST_KEY),
    `untrusted comment: minisign public key 3159454B54534554\n${TEST_PUBKEY}\n`,
  );
});

test('signing is deterministic and reproduces the cross-checked vectors', () => {
  const lic = issueLicense(
    {
      email: 'test@example.com',
      issuedAt: 1793000000,
      licenseId: '1b4e28ba-2fa1-4d2e-8b5c-3f6a1c2d9e70',
    },
    TEST_KEY,
  );
  assert.deepEqual(lic.envelope, VECTOR_LICENSE);
  const tok = issueActivationToken(
    {
      licenseId: '1b4e28ba-2fa1-4d2e-8b5c-3f6a1c2d9e70',
      machine: '0123456789abcdef0123456789abcdef',
      checkedAt: 1793000000,
    },
    TEST_KEY,
  );
  assert.deepEqual(tok, VECTOR_ACTIVATION);
  assert.equal(openEnvelope(VECTOR_LICENSE, TEST_PUBKEY).updates_until, 1793000000 + 365 * 86400);
  assert.equal(openEnvelope(VECTOR_ACTIVATION, TEST_PUBKEY).kind, 'activation');
});

test('signature file format: 4 lines, ED prehashed, key id embedded', () => {
  const key = generateKeyPair();
  const sig = sign('hello', key, { trustedComment: 'timestamp:1\thashed', untrustedComment: 'x' });
  const lines = sig.split('\n');
  assert.equal(lines.length, 5);
  assert.equal(lines[4], '');
  assert.equal(lines[0], 'untrusted comment: x');
  assert.equal(lines[2], 'trusted comment: timestamp:1\thashed');
  const blob = Buffer.from(lines[1], 'base64');
  assert.equal(blob.length, 74);
  assert.equal(blob.subarray(0, 2).toString('ascii'), 'ED');
  assert.ok(blob.subarray(2, 10).equals(key.keyId));
  assert.equal(Buffer.from(lines[3], 'base64').length, 64);
  const pk = Buffer.from(publicKeyLine(key), 'base64');
  assert.equal(pk.length, 42);
  assert.equal(pk.subarray(0, 2).toString('ascii'), 'Ed');
});

test('round trip: both signatures verify; tampering is rejected', () => {
  const key = generateKeyPair();
  const line = publicKeyLine(key);
  const message = '{"a":"ăîșțâ"}';
  const sig = sign(message, key, { trustedComment: 'timestamp:5\tfile:x\thashed' });
  assert.deepEqual(verify(message, sig, line), { trustedComment: 'timestamp:5\tfile:x\thashed' });
  assert.deepEqual(verify(Buffer.from(message, 'utf8'), sig, parsePublicKey(line)).trustedComment,
    'timestamp:5\tfile:x\thashed');
  assert.throws(() => verify(`${message} `, sig, line), /invalid/);
  const lines = sig.split('\n');
  const forgedComment = [lines[0], lines[1], 'trusted comment: timestamp:6\tfile:x\thashed', lines[3], ''].join('\n');
  assert.throws(() => verify(message, forgedComment, line), /trusted comment signature/);
  const other = generateKeyPair();
  assert.throws(() => verify(message, sig, publicKeyLine(other)), /key id mismatch/);
  const sameIdOtherKey = { ...other, keyId: key.keyId };
  assert.throws(() => verify(message, sig, publicKeyLine(sameIdOtherKey)), /invalid/);
});

test('legacy (non-prehashed) signatures are rejected', () => {
  const key = generateKeyPair();
  const lines = sign('m', key).split('\n');
  const blob = Buffer.from(lines[1], 'base64');
  blob.write('Ed', 0, 'ascii');
  lines[1] = blob.toString('base64');
  assert.throws(() => verify('m', lines.join('\n'), publicKeyLine(key)), /legacy/);
});

test('comments must be single lines', () => {
  const key = generateKeyPair();
  assert.throws(() => sign('m', key, { trustedComment: 'a\nb' }), /single line/);
  assert.throws(() => sign('m', key, { untrustedComment: 'a\rb' }), /single line/);
});

test('secret key loads from the documented env encoding', () => {
  const key = generateKeyPair();
  const env = {
    VALET_LICENSE_SECRET_KEY: exportSecretKey(key.privateKey),
    VALET_LICENSE_KEY_ID: key.keyId.toString('hex'),
  };
  const loaded = loadSecretKeyFromEnv(env);
  assert.equal(publicKeyLine(loaded), publicKeyLine(key));
  const sig = sign('m', loaded);
  verify('m', sig, publicKeyLine(key));

  assert.throws(() => loadSecretKeyFromEnv({}), /VALET_LICENSE_SECRET_KEY is not set/);
  assert.throws(
    () => loadSecretKeyFromEnv({ ...env, VALET_LICENSE_KEY_ID: 'XYZ' }),
    /16 lowercase hex/,
  );
  let message = '';
  try {
    loadSecretKeyFromEnv({ ...env, VALET_LICENSE_SECRET_KEY: 'bm90IGEga2V5' });
  } catch (err) {
    message = err.message;
  }
  assert.match(message, /not a base64 PKCS#8 DER key/);
  assert.ok(!message.includes('bm90IGEga2V5'));
});
