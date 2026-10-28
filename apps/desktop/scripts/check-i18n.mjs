// Fails when ro.json and en.json do not have the same keys, a value is not a
// string, or a literal t('key') in src/ has no entry.
import { readdirSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { flattenKeys, parityErrors, unknownKeyErrors } from './i18n-keys.mjs';

const dir = join(dirname(fileURLToPath(import.meta.url)), '..', 'src', 'i18n');

const errors = [];
const load = (file) => flattenKeys(JSON.parse(readFileSync(join(dir, file), 'utf8')), file, errors);
const ro = load('ro.json');
const en = load('en.json');
errors.push(...parityErrors(ro, en));

const src = join(dir, '..');
for (const entry of readdirSync(src, { recursive: true })) {
    const file = String(entry);
    if (!/\.(ts|tsx)$/.test(file)) continue;
    errors.push(...unknownKeyErrors(readFileSync(join(src, file), 'utf8'), file, ro));
}

if (errors.length > 0) {
    for (const e of errors) console.error(e);
    process.exit(1);
}
console.log(`i18n ok: ${ro.size} keys in ro.json and en.json`);
