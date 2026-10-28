// Fails when ro.json and en.json do not have the same keys, a value is not a
// string, or a literal t('key') in src/ has no entry.
import { readdirSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const dir = join(dirname(fileURLToPath(import.meta.url)), '..', 'src', 'i18n');

function flatten(obj, prefix, out, errors, file) {
    for (const [k, v] of Object.entries(obj)) {
        const key = prefix ? `${prefix}.${k}` : k;
        if (v !== null && typeof v === 'object' && !Array.isArray(v)) {
            flatten(v, key, out, errors, file);
        } else if (typeof v === 'string' && v.trim() !== '') {
            out.add(key);
        } else {
            errors.push(`${file}: ${key} is not a non-empty string`);
        }
    }
    return out;
}

const errors = [];
const load = (file) =>
    flatten(JSON.parse(readFileSync(join(dir, file), 'utf8')), '', new Set(), errors, file);
const ro = load('ro.json');
const en = load('en.json');

for (const k of ro) if (!en.has(k)) errors.push(`en.json: missing ${k}`);
for (const k of en) if (!ro.has(k)) errors.push(`ro.json: missing ${k}`);

// Literal keys used in the code: t('a.b') and t("a.b").
const src = join(dir, '..');
for (const entry of readdirSync(src, { recursive: true })) {
    const file = String(entry);
    if (!/\.(ts|tsx)$/.test(file)) continue;
    const text = readFileSync(join(src, file), 'utf8');
    for (const m of text.matchAll(/\bt\(\s*['"]([A-Za-z0-9_.]+)['"]/g)) {
        if (!ro.has(m[1])) errors.push(`src/${file}: key ${m[1]} not in ro.json`);
    }
}

if (errors.length > 0) {
    for (const e of errors) console.error(e);
    process.exit(1);
}
console.log(`i18n ok: ${ro.size} keys in ro.json and en.json`);
