// License files and activation tokens (docs/LICENSING.md).
// Both use the envelope { payload, signature }: `payload` is a JSON string and the
// signed bytes are exactly its UTF-8 bytes; `signature` is a prehashed minisign file.

import crypto from 'node:crypto';
import { sign, verify } from './signing.mjs';

export const PRODUCT = 'valet-pro';
export const DEFAULT_DAYS = 365;
export const DEFAULT_MAX_ACTIVATIONS = 3;
const DAY = 86_400;

export const UUID_V4 = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
export const MACHINE_ID = /^[0-9a-f]{32}$/;

export function nowSeconds() {
  return Math.floor(Date.now() / 1000);
}

function checkEmail(email) {
  if (
    typeof email !== 'string' ||
    email.length > 254 ||
    !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email)
  ) {
    throw new Error('invalid email');
  }
  return email;
}

function checkPositiveInt(value, name) {
  if (!Number.isSafeInteger(value) || value <= 0) throw new Error(`${name} must be a positive integer`);
  return value;
}

/**
 * Builds the license payload object. Key order follows docs/LICENSING.md.
 * `updatesUntil` (Unix seconds) replaces `issuedAt + days`: a reissued license keeps the
 * old license's date, so the buyer neither loses nor gains update time.
 */
export function buildLicensePayload({
  email,
  days = DEFAULT_DAYS,
  maxActivations = DEFAULT_MAX_ACTIVATIONS,
  issuedAt = nowSeconds(),
  licenseId = crypto.randomUUID(),
  updatesUntil,
}) {
  if (!UUID_V4.test(licenseId)) throw new Error('license id must be a lowercase UUID v4');
  checkPositiveInt(days, 'days');
  checkPositiveInt(maxActivations, 'max activations');
  checkPositiveInt(issuedAt, 'issued_at');
  if (updatesUntil !== undefined) checkPositiveInt(updatesUntil, 'updates_until');
  return {
    v: 1,
    kind: 'license',
    license_id: licenseId,
    product: PRODUCT,
    issued_at: issuedAt,
    updates_until: updatesUntil ?? issuedAt + days * DAY,
    max_activations: maxActivations,
    email: checkEmail(email),
  };
}

/** Builds the activation token payload object. */
export function buildActivationPayload({ licenseId, machine, checkedAt = nowSeconds() }) {
  if (!UUID_V4.test(licenseId)) throw new Error('license id must be a lowercase UUID v4');
  if (!MACHINE_ID.test(machine)) throw new Error('machine must be 32 lowercase hex characters');
  checkPositiveInt(checkedAt, 'checked_at');
  return { v: 1, kind: 'activation', license_id: licenseId, machine, checked_at: checkedAt };
}

/**
 * Signs a payload object and returns the envelope. The trusted comment carries the
 * signing time and the kind only (no email, no machine id).
 */
export function signEnvelope(payloadObject, key, { now = nowSeconds() } = {}) {
  const payload = JSON.stringify(payloadObject);
  const signature = sign(payload, key, {
    trustedComment: `timestamp:${now}\tfile:valet-${payloadObject.kind}\thashed`,
  });
  return { payload, signature };
}

/** Issues a license: returns { payload (object), envelope }. */
export function issueLicense(options, key) {
  const payload = buildLicensePayload(options);
  return { payload, envelope: signEnvelope(payload, key, { now: payload.issued_at }) };
}

/**
 * Signs the license of a store record again for `email`: same id, dates and activation
 * limit. Ed25519 signatures are deterministic and the trusted comment uses `issued_at`,
 * so with the same email the file is byte-identical to the one sent at purchase.
 */
export function resignLicense(record, email, key) {
  return issueLicense(
    {
      email,
      licenseId: record.license_id,
      issuedAt: record.issued_at,
      updatesUntil: record.updates_until,
      maxActivations: record.max_activations,
    },
    key,
  );
}

/** Issues an activation token envelope. */
export function issueActivationToken(options, key) {
  const payload = buildActivationPayload(options);
  return signEnvelope(payload, key, { now: payload.checked_at });
}

/** Verifies an envelope and returns the parsed payload object. Throws when invalid. */
export function openEnvelope(envelope, publicKey) {
  if (
    !envelope ||
    typeof envelope.payload !== 'string' ||
    typeof envelope.signature !== 'string'
  ) {
    throw new Error('envelope: expected { payload: string, signature: string }');
  }
  verify(envelope.payload, envelope.signature, publicKey);
  return JSON.parse(envelope.payload);
}
