// Editor model for rules and its conversion to and from the A1 rule JSON
// (`src-tauri/src/commands/rules.rs`). The editor works on drafts with
// text fields; `draftToRule` checks them and builds the JSON `upsert_rule`
// accepts. `ruleToDraft(draftToRule(d))` gives `d` back for valid drafts.
import {
    TYPE_GROUPS,
    type ConditionType,
    type Rule,
    type RuleAction,
    type RuleCondition,
    type TypeGroup,
} from '../api/commands';

export const SIZE_UNITS = ['KB', 'MB', 'GB'] as const;
export type SizeUnit = (typeof SIZE_UNITS)[number];

export const UNIT_BYTES: Record<SizeUnit, number> = {
    KB: 1024,
    MB: 1024 ** 2,
    GB: 1024 ** 3,
};

/** Largest accepted `*OlderThanDays` value, as in the backend. */
export const MAX_DAYS = 36_500;

export type ConditionDraft =
    | { type: 'ext' | 'extIn' | 'extNotIn'; exts: string[] }
    | { type: 'nameContains'; words: string[] }
    | { type: 'nameMatches' | 'pathContains'; text: string }
    | { type: 'sizeGt' | 'sizeLt'; amount: string; unit: SizeUnit }
    | { type: 'modifiedOlderThanDays' | 'createdOlderThanDays'; days: string }
    | { type: 'typeGroup'; group: TypeGroup }
    | { type: 'emptyFolder' | 'duplicate' };

export type ActionDraft = { type: 'moveTo' | 'copyTo'; path: string } | { type: 'trash' } | { type: 'report' };

export interface RuleDraft {
    id: string;
    name: string;
    enabled: boolean;
    alwaysApply: boolean;
    conditions: ConditionDraft[];
    action: ActionDraft;
    /** Actions after the first one, kept unchanged (the editor edits one action). */
    extraActions: RuleAction[];
    includeHidden: boolean;
    topLevelOnly: boolean;
    /** Option keys the editor does not show (for example a preset id), kept unchanged. */
    otherOptions: Record<string, unknown>;
    scope: string[];
    version: number;
}

/** Problems found in a draft, as i18n keys under `ruleEditor.error`. */
export interface DraftErrors {
    name?: string;
    conditions: Record<number, string>;
    action?: string;
    general?: string;
}

export function hasErrors(e: DraftErrors): boolean {
    return (
        e.name !== undefined ||
        e.action !== undefined ||
        e.general !== undefined ||
        Object.keys(e.conditions).length > 0
    );
}

/** Empty value for a condition type, keeping extensions or texts when the input kind stays the same. */
export function defaultCondition(type: ConditionType, from?: ConditionDraft): ConditionDraft {
    switch (type) {
        case 'ext':
        case 'extIn':
        case 'extNotIn': {
            const exts = from && 'exts' in from ? from.exts : [];
            return { type, exts: type === 'ext' ? exts.slice(0, 1) : exts };
        }
        case 'nameContains':
            return { type, words: from && 'words' in from ? from.words : [] };
        case 'nameMatches':
        case 'pathContains':
            return { type, text: from && 'text' in from ? from.text : '' };
        case 'sizeGt':
        case 'sizeLt':
            return from && 'amount' in from ? { ...from, type } : { type, amount: '', unit: 'MB' };
        case 'modifiedOlderThanDays':
        case 'createdOlderThanDays':
            return { type, days: from && 'days' in from ? from.days : '' };
        case 'typeGroup':
            return { type, group: 'document' };
        case 'emptyFolder':
        case 'duplicate':
            return { type };
    }
}

export function newDraft(id: string): RuleDraft {
    return {
        id,
        name: '',
        enabled: true,
        alwaysApply: false,
        conditions: [defaultCondition('extIn')],
        action: { type: 'moveTo', path: '' },
        extraActions: [],
        includeHidden: false,
        topLevelOnly: false,
        otherOptions: {},
        scope: [],
        version: 1,
    };
}

/** `" .PDF "` → `"pdf"`. */
export function normalizeExt(raw: string): string {
    return raw.trim().replace(/^\.+/, '').toLowerCase();
}

