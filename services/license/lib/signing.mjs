// Minisign signatures in the prehashed format ("ED", BLAKE2b-512), byte-compatible
// with minisign 0.11 and the Rust `minisign-verify` crate (allow_legacy = false).
//
// Signature file (4 lines, each ending in "\n"):
//   untrusted comment: <text>
//   base64("ED" || key_id[8] || ed25519(BLAKE2b-512(message)))
//   trusted comment: <text>
//   base64(ed25519(signature[64] || trusted_comment_bytes))
//
// Public key line: base64("Ed" || key_id[8] || public_key[32]).
//
// Secret key in the environment:
//   VALET_LICENSE_SECRET_KEY  base64 of the Ed25519 private key as PKCS#8 DER (48 bytes)
//   VALET_LICENSE_KEY_ID      the 8 key id bytes as 16 lowercase hex characters, in the
//                             order they appear inside the signature and public key blobs

import crypto from 'node:crypto';

const SIG_ALG_PREHASHED = Buffer.from('ED', 'ascii');
const SIG_ALG_LEGACY = Buffer.from('Ed', 'ascii');
const PK_ALG = Buffer.from('Ed', 'ascii');
const UNTRUSTED_PREFIX = 'untrusted comment: ';
const TRUSTED_PREFIX = 'trusted comment: ';
// DER prefixes for Ed25519 keys (RFC 8410).
const SPKI_PREFIX = Buffer.from('302a300506032b6570032100', 'hex');
const PKCS8_PREFIX = Buffer.from('302e020100300506032b657004220420', 'hex');

export const DEFAULT_UNTRUSTED_COMMENT = 'signature from valet license service';

/** Generates a new key pair with a random 8-byte key id. */
export function generateKeyPair() {
  const { privateKey, publicKey } = crypto.generateKeyPairSync('ed25519');
  return { keyId: crypto.randomBytes(8), privateKey, publicKey };
}

/** Builds a key pair from a fixed 32-byte seed. For test vectors only. */
export function keyPairFromSeed(seed, keyId) {
  if (!Buffer.isBuffer(seed) || seed.length !== 32) throw new Error('seed must be 32 bytes');
  const privateKey = crypto.createPrivateKey({
    key: Buffer.concat([PKCS8_PREFIX, seed]),
    format: 'der',
    type: 'pkcs8',
  });
  return { keyId: checkKeyId(keyId), privateKey, publicKey: crypto.createPublicKey(privateKey) };
}

function checkKeyId(keyId) {
  if (!Buffer.isBuffer(keyId) || keyId.length !== 8) throw new Error('key id must be 8 bytes');
  return keyId;
}

function rawPublicKey(publicKey) {
  const der = publicKey.export({ format: 'der', type: 'spki' });
  if (der.length !== 44 || !der.subarray(0, 12).equals(SPKI_PREFIX)) {
    throw new Error('not an Ed25519 public key');
  }
  return der.subarray(12);
}

/** The key id as minisign prints it in comments (little-endian u64, uppercase hex). */
export function keyIdDisplay(keyId) {
  return Buffer.from(checkKeyId(keyId)).reverse().toString('hex').toUpperCase();
}

/** The public key line that goes into crates/valet-core/license/pubkey.txt. */
export function publicKeyLine({ keyId, publicKey }) {
  return Buffer.concat([PK_ALG, checkKeyId(keyId), rawPublicKey(publicKey)]).toString('base64');
}

/** A complete minisign public key file (comment line + key line). */
export function publicKeyFile(key) {
  return `${UNTRUSTED_PREFIX}minisign public key ${keyIdDisplay(key.keyId)}\n${publicKeyLine(key)}\n`;
}

/** Parses a public key line (or a two-line public key file). */
export function parsePublicKey(text) {
  const lines = String(text)
    .split('\n')
    .map((l) => l.trim())
    .filter(Boolean);
  const line = lines.length >= 2 && lines[0].startsWith(UNTRUSTED_PREFIX) ? lines[1] : lines[0];
  const bin = Buffer.from(line ?? '', 'base64');
  if (bin.length !== 42 || !bin.subarray(0, 2).equals(PK_ALG)) {
    throw new Error('invalid minisign public key');
  }
  const publicKey = crypto.createPublicKey({
    key: Buffer.concat([SPKI_PREFIX, bin.subarray(10)]),
    format: 'der',
    type: 'spki',
  });
  return { keyId: Buffer.from(bin.subarray(2, 10)), publicKey };
}

/** base64 of the PKCS#8 DER private key: the value format of VALET_LICENSE_SECRET_KEY. */
export function exportSecretKey(privateKey) {
  return privateKey.export({ format: 'der', type: 'pkcs8' }).toString('base64');
}

/**
 * Loads the signing key from the environment. Error messages never contain the values.
 * Returns { keyId, privateKey, publicKey }.
 */
