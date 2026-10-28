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
        const label = t(`backendError.${parsed.code}`, { defaultValue: '' });
        if (label !== '') {
            return parsed.detail ? `${label} (${parsed.detail})` : label;
        }
    }
    return errorMessage(err);
}

/** Like `commandErrorMessage`, with untranslated text prefixed by "Eroare:". */
export function commandErrorText(t: TFunction, err: unknown): string {
    const parsed = parseCommandError(err);
    if (parsed !== null && t(`backendError.${parsed.code}`, { defaultValue: '' }) !== '') {
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
