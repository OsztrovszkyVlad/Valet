import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { flattenKeys, parityErrors, unknownKeyErrors, usedKeys } from './i18n-keys.mjs';

describe('flattenKeys', () => {
    it('collects nested keys with dots', () => {
        const errors = [];
        const keys = flattenKeys({ a: { b: 'x', c: { d: 'y' } }, e: 'z' }, 'ro.json', errors);
        expect([...keys].sort()).toEqual(['a.b', 'a.c.d', 'e']);
        expect(errors).toEqual([]);
    });

    it('reports empty strings, numbers and arrays', () => {
        const errors = [];
        const keys = flattenKeys({ a: '', b: '  ', c: 3, d: ['x'], e: 'ok' }, 'en.json', errors);
        expect([...keys]).toEqual(['e']);
        expect(errors).toEqual([
            'en.json: a is not a non-empty string',
            'en.json: b is not a non-empty string',
            'en.json: c is not a non-empty string',
            'en.json: d is not a non-empty string',
        ]);
    });
});

describe('parityErrors', () => {
    it('lists keys missing on either side', () => {
        const ro = new Set(['a', 'b.c']);
        const en = new Set(['a', 'd']);
        expect(parityErrors(ro, en)).toEqual(['en.json: missing b.c', 'ro.json: missing d']);
    });

    it('is empty for equal sets', () => {
        expect(parityErrors(new Set(['a']), new Set(['a']))).toEqual([]);
    });
});

describe('usedKeys and unknownKeyErrors', () => {
    const text = `t('nav.preview'); t("settings.title", { x: 1 }); t( 'missing.key' ); t(\`dyn.\${x}\`); at('no')`;

    it('finds literal keys only', () => {
        expect(usedKeys(text)).toEqual(['nav.preview', 'settings.title', 'missing.key']);
    });

    it('reports keys that are not known', () => {
        const known = new Set(['nav.preview', 'settings.title']);
        expect(unknownKeyErrors(text, 'App.tsx', known)).toEqual([
            'src/App.tsx: key missing.key not in ro.json',
        ]);
    });
});

describe('shipped translations', () => {
    it('ro.json and en.json have the same non-empty keys', () => {
        const dir = join(dirname(fileURLToPath(import.meta.url)), '..', 'src', 'i18n');
        const errors = [];
        const load = (f) => flattenKeys(JSON.parse(readFileSync(join(dir, f), 'utf8')), f, errors);
        const ro = load('ro.json');
        const en = load('en.json');
        expect(errors).toEqual([]);
        expect(parityErrors(ro, en)).toEqual([]);
    });
});
