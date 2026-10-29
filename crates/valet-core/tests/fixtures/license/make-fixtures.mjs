// Generates the signed license fixtures for crates/valet-core/tests/license.rs.
//
//   node crates/valet-core/tests/fixtures/license/make-fixtures.mjs
//
// Built-in modules only. Output is byte-identical on every run: the keys come
// from fixed seeds, Ed25519 signatures are deterministic, and every timestamp
// is a constant below.
//
// The keys here are TEST keys. Their secret seeds are committed on purpose:
// they only sign these fixtures, and no TEST public key is ever shipped
// (crates/valet-core/license/pubkey.txt holds the real key, committed by Vlad).
//
// Signature format (minisign, prehashed "ED"):
//   untrusted comment: <text>
//   base64("ED" || key_id[8] || Ed25519(BLAKE2b-512(payload))[64])
//   trusted comment: <text>
//   base64(Ed25519(signature[64] || trusted_comment_text))
// The legacy fixture uses "Ed" and signs the payload bytes directly.

import { createHash, createPrivateKey, createPublicKey, sign } from 'node:crypto';
import { writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const OUT = dirname(fileURLToPath(import.meta.url));
const DAY = 86400;

// Keep in sync with the constants in tests/license.rs.
const NOW = 1_794_000_000;
const LICENSE_ID = '6f1c2a3b-4d5e-4f60-8a71-92b3c4d5e6f7';
const OTHER_LICENSE_ID = '0a1b2c3d-4e5f-4a6b-9c7d-8e9fa0b1c2d3';
const MACHINE = '0123456789abcdef0123456789abcdef';
const OTHER_MACHINE = 'fedcba9876543210fedcba9876543210';
const EMAIL = 'test@example.com';
const ISSUED_AT = 1_790_000_000;
const UPDATES_UNTIL = ISSUED_AT + 365 * DAY; // 1_821_536_000
const OLD_ISSUED_AT = 1_750_000_000;
const OLD_UPDATES_UNTIL = OLD_ISSUED_AT + 365 * DAY; // 1_781_536_000
const SIGNED_AT = 1_793_000_000; // goes in the trusted comment only

const sha256 = (s) => createHash('sha256').update(s).digest();

function testKey(label, keyIdLabel) {
    const seed = sha256(label);
    const der = Buffer.concat([Buffer.from('302e020100300506032b657004220420', 'hex'), seed]);
    const priv = createPrivateKey({ key: der, format: 'der', type: 'pkcs8' });
    const spki = createPublicKey(priv).export({ format: 'der', type: 'spki' });
    return { priv, pub: spki.subarray(spki.length - 32), keyId: sha256(keyIdLabel).subarray(0, 8) };
}

const TEST = testKey('valet TEST license key 1', 'valet TEST key id 1');
const OTHER = testKey('valet TEST license key 2 (not trusted)', 'valet TEST key id 2');
// Same key id as TEST, different secret: only the Ed25519 check can catch it.
const FORGED = testKey('valet TEST license key 3 (forged)', 'valet TEST key id 1');

function minisign(key, payload, { legacy = false } = {}) {
    const data = Buffer.from(payload, 'utf8');
    const signed = legacy ? data : createHash('blake2b512').update(data).digest();
    const sig = sign(null, signed, key.priv);
    const alg = Buffer.from(legacy ? 'Ed' : 'ED', 'ascii');
    const trusted = `timestamp:${SIGNED_AT}\tfile:payload\t${legacy ? 'legacy' : 'hashed'}`;
    const global = sign(null, Buffer.concat([sig, Buffer.from(trusted, 'utf8')]), key.priv);
    return [
        'untrusted comment: signature from Valet TEST key',
        Buffer.concat([alg, key.keyId, sig]).toString('base64'),
        `trusted comment: ${trusted}`,
        global.toString('base64'),
        '',
    ].join('\n');
}

function envelope(payloadObj, key = TEST, opts = {}) {
    const payload = JSON.stringify(payloadObj);
    return JSON.stringify({ payload, signature: minisign(key, payload, opts) }, null, 2) + '\n';
}

const license = (over = {}) => ({
    v: 1,
    kind: 'license',
    license_id: LICENSE_ID,
    product: 'valet-pro',
    issued_at: ISSUED_AT,
    updates_until: UPDATES_UNTIL,
    max_activations: 3,
    email: EMAIL,
    ...over,
});

const token = (over = {}) => ({
    v: 1,
    kind: 'activation',
    license_id: LICENSE_ID,
    machine: MACHINE,
    checked_at: NOW - DAY,
    ...over,
});

const files = {
    'TEST-pubkey.txt': `${Buffer.concat([Buffer.from('Ed', 'ascii'), TEST.keyId, TEST.pub]).toString('base64')}\n`,
    'TEST-other-pubkey.txt': `${Buffer.concat([Buffer.from('Ed', 'ascii'), OTHER.keyId, OTHER.pub]).toString('base64')}\n`,
    'license-valid.valet-license': envelope(license()),
    'license-wrong-product.valet-license': envelope(license({ product: 'valet-lite' })),
    'license-not-covering-build.valet-license': envelope(
        license({ issued_at: OLD_ISSUED_AT, updates_until: OLD_UPDATES_UNTIL }),
    ),
    'license-v2.valet-license': envelope(license({ v: 2 })),
    'license-other-key.valet-license': envelope(license(), OTHER),
    'license-forged-same-key-id.valet-license': envelope(license(), FORGED),
    'license-legacy.valet-license': envelope(license(), TEST, { legacy: true }),
    'license-malformed.valet-license': '{"payload": "{\\"v\\":1,\\"kind\\":\\"license\\"", "signature": \n',
    'token-valid.valet-token': envelope(token()),
    'token-31-days.valet-token': envelope(token({ checked_at: NOW - 31 * DAY })),
    'token-61-days.valet-token': envelope(token({ checked_at: NOW - 61 * DAY })),
    'token-future-2-days.valet-token': envelope(token({ checked_at: NOW + 2 * DAY })),
    'token-other-machine.valet-token': envelope(token({ machine: OTHER_MACHINE })),
    'token-other-license.valet-token': envelope(token({ license_id: OTHER_LICENSE_ID })),
    'token-other-key.valet-token': envelope(token(), OTHER),
};

// Tampered: one character of the signed payload changed after signing.
{
    const env = JSON.parse(envelope(license()));
    env.payload = env.payload.replace('test@example.com', 'test@exbmple.com');
    files['license-tampered.valet-license'] = JSON.stringify(env, null, 2) + '\n';
}
{
    const env = JSON.parse(envelope(token()));
    env.payload = env.payload.replace(MACHINE, MACHINE.replace('0123', '0124'));
    files['token-tampered.valet-token'] = JSON.stringify(env, null, 2) + '\n';
}

for (const [name, text] of Object.entries(files)) {
    writeFileSync(join(OUT, name), text);
}
console.log(`wrote ${Object.keys(files).length} files to ${OUT}`);