/** Bytes as an amount and unit: the largest unit that divides exactly, else KB with decimals. */
export function bytesToSize(bytes: number): { amount: string; unit: SizeUnit } {
    for (const unit of ['GB', 'MB', 'KB'] as const) {
        const b = UNIT_BYTES[unit];
        if (bytes >= b && bytes % b === 0) {
            return { amount: String(bytes / b), unit };
        }
    }
    return { amount: String(bytes / UNIT_BYTES.KB), unit: 'KB' };
}

/** `"1,5"` or `"1.5"` with a unit → bytes; null when not a number >= 0. */
export function sizeToBytes(amount: string, unit: SizeUnit): number | null {
    const text = amount.trim().replace(',', '.');
    if (!/^\d+(\.\d+)?$/.test(text)) return null;
    const bytes = Math.round(Number(text) * UNIT_BYTES[unit]);
    return Number.isSafeInteger(bytes) ? bytes : null;
}

function asStringList(v: unknown): string[] | null {
    if (typeof v === 'string') return [v];
    if (Array.isArray(v) && v.every((x) => typeof x === 'string')) return v as string[];
    return null;
}

function isTypeGroup(v: unknown): v is TypeGroup {
    return typeof v === 'string' && (TYPE_GROUPS as readonly string[]).includes(v);
}

/** One stored condition as a draft; null when the type or value is not one the editor knows. */
export function conditionToDraft(c: RuleCondition): ConditionDraft | null {
    switch (c.type) {
        case 'ext':
            return typeof c.value === 'string' ? { type: 'ext', exts: [normalizeExt(c.value)] } : null;
        case 'extIn':
        case 'extNotIn': {
            const list = Array.isArray(c.value) ? asStringList(c.value) : null;
            return list ? { type: c.type, exts: list.map(normalizeExt) } : null;
        }
        case 'nameContains': {
            const list = asStringList(c.value);
            return list ? { type: 'nameContains', words: list } : null;
        }
        case 'nameMatches':
        case 'pathContains':
            return typeof c.value === 'string' ? { type: c.type, text: c.value } : null;
        case 'sizeGt':
        case 'sizeLt':
            return typeof c.value === 'number' && Number.isSafeInteger(c.value) && c.value >= 0
                ? { type: c.type, ...bytesToSize(c.value) }
                : null;
        case 'modifiedOlderThanDays':
        case 'createdOlderThanDays':
            return typeof c.value === 'number' && Number.isInteger(c.value)
                ? { type: c.type, days: String(c.value) }
                : null;
        case 'typeGroup':
            return isTypeGroup(c.value) ? { type: 'typeGroup', group: c.value } : null;
        case 'emptyFolder':
        case 'duplicate':
            return c.value === true ? { type: c.type } : null;
        default:
            return null;
    }
}

function actionToDraft(a: RuleAction | undefined): ActionDraft | null {
    if (a === undefined) return null;
    switch (a.type) {
        case 'moveTo':
        case 'copyTo': {
            const params = a.params as { path?: unknown } | null | undefined;
            const path = params && typeof params.path === 'string' ? params.path : '';
            return { type: a.type, path };
        }
        case 'trash':
        case 'report':
            return { type: a.type };
        default:
            return null;
    }
}

/** A stored rule as a draft; null when it holds something the editor cannot show. */
export function ruleToDraft(rule: Rule): RuleDraft | null {
    const conditions: ConditionDraft[] = [];
    for (const c of rule.conditions) {
        const d = conditionToDraft(c);
        if (d === null) return null;
        conditions.push(d);
    }
    const action = actionToDraft(rule.actions[0]);
    if (action === null) return null;
    const { includeHidden, topLevelOnly, ...otherOptions } = rule.options ?? {};
    return {
        id: rule.id,
        name: rule.name,
        enabled: rule.enabled,
        alwaysApply: rule.alwaysApply,
        conditions,
        action,
        extraActions: rule.actions.slice(1),
        includeHidden: includeHidden === true,
        topLevelOnly: topLevelOnly === true,
        otherOptions,
        scope: [...rule.scope],
        version: rule.version,
    };
}

