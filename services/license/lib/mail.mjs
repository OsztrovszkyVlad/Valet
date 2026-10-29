// License email through Resend (POST https://api.resend.com/emails).
// The email address is used for this one request and is not stored or logged.

export const RESEND_URL = 'https://api.resend.com/emails';
export const ATTACHMENT_NAME = 'valet-pro.valet-license';
const TIMEOUT_MS = 8_000;

const isoDate = (seconds) => new Date(seconds * 1000).toISOString().slice(0, 10);

/** Subject and plain-text body, Romanian first, then English. */
export function licenseMessage(payload) {
  const until = isoDate(payload.updates_until);
  const n = payload.max_activations;
  const text = [
    'Licența ta Valet Pro este atașată acestui email (fișierul valet-pro.valet-license).',
    '',
    'Ca s-o activezi: deschide Valet → Setări → Valet Pro → Am o licență și alege fișierul atașat.',
    `Licența poate fi activă pe cel mult ${n} calculatoare și include actualizările publicate până la ${until}.`,
    'Păstrează emailul: fișierul îți trebuie din nou dacă reinstalezi Valet sau schimbi calculatorul.',
    'Pentru asistență, scrie numărul comenzii de pe chitanța de plată. Nu publica și nu trimite nimănui fișierul licenței: cu el oricine poate activa licența.',
    '',
    '---',
    '',
    'Your Valet Pro license is attached to this email (the file valet-pro.valet-license).',
    '',
    'To activate it: open Valet → Settings → Valet Pro → I have a license, and pick the attached file.',
    `The license can be active on up to ${n} computers and includes updates published until ${until}.`,
    'Keep this email: you need the file again if you reinstall Valet or change computers.',
    'For support, quote the order number from your payment receipt. Do not post or send the license file to anyone: whoever has it can activate the license.',
    '',
  ].join('\n');
  return { subject: 'Licența Valet Pro / Your Valet Pro license', text };
}

export class MailError extends Error {
  constructor(message) {
    super(message);
    this.name = 'MailError';
  }
}

/**
 * Sends the license file. Resolves on a 2xx answer, throws MailError otherwise.
 * The error message carries the HTTP status only, never the address or the API key.
 */
export async function sendLicenseEmail({ fetch: fetchFn = fetch, apiKey, from, replyTo }, { to, payload, envelope }) {
  const { subject, text } = licenseMessage(payload);
  const body = {
    from,
    to: [to],
    subject,
    text,
    attachments: [
      {
        filename: ATTACHMENT_NAME,
        content: Buffer.from(`${JSON.stringify(envelope, null, 2)}\n`, 'utf8').toString('base64'),
      },
    ],
  };
  if (replyTo) body.reply_to = replyTo;
  let res;
  try {
    res = await fetchFn(RESEND_URL, {
      method: 'POST',
      headers: { Authorization: `Bearer ${apiKey}`, 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
      signal: AbortSignal.timeout(TIMEOUT_MS),
    });
  } catch (err) {
    throw new MailError(`resend request failed (${err?.name ?? 'Error'})`);
  }
  // Drain the body so the connection can be reused; its content is not needed.
  await res.text().catch(() => {});
  if (!res.ok) throw new MailError(`resend answered ${res.status}`);
}
