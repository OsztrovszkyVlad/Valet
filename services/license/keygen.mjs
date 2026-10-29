#!/usr/bin/env node
// Prints a new license signing key pair to stdout. Writes nothing to disk.
// The secret goes only into the host's secret store. The public key line goes into
// crates/valet-core/license/pubkey.txt (committed by Vlad).

import { generateKeyPair, exportSecretKey, publicKeyLine, keyIdDisplay } from './lib/signing.mjs';

const key = generateKeyPair();
const out = [
  `# Valet license signing key, generated ${new Date().toISOString()}`,
  `# minisign key id ${keyIdDisplay(key.keyId)}`,
  '#',
  '# Public key line, for crates/valet-core/license/pubkey.txt:',
  publicKeyLine(key),
  '#',
  '# Server environment. VALET_LICENSE_SECRET_KEY is the secret: put it only in the',
  "# host's secret store. Do not save it to a file, paste it in chat, or commit it.",
  `VALET_LICENSE_KEY_ID=${key.keyId.toString('hex')}`,
  `VALET_LICENSE_SECRET_KEY=${exportSecretKey(key.privateKey)}`,
];
process.stdout.write(`${out.join('\n')}\n`);