function days(text: string): number | null {
    const t = text.trim();
    if (!/^\d+$/.test(t)) return null;
    const n = Number(t);
    return n <= MAX_DAYS ? n : null;
}

/** The condition as JSON, or the i18n key (under `ruleEditor.error`) of what is wrong. */
export function conditionToJson(d: ConditionDraft): RuleCondition | { error: string } {
    switch (d.type) {
        case 'ext': {
            const ext = d.exts.map(normalizeExt).filter((e) => e !== '');
            return ext.length === 1 ? { type: 'ext', value: ext[0] } : { error: 'extOne' };
        }
        case 'extIn':
        case 'extNotIn': {
            const exts = d.exts.map(normalizeExt).filter((e) => e !== '');
            return exts.length > 0 ? { type: d.type, value: exts } : { error: 'extEmpty' };
        }
        case 'nameContains': {
            const words = d.words.map((w) => w.trim()).filter((w) => w !== '');
            if (words.length === 0) return { error: 'textEmpty' };
            return { type: 'nameContains', value: words.length === 1 ? words[0] : words };
        }
        case 'nameMatches':
            return d.text === '' ? { error: 'regexEmpty' } : { type: d.type, value: d.text };
        case 'pathContains':
            return d.text.trim() === '' ? { error: 'textEmpty' } : { type: d.type, value: d.text.trim() };
        case 'sizeGt':
        case 'sizeLt': {
            const bytes = sizeToBytes(d.amount, d.unit);
            return bytes === null ? { error: 'size' } : { type: d.type, value: bytes };
        }
        case 'modifiedOlderThanDays':
        case 'createdOlderThanDays': {
            const n = days(d.days);
            return n === null ? { error: 'days' } : { type: d.type, value: n };
        }
        case 'typeGroup':
            return { type: 'typeGroup', value: d.group };
        case 'emptyFolder':
        case 'duplicate':
            return { type: d.type, value: true };
    }
}

function actionToJson(a: ActionDraft): RuleAction | { error: string } {
    switch (a.type) {
        case 'moveTo':
        case 'copyTo':
            return a.path.trim() === '' ? { error: 'destinationEmpty' } : { type: a.type, params: { path: a.path.trim() } };
        case 'trash':
        case 'report':
            return { type: a.type, params: {} };
    }
}

export type DraftResult = { ok: true; rule: Rule } | { ok: false; errors: DraftErrors };

/**
 * Checks the fields the editor can check by itself and builds the rule JSON.
 * Combination rules (for example `duplicate` with `moveTo`) are left to
 * `upsert_rule`, whose error code the editor shows.
 */
export function draftToRule(d: RuleDraft): DraftResult {
    const errors: DraftErrors = { conditions: {} };
    if (d.name.trim() === '') errors.name = 'nameEmpty';

    const conditions: RuleCondition[] = [];
    if (!d.alwaysApply) {
        d.conditions.forEach((c, i) => {
            const json = conditionToJson(c);
            if ('error' in json) errors.conditions[i] = json.error;
            else conditions.push(json);
        });
        if (d.conditions.length === 0) errors.general = 'noConditions';
    }

    const action = actionToJson(d.action);
    if ('error' in action) errors.action = action.error;

    if (hasErrors(errors) || 'error' in action) return { ok: false, errors };

    const options: Record<string, unknown> = { ...d.otherOptions };
    if (d.includeHidden) options.includeHidden = true;
    if (d.topLevelOnly) options.topLevelOnly = true;

    return {
        ok: true,
        rule: {
            id: d.id,
            name: d.name.trim(),
            enabled: d.enabled,
            alwaysApply: d.alwaysApply,
            conditions,
            actions: [action, ...d.extraActions],
            version: d.version,
            options,
            scope: [...d.scope],
        },
    };
}

/** Option key under which preset rules carry the id of their preset. */
export const PRESET_OPTION = 'presetId';

/** Id of the preset a rule was installed from, or null for rules the user made. */
export function presetIdOf(rule: Rule): string | null {
    const v = rule.options?.[PRESET_OPTION];
    return typeof v === 'string' && v !== '' ? v : null;
}
