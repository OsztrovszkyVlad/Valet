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

/** Keys present in one language and missing in the other. */
export function parityErrors(ro, en) {
    const errors = [];
    for (const k of ro) if (!en.has(k)) errors.push(`en.json: missing ${k}`);
    for (const k of en) if (!ro.has(k)) errors.push(`ro.json: missing ${k}`);
    return errors;
}

/** Literal keys used in source text: t('a.b') and t("a.b"). */
export function usedKeys(text) {
    return [...text.matchAll(/\bt\(\s*['"]([A-Za-z0-9_.]+)['"]/g)].map((m) => m[1]);
}

/** Literal keys in `text` that `known` does not contain, as error lines. */
export function unknownKeyErrors(text, file, known) {
    return usedKeys(text)
        .filter((k) => !known.has(k))
        .map((k) => `src/${file}: key ${k} not in ro.json`);
}