export function loadSecretKeyFromEnv(env = process.env) {
  const secret = env.VALET_LICENSE_SECRET_KEY;
  const keyIdHex = env.VALET_LICENSE_KEY_ID;
  if (!secret) throw new Error('VALET_LICENSE_SECRET_KEY is not set');
  if (!keyIdHex) throw new Error('VALET_LICENSE_KEY_ID is not set');
  if (!/^[0-9a-f]{16}$/.test(keyIdHex)) {
    throw new Error('VALET_LICENSE_KEY_ID must be 16 lowercase hex characters');
  }
  let privateKey;
  try {
    privateKey = crypto.createPrivateKey({
      key: Buffer.from(secret, 'base64'),
      format: 'der',
      type: 'pkcs8',
    });
  } catch {
    throw new Error('VALET_LICENSE_SECRET_KEY is not a base64 PKCS#8 DER key');
  }
  if (privateKey.asymmetricKeyType !== 'ed25519') {
    throw new Error('VALET_LICENSE_SECRET_KEY is not an Ed25519 key');
  }
  return {
    keyId: Buffer.from(keyIdHex, 'hex'),
    privateKey,
    publicKey: crypto.createPublicKey(privateKey),
  };
}

function checkComment(comment, name) {
  if (typeof comment !== 'string' || /[\r\n]/.test(comment)) {
    throw new Error(`${name} must be a single line`);
  }
  return comment;
}

function blake2b512(data) {
  return crypto.createHash('blake2b512').update(data).digest();
}

/**
 * Signs `message` (Buffer or string; strings are taken as UTF-8) and returns the
 * signature file text. Ed25519 is deterministic: same key, message and comments give
 * the same output.
 */
export function sign(message, { keyId, privateKey }, { trustedComment, untrustedComment } = {}) {
  const data = Buffer.isBuffer(message) ? message : Buffer.from(String(message), 'utf8');
  const trusted = checkComment(
    trustedComment ?? `timestamp:${Math.floor(Date.now() / 1000)}\thashed`,
    'trusted comment',
  );
  const untrusted = checkComment(untrustedComment ?? DEFAULT_UNTRUSTED_COMMENT, 'untrusted comment');
  const signature = crypto.sign(null, blake2b512(data), privateKey);
  const globalSignature = crypto.sign(
    null,
    Buffer.concat([signature, Buffer.from(trusted, 'utf8')]),
    privateKey,
  );
  const sigBlob = Buffer.concat([SIG_ALG_PREHASHED, checkKeyId(keyId), signature]);
  return (
    `${UNTRUSTED_PREFIX}${untrusted}\n` +
    `${sigBlob.toString('base64')}\n` +
    `${TRUSTED_PREFIX}${trusted}\n` +
    `${globalSignature.toString('base64')}\n`
  );
}

/**
 * Verifies a signature file against a message and a public key ({keyId, publicKey}
 * or a public key line). Rejects the legacy non-prehashed format, like
 * minisign-verify with allow_legacy = false. Returns { trustedComment } or throws.
 */
export function verify(message, signatureText, key) {
  const { keyId, publicKey } = typeof key === 'string' ? parsePublicKey(key) : key;
  const data = Buffer.isBuffer(message) ? message : Buffer.from(String(message), 'utf8');
  const lines = String(signatureText)
    .split('\n')
    .map((l) => l.replace(/\r$/, ''));
  if (lines.length < 4) throw new Error('signature: expected 4 lines');
  const [untrusted, sigLine, trusted, globalLine] = lines;
  if (!untrusted.startsWith(UNTRUSTED_PREFIX)) throw new Error('signature: bad untrusted comment');
  if (!trusted.startsWith(TRUSTED_PREFIX)) throw new Error('signature: bad trusted comment');
  const sigBlob = Buffer.from(sigLine, 'base64');
  const globalSignature = Buffer.from(globalLine, 'base64');
  if (sigBlob.length !== 74 || globalSignature.length !== 64) {
    throw new Error('signature: bad encoding');
  }
  const alg = sigBlob.subarray(0, 2);
  if (alg.equals(SIG_ALG_LEGACY)) throw new Error('signature: legacy format not allowed');
  if (!alg.equals(SIG_ALG_PREHASHED)) throw new Error('signature: unknown algorithm');
  if (!sigBlob.subarray(2, 10).equals(keyId)) throw new Error('signature: key id mismatch');
  const signature = sigBlob.subarray(10);
  if (!crypto.verify(null, blake2b512(data), publicKey, signature)) {
    throw new Error('signature: invalid');
  }
  const trustedComment = trusted.slice(TRUSTED_PREFIX.length);
  const globalData = Buffer.concat([signature, Buffer.from(trustedComment, 'utf8')]);
  if (!crypto.verify(null, globalData, publicKey, globalSignature)) {
    throw new Error('signature: invalid trusted comment signature');
  }
  return { trustedComment };
}
