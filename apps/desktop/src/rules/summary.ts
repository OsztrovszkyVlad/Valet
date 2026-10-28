// Short human text for a rule, e.g. "PDF cu «factura» în nume → Documente/Facturi/{year}".
import type { TFunction } from 'i18next';
import type { Rule, RuleAction, RuleCondition } from '../api/commands';
import { formatBytes } from '../i18n/format';
import { normalizeExt } from './model';

/** Folder tokens a path or scope entry may start with. */
export const FOLDER_TOKENS = ['downloads', 'desktop', 'documents', 'pictures', 'videos', 'music', 'home'] as const;

/** `{documents}/Facturi/{year}` → `Documente/Facturi/{year}`; other paths unchanged. */
export function displayPath(t: TFunction, path: string): string {
    const m = /^\{([a-z]+)\}(.*)$/.exec(path.trim());
    if (m === null || !(FOLDER_TOKENS as readonly string[]).includes(m[1])) return path;
    return t(`folder.${m[1]}`) + m[2];
}

/** Relative to the watched folder: no folder token, `~`, `%VAR%`, drive or root. */
function isRelative(path: string): boolean {
    return path !== '' && !/^(\{|~|%|[\\/]|[A-Za-z]:)/.test(path.trim());
}

/** Longest pattern shown in full in a summary. */
const MAX_PATTERN = 40;

function list(t: TFunction, items: string[]): string {
    return items.join(t('summary.or'));
}

function strings(v: unknown): string[] {
    if (typeof v === 'string') return [v];
    if (Array.isArray(v)) return v.filter((x): x is string => typeof x === 'string');
    return [];
}

function exts(v: unknown): string {
    return strings(v)
        .map((e) => normalizeExt(e).toUpperCase())
        .join(', ');
}

export function conditionSummary(t: TFunction, c: RuleCondition, lng?: string): string {
    switch (c.type) {
        case 'ext':
        case 'extIn':
            return exts(c.value);
        case 'extNotIn':
            return t('summary.extNotIn', { exts: exts(c.value) });
        case 'typeGroup':
            return t(`typeGroupPlural.${String(c.value)}`, { defaultValue: String(c.value) });
        case 'nameContains':
            return t('summary.nameContains', {
                words: list(t, strings(c.value).map((w) => t('summary.quoted', { text: w }))),
            });
        case 'nameMatches': {
            const pattern = String(c.value);
            return t('summary.nameMatches', {
                pattern: pattern.length > MAX_PATTERN ? `${pattern.slice(0, MAX_PATTERN)}…` : pattern,
            });
        }
        case 'pathContains':
            return t('summary.pathContains', { text: t('summary.quoted', { text: String(c.value) }) });
        case 'sizeGt':
            return t('summary.sizeGt', { size: formatBytes(Number(c.value), lng) });
        case 'sizeLt':
            return t('summary.sizeLt', { size: formatBytes(Number(c.value), lng) });
        case 'modifiedOlderThanDays':
            return t('summary.modifiedOlder', { count: Number(c.value) });
        case 'createdOlderThanDays':
            return t('summary.createdOlder', { count: Number(c.value) });
        case 'emptyFolder':
            return t('summary.emptyFolder');
        case 'duplicate':
            return t('summary.duplicate');
        default:
            return c.type;
    }
}

export function actionSummary(t: TFunction, a: RuleAction): string {
    switch (a.type) {
        case 'moveTo':
        case 'copyTo': {
            const params = a.params as { path?: unknown } | null | undefined;
            const raw = typeof params?.path === 'string' ? params.path : '';
            const path = isRelative(raw) ? t('summary.relative', { path: raw }) : displayPath(t, raw);
            return a.type === 'moveTo' ? path : t('summary.copyTo', { path });
        }
        case 'trash':
            return t('summary.trash');
        case 'report':
            return t('summary.report');
        default:
            return a.type;
    }
}

function capitalize(s: string): string {
    return s.charAt(0).toLocaleUpperCase() + s.slice(1);
}

/** One line: conditions, an arrow, the actions. */
export function ruleSummary(t: TFunction, rule: Rule, lng?: string): string {
    const conditions =
        rule.alwaysApply || rule.conditions.length === 0
            ? t('summary.allFiles')
            : rule.conditions.map((c) => conditionSummary(t, c, lng)).join(', ');
    const actions = rule.actions.map((a) => actionSummary(t, a)).join(', ');
    return capitalize(t('summary.rule', { conditions, actions }));
}
