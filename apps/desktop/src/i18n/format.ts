import type { TFunction } from 'i18next';
import {
    errorMessage,
    parseCommandError,
    type OpError,
    type SkippedFile,
} from '../api/commands';

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

/**
 * Text for a rejected command. Codes with an entry under `backendError`
 * (for example `config.driveRoot`) are translated; anything else is shown
 * as the backend sent it.
 */
export function commandErrorText(t: TFunction, err: unknown): string {
    const parsed = parseCommandError(err);
    if (parsed !== null) {
        const label = t(`backendError.${parsed.code}`, { defaultValue: '' });
        if (label !== '') {
            return parsed.detail ? `${label} (${parsed.detail})` : label;
        }
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
