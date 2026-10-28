import { describe, expect, it } from 'vitest';
import { CONDITION_TYPES, type Rule, type RuleAction, type RuleCondition } from '../api/commands';
import {
    bytesToSize,
    conditionToDraft,
    conditionToJson,
    defaultCondition,
    draftToRule,
    newDraft,
    presetIdOf,
    ruleToDraft,
    sizeToBytes,
    type ConditionDraft,
    type RuleDraft,
} from './model';

const ID = '00000000-0000-0000-0000-000000000007';

function rule(conditions: RuleCondition[], actions: RuleAction[], extra: Partial<Rule> = {}): Rule {
    return {
        id: ID,
        name: 'Regula',
        enabled: true,
        alwaysApply: false,
        conditions,
        actions,
        version: 3,
        options: {},
        scope: [],
        ...extra,
    };
}

/** One canonical stored value per condition type (as `upsert_rule` accepts it). */
const CONDITIONS: Record<(typeof CONDITION_TYPES)[number], unknown[]> = {
    ext: ['pdf'],
    extIn: [['jpg', 'png']],
    extNotIn: [['exe']],
    typeGroup: ['image', 'document', 'archive', 'installer', 'video', 'audio', 'code'],
    nameContains: ['factura', ['invoice', 'bon']],
    nameMatches: ['(?i)^scan_\\d+'],
    pathContains: ['work'],
    sizeGt: [0, 1500, 1024, 5 * 1024 ** 2, 1024 ** 3, 3 * 1024 ** 3],
    sizeLt: [512 * 1024],
    modifiedOlderThanDays: [0, 30, 36_500],
    createdOlderThanDays: [7],
    emptyFolder: [true],
    duplicate: [true],
};

const ACTIONS: RuleAction[] = [
    { type: 'moveTo', params: { path: '{documents}/Facturi/{year}' } },
    { type: 'copyTo', params: { path: 'D:\\Backup\\{ext}' } },
    { type: 'trash', params: {} },
    { type: 'report', params: {} },
];

describe('rule JSON ↔ editor draft', () => {
    it('covers every condition type the backend accepts', () => {
        expect(Object.keys(CONDITIONS).sort()).toEqual([...CONDITION_TYPES].sort());
    });

    for (const [type, values] of Object.entries(CONDITIONS)) {
        for (const value of values) {
            it(`round-trips ${type} ${JSON.stringify(value)}`, () => {
                const stored = rule([{ type, value }], [{ type: 'report', params: {} }]);
                const draft = ruleToDraft(stored);
                expect(draft).not.toBeNull();
                const back = draftToRule(draft as RuleDraft);
                expect(back).toEqual({ ok: true, rule: stored });
                // And the draft survives a second pass.
                expect(ruleToDraft((back as { rule: Rule }).rule)).toEqual(draft);
            });
        }
    }

    for (const action of ACTIONS) {
        it(`round-trips action ${action.type}`, () => {
            const stored = rule([{ type: 'ext', value: 'pdf' }], [action]);
            const back = draftToRule(ruleToDraft(stored) as RuleDraft);
            expect(back).toEqual({ ok: true, rule: stored });
        });
    }

    it('keeps scope, options, unknown option keys, alwaysApply, extra actions and version', () => {
        const stored = rule(
            [],
            [
                { type: 'report', params: {} },
                { type: 'trash', params: {} },
            ],
            {
                alwaysApply: true,
                enabled: false,
                options: { includeHidden: true, topLevelOnly: true, presetId: 'screenshots' },
                scope: ['{downloads}', 'C:\\Users\\ana\\Desktop'],
                version: 9,
            },
        );
        const draft = ruleToDraft(stored) as RuleDraft;
        expect(draft.includeHidden).toBe(true);
        expect(draft.topLevelOnly).toBe(true);
        expect(draft.otherOptions).toEqual({ presetId: 'screenshots' });
        expect(draft.extraActions).toEqual([{ type: 'trash', params: {} }]);
        expect(draftToRule(draft)).toEqual({ ok: true, rule: stored });
        expect(presetIdOf(stored)).toBe('screenshots');
    });

    it('reads null options and drops false flags', () => {
        const stored = rule([{ type: 'ext', value: 'pdf' }], [{ type: 'report' }], {
            options: null,
        });
        const draft = ruleToDraft(stored) as RuleDraft;
        const back = draftToRule(draft);
        expect(back.ok && back.rule.options).toEqual({});
        expect(back.ok && back.rule.actions).toEqual([{ type: 'report', params: {} }]);
        expect(presetIdOf(stored)).toBeNull();
    });

    it('normalizes extensions and trims texts', () => {
        expect(conditionToDraft({ type: 'ext', value: '.PDF' })).toEqual({ type: 'ext', exts: ['pdf'] });
        expect(conditionToJson({ type: 'extIn', exts: ['.JPG', ' png ', ''] })).toEqual({
            type: 'extIn',
            value: ['jpg', 'png'],
        });
        expect(conditionToJson({ type: 'nameContains', words: [' factura ', ''] })).toEqual({
            type: 'nameContains',
            value: 'factura',
        });
        expect(conditionToJson({ type: 'pathContains', text: ' work ' })).toEqual({
            type: 'pathContains',
            value: 'work',
        });
    });

    it('returns null for values the editor cannot show', () => {
        expect(ruleToDraft(rule([{ type: 'unknown', value: 1 }], [{ type: 'report' }]))).toBeNull();
        expect(ruleToDraft(rule([{ type: 'sizeGt', value: '10' }], [{ type: 'report' }]))).toBeNull();
        expect(ruleToDraft(rule([{ type: 'typeGroup', value: 'other' }], [{ type: 'report' }]))).toBeNull();
        expect(ruleToDraft(rule([{ type: 'ext', value: 'pdf' }], [{ type: 'rename' }]))).toBeNull();
        expect(ruleToDraft(rule([{ type: 'ext', value: 'pdf' }], []))).toBeNull();
    });
});

