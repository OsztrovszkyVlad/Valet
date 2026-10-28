// Pure helpers for check-i18n.mjs, kept separate so they can be tested.

/**
 * Collects the dotted keys of a translation tree into `out`. Leaves that are
 * not non-empty strings are reported in `errors` and not collected.
 */
export function flattenKeys(obj, file, errors, prefix = '', out = new Set()) {
    for (const [k, v] of Object.entries(obj)) {
        const key = prefix ? `${prefix}.${k}` : k;
        if (v !== null && typeof v === 'object' && !Array.isArray(v)) {
            flattenKeys(v, file, errors, key, out);
        } else if (typeof v === 'string' && v.trim() !== '') {
            out.add(key);
        } else {
            errors.push(`${file}: ${key} is not a non-empty string`);
        }
    }
    return out;
}

/** i18next plural suffixes; the forms a language needs differ (ro has `_few`, en does not). */
const PLURAL = /_(zero|one|two|few|many|other)$/;

/**
 * Keys present in one language and missing in the other. A plural key
 * (`x_one`, `x_few`, ...) only needs `x_other` in the other language.
 */
export function parityErrors(ro, en) {
    const errors = [];
    const missing = (k, other) =>
        PLURAL.test(k) ? !other.has(k.replace(PLURAL, '_other')) : !other.has(k);
    const report = (k, other, file) => {
        const key = PLURAL.test(k) ? k.replace(PLURAL, '_other') : k;
        const line = `${file}: missing ${key}`;
        if (!errors.includes(line)) errors.push(line);
    };
    for (const k of ro) if (missing(k, en)) report(k, en, 'en.json');
    for (const k of en) if (missing(k, ro)) report(k, ro, 'ro.json');
    return errors;
}

/** Literal keys used in source text: t('a.b') and t("a.b"). */
export function usedKeys(text) {
    return [...text.matchAll(/\bt\(\s*['"]([A-Za-z0-9_.]+)['"]/g)].map((m) => m[1]);
}

/** Literal keys in `text` that `known` does not contain (as a key or a plural `key_other`), as error lines. */
export function unknownKeyErrors(text, file, known) {
    return usedKeys(text)
        .filter((k) => !known.has(k) && !known.has(`${k}_other`))
        .map((k) => `src/${file}: key ${k} not in ro.json`);
}
