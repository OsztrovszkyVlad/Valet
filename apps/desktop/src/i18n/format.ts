import type { TFunction } from 'i18next';
import {
    errorMessage,
    parseCommandError,
    type OpError,
    type SkippedFile,
} from '../api/commands';

/** Date and time in the UI language (`i18n.language`), not the OS locale. */
export function formatDateTime(iso: string, lng: string): string {
    const d = new Date(iso);
    if (Number.isNaN(d.getTime())) {
        return iso;
    }
    return d.toLocaleString(lng, { dateStyle: 'medium', timeStyle: 'medium' });
}

/** A Unix time (seconds) as a date in the UI language, e.g. `29 octombrie 2027`. */
export function formatUnixDate(seconds: number, lng: string): string {
    return new Date(seconds * 1000).toLocaleDateString(lng, { dateStyle: 'long' });
}

/** A Unix time (seconds) as date and time in the UI language. */
export function formatUnixDateTime(seconds: number, lng: string): string {
    return new Date(seconds * 1000).toLocaleString(lng, { dateStyle: 'long', timeStyle: 'short' });
}

/** Whole days left until `endsAt` (Unix seconds), rounded up; 0 once passed. */
export function daysLeft(endsAt: number, nowMs: number = Date.now()): number {
    return Math.max(0, Math.ceil((endsAt * 1000 - nowMs) / 86_400_000));
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

/**
 * The translated label for a rejected command whose code has an entry under
 * `backendError` (for example `config.driveRoot`), with its detail; the text
 * as the backend sent it otherwise.
 */
export function commandErrorMessage(t: TFunction, err: unknown): string {
    const parsed = parseCommandError(err);
    if (parsed !== null) {
        const label = backendErrorLabel(t, parsed.code);
        if (label !== '') {
            return parsed.detail ? `${label} (${parsed.detail})` : label;
        }
    }
    return errorMessage(err);
}

/**
 * The `backendError` key of a code. Snake_case parts (`license.too_many`) are
 * stored camelCase (`license.tooMany`): a key ending in `_many` or `_other`
 * would read as an i18next plural form.
 */
export function backendErrorKey(code: string): string {
    return `backendError.${code.replace(/_([a-z0-9])/g, (_, c: string) => c.toUpperCase())}`;
}

/** The translated label of a backend error code, or `''` when there is none. */
export function backendErrorLabel(t: TFunction, code: string): string {
    return t(backendErrorKey(code), { defaultValue: '' });
}

/** Like `commandErrorMessage`, with untranslated text prefixed by "Eroare:". */
export function commandErrorText(t: TFunction, err: unknown): string {
    const parsed = parseCommandError(err);
    if (parsed !== null && backendErrorLabel(t, parsed.code) !== '') {
        return commandErrorMessage(t, err);
    }
    return t('common.error', { message: errorMessage(err) });
}

/** Why a file was skipped, in the UI language. */
export function skipReasonText(t: TFunction, skipped: SkippedFile): string {
    if (skipped.reason === 'noExtension') {
        return skipped.ext
            ? t('skipReason.noExtension', { ext: skipped.ext })
            : t('skipReason.noExtensionPlain');
    }
    return t(`skipReason.${skipped.reason}`, {
        defaultValue: t('skipReason.unknown', { reason: skipped.reason }),
    });
}

const BYTE_UNITS = ['B', 'KB', 'MB', 'GB', 'TB'] as const;

/** `1073741824` → `1 GB`; one decimal at most, in the given locale. */
export function formatBytes(bytes: number, lng?: string): string {
    let value = bytes;
    let unit = 0;
    while (value >= 1024 && unit < BYTE_UNITS.length - 1) {
        value /= 1024;
        unit += 1;
    }
    const n = new Intl.NumberFormat(lng, { maximumFractionDigits: unit === 0 ? 0 : 1 }).format(value);
    return `${n} ${BYTE_UNITS[unit]}`;
}