describe('sizes', () => {
    it('converts amounts with units to bytes', () => {
        expect(sizeToBytes('1', 'KB')).toBe(1024);
        expect(sizeToBytes('1,5', 'MB')).toBe(1.5 * 1024 ** 2);
        expect(sizeToBytes('1.5', 'GB')).toBe(1.5 * 1024 ** 3);
        expect(sizeToBytes(' 2 ', 'GB')).toBe(2 * 1024 ** 3);
        expect(sizeToBytes('0', 'MB')).toBe(0);
        for (const bad of ['', '-1', 'abc', '1e3', '1,2,3']) {
            expect(sizeToBytes(bad, 'MB')).toBeNull();
        }
    });

    it('shows bytes in the largest exact unit', () => {
        expect(bytesToSize(1024 ** 3)).toEqual({ amount: '1', unit: 'GB' });
        expect(bytesToSize(1536 * 1024)).toEqual({ amount: '1536', unit: 'KB' });
        expect(bytesToSize(3 * 1024 ** 2)).toEqual({ amount: '3', unit: 'MB' });
        expect(bytesToSize(0)).toEqual({ amount: '0', unit: 'KB' });
    });
});

describe('draft validation', () => {
    function withCondition(c: ConditionDraft): RuleDraft {
        return { ...newDraft(ID), name: 'x', conditions: [c], action: { type: 'report' } };
    }

    it('reports each invalid value on its row', () => {
        const cases: [ConditionDraft, string][] = [
            [{ type: 'ext', exts: [] }, 'extOne'],
            [{ type: 'extIn', exts: [] }, 'extEmpty'],
            [{ type: 'extNotIn', exts: ['.'] }, 'extEmpty'],
            [{ type: 'nameContains', words: ['  '] }, 'textEmpty'],
            [{ type: 'nameMatches', text: '' }, 'regexEmpty'],
            [{ type: 'pathContains', text: ' ' }, 'textEmpty'],
            [{ type: 'sizeGt', amount: 'mult', unit: 'MB' }, 'size'],
            [{ type: 'sizeLt', amount: '', unit: 'KB' }, 'size'],
            [{ type: 'modifiedOlderThanDays', days: '1.5' }, 'days'],
            [{ type: 'createdOlderThanDays', days: '36501' }, 'days'],
        ];
        for (const [c, key] of cases) {
            const r = draftToRule(withCondition(c));
            expect(r.ok, JSON.stringify(c)).toBe(false);
            expect(!r.ok && r.errors.conditions).toEqual({ 0: key });
        }
    });

    it('needs a name, a destination and at least one condition', () => {
        const d: RuleDraft = { ...newDraft(ID), conditions: [], action: { type: 'moveTo', path: ' ' } };
        const r = draftToRule(d);
        expect(r.ok).toBe(false);
        expect(!r.ok && r.errors).toEqual({
            name: 'nameEmpty',
            action: 'destinationEmpty',
            general: 'noConditions',
            conditions: {},
        });
        // alwaysApply needs no condition and ignores the hidden rows.
        const all = draftToRule({
            ...d,
            name: 'Tot',
            alwaysApply: true,
            conditions: [{ type: 'extIn', exts: [] }],
            action: { type: 'trash' },
        });
        expect(all.ok && all.rule.conditions).toEqual([]);
    });

    it('new drafts start enabled with an extension row and a move', () => {
        const d = newDraft(ID);
        expect(d).toMatchObject({ id: ID, enabled: true, version: 1, scope: [] });
        expect(d.conditions).toEqual([{ type: 'extIn', exts: [] }]);
        expect(d.action).toEqual({ type: 'moveTo', path: '' });
    });

    it('keeps compatible values when the condition type changes', () => {
        expect(defaultCondition('ext', { type: 'extIn', exts: ['jpg', 'png'] })).toEqual({ type: 'ext', exts: ['jpg'] });
        expect(defaultCondition('sizeLt', { type: 'sizeGt', amount: '2', unit: 'GB' })).toEqual({
            type: 'sizeLt',
            amount: '2',
            unit: 'GB',
        });
        expect(defaultCondition('typeGroup', { type: 'nameMatches', text: 'x' })).toEqual({
            type: 'typeGroup',
            group: 'document',
        });
        expect(defaultCondition('createdOlderThanDays', { type: 'modifiedOlderThanDays', days: '9' })).toEqual({
            type: 'createdOlderThanDays',
            days: '9',
        });
    });
});
