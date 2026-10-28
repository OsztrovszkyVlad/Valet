import { describe, expect, it } from 'vitest';
import i18n from '../i18n';
import type { Rule, RuleAction, RuleCondition } from '../api/commands';
import { actionSummary, conditionSummary, displayPath, ruleSummary } from './summary';

const t = i18n.t.bind(i18n);

function rule(conditions: RuleCondition[], actions: RuleAction[], extra: Partial<Rule> = {}): Rule {
    return {
        id: '00000000-0000-0000-0000-000000000001',
        name: 'r',
        enabled: true,
        alwaysApply: false,
        conditions,
        actions,
        version: 1,
        options: {},
        scope: [],
        ...extra,
    };
}

describe('conditionSummary (ro)', () => {
    const cases: [RuleCondition, string][] = [
        [{ type: 'ext', value: 'pdf' }, 'PDF'],
        [{ type: 'extIn', value: ['jpg', '.png'] }, 'JPG, PNG'],
        [{ type: 'extNotIn', value: ['exe'] }, 'în afară de EXE'],
        [{ type: 'typeGroup', value: 'installer' }, 'instalatoare'],
        [{ type: 'nameContains', value: 'factura' }, 'cu «factura» în nume'],
        [{ type: 'nameContains', value: ['invoice', 'bon'] }, 'cu «invoice» sau «bon» în nume'],
        [{ type: 'nameMatches', value: '^scan_\\d+' }, 'nume potrivit cu ^scan_\\d+'],
        [{ type: 'pathContains', value: 'work' }, 'cu «work» în cale'],
        [
            { type: 'nameMatches', value: '(?i)(^|[^\\p{L}])(screenshot|captur[aă]|snip|clipboard)$' },
            'nume potrivit cu (?i)(^|[^\\p{L}])(screenshot|captur[aă]|s…',
        ],
        [{ type: 'sizeGt', value: 1024 ** 3 }, 'peste 1 GB'],
        [{ type: 'sizeLt', value: 1536 * 1024 }, 'sub 1,5 MB'],
        [{ type: 'modifiedOlderThanDays', value: 1 }, 'modificate de peste o zi'],
        [{ type: 'modifiedOlderThanDays', value: 7 }, 'modificate de peste 7 zile'],
        [{ type: 'modifiedOlderThanDays', value: 30 }, 'modificate de peste 30 de zile'],
        [{ type: 'createdOlderThanDays', value: 14 }, 'create de peste 14 zile'],
        [{ type: 'emptyFolder', value: true }, 'foldere goale'],
        [{ type: 'duplicate', value: true }, 'duplicate'],
    ];
    it.each(cases)('%j', (c, text) => {
        expect(conditionSummary(t, c, 'ro')).toBe(text);
    });
});

describe('actionSummary (ro)', () => {
    it('describes each action', () => {
        expect(actionSummary(t, { type: 'moveTo', params: { path: '{documents}/Facturi/{year}' } })).toBe(
            'Documente/Facturi/{year}',
        );
        expect(actionSummary(t, { type: 'copyTo', params: { path: 'D:\\Backup' } })).toBe('copie în D:\\Backup');
        expect(actionSummary(t, { type: 'moveTo', params: { path: 'Imagini' } })).toBe('subfolderul Imagini');
        expect(actionSummary(t, { type: 'moveTo', params: { path: '~/Arhiva' } })).toBe('~/Arhiva');
        expect(actionSummary(t, { type: 'moveTo', params: { path: '\\\\nas\\x' } })).toBe('\\\\nas\\x');
        expect(actionSummary(t, { type: 'trash' })).toBe('Coșul de reciclare');
        expect(actionSummary(t, { type: 'report', params: {} })).toBe('doar raport');
    });
});

describe('ruleSummary', () => {
    it('matches the PLAN example', () => {
        const r = rule(
            [
                { type: 'ext', value: 'pdf' },
                { type: 'nameContains', value: 'factura' },
            ],
            [{ type: 'moveTo', params: { path: '{documents}/Facturi/{year}' } }],
        );
        expect(ruleSummary(t, r, 'ro')).toBe('PDF, cu «factura» în nume → Documente/Facturi/{year}');
    });

    it('starts with a capital letter and lists every action', () => {
        const r = rule([{ type: 'duplicate', value: true }], [{ type: 'report' }, { type: 'trash' }]);
        expect(ruleSummary(t, r, 'ro')).toBe('Duplicate → doar raport, Coșul de reciclare');
    });

    it('says all files for alwaysApply', () => {
        const r = rule([], [{ type: 'moveTo', params: { path: 'Arhiva' } }], { alwaysApply: true });
        expect(ruleSummary(t, r, 'ro')).toBe('Toate fișierele → subfolderul Arhiva');
    });

    it('follows the UI language', async () => {
        await i18n.changeLanguage('en');
        const r = rule(
            [
                { type: 'typeGroup', value: 'image' },
                { type: 'modifiedOlderThanDays', value: 1 },
            ],
            [{ type: 'moveTo', params: { path: '{pictures}/Old' } }],
        );
        expect(ruleSummary(t, r, 'en')).toBe('Images, modified over 1 day ago → Pictures/Old');
        expect(conditionSummary(t, { type: 'modifiedOlderThanDays', value: 30 }, 'en')).toBe(
            'modified over 30 days ago',
        );
        expect(conditionSummary(t, { type: 'sizeLt', value: 1536 * 1024 }, 'en')).toBe('under 1.5 MB');
    });
});

describe('displayPath', () => {
    it('names folder tokens and leaves other paths alone', () => {
        expect(displayPath(t, '{downloads}')).toBe('Descărcări');
        expect(displayPath(t, '{home}/Proiecte')).toBe('Folderul personal/Proiecte');
        expect(displayPath(t, '{year}/x')).toBe('{year}/x');
        expect(displayPath(t, 'C:\\Users\\ana')).toBe('C:\\Users\\ana');
    });
});
