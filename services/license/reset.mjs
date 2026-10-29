#!/usr/bin/env node
// Support reset: clears the active machines of a license (the old PC is gone and can't
// deactivate). Machine history and the record are kept; the user activates again.
//   node services/license/reset.mjs --license <uuid> [--store path.json]

import { parseArgs } from 'node:util';
import { createJsonFileStore, resetActiveMachines } from './lib/store.mjs';
import { UUID_V4, nowSeconds } from './lib/license.mjs';

function fail(message) {
  process.stderr.write(`reset: ${message}\n`);
  process.exit(2);
}

let args;
try {
  args = parseArgs({
    options: { license: { type: 'string' }, store: { type: 'string' } },
    strict: true,
  }).values;
} catch (err) {
  fail(err.message);
}

const id = args.license;
if (!id || !UUID_V4.test(id)) fail('--license must be a lowercase UUID v4');
const storePath = args.store ?? process.env.VALET_LICENSE_STORE;
if (!storePath) fail('--store or VALET_LICENSE_STORE is required');

const store = createJsonFileStore(storePath);
const record = await store.get(id);
if (!record) fail('license not found in the store');
const cleared = resetActiveMachines(record, nowSeconds());
if (cleared > 0) await store.put(id, record);
process.stderr.write(`reset: license ${id}, ${cleared} active machine(s) cleared\n`);
