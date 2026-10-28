import type { TFunction } from 'i18next';
import type { OpError } from '../api/commands';

/** Date and time in the user's locale. */
export function formatDateTime(iso: string): string {
    const d = new Date(iso);
    if (Number.isNaN(d.getTime())) {
        return iso;
    }
    return d.toLocaleString(undefined, { dateStyle: 'medium', timeStyle: 'medium' });
}

function codeText(t: TFunction, code: string, detail: string | null): string {
    const key = `failCode.${code}`;
    const label = t(key, { defaultValue: '' });
    const base = label === '' ? code : label;
    return detail ? `${base} (${detail})` : base;
}

/** Translated text for a journal `error` column (`Code` or `Code: detail`). */
export function journalErrorText(t: TFunction, error: string): string {
    const sep = error.indexOf(': ');
    const code = sep === -1 ? error : error.slice(0, sep);
    const detail = sep === -1 ? null : error.slice(sep + 2);
    return codeText(t, code, detail);
}

export function opErrorText(t: TFunction, error: OpError): string {
    return codeText(t, error.code, error.detail);
}
