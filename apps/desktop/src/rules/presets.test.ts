// Every rule of the shipped presets (crates/valet-core/presets) must open in
// the editor, save back unchanged, and get a summary without raw type names.
import { describe, expect, it } from 'vitest';
import i18n from '../i18n';
import type { Rule, RuleAction } from '../api/commands';
import { draftToRule, ruleToDraft, type RuleDraft } from './model';
import { ruleSummary } from './summary';

interface PresetFile {
    id: string;
    defaultScope: string[];
    rules: {
        idSuffix: string;
        name: { ro: string; en: string };
        conditions: Rule['conditions'];
        actions: { type: string; params?: { path?: string | { ro: string; en: string } } }[];
        options?: Record<string, unknown>;
        scope?: string[];
    }[];
}

const FILES = import.meta.glob('../../../../crates/valet-core/presets/*.json', {
    import: 'default',
    eager: true,
}) as Record<string, PresetFile>;

/** The rules a preset installs in Romanian, as `presets::Preset::rules_for` builds them. */
function rulesFor(p: PresetFile): Rule[] {
    return p.rules.map((r, i) => ({
        id: `00000000-0000-5000-8000-${String(i).padStart(12, '0')}`,
        name: r.name.ro,
        enabled: true,
        alwaysApply: false,
        conditions: r.conditions,
        actions: r.actions.map((a): RuleAction => {
            const path = a.params?.path;
            if (path === undefined) return a;
            return { ...a, params: { path: typeof path === 'string' ? path : path.ro } };
        }),
        version: 1,
        options: r.options ?? {},
        scope: r.scope ?? p.defaultScope,
    }));
}

const t = i18n.t.bind(i18n);
const RAW = /\b(ext|extIn|extNotIn|typeGroup|nameContains|nameMatches|pathContains|sizeGt|sizeLt|moveTo|copyTo)\b/;

describe('shipped presets in the rule editor', () => {
    const presets = Object.values(FILES);

    it('finds the 8 preset files', () => {
        expect(presets).toHaveLength(8);
    });

    for (const p of presets) {
        it(`edits and summarizes ${p.id}`, () => {
            for (const rule of rulesFor(p)) {
                const draft = ruleToDraft(rule);
                expect(draft, `${p.id}: ${rule.name}`).not.toBeNull();
                const back = draftToRule(draft as RuleDraft);
                expect(back.ok, `${p.id}: ${rule.name}`).toBe(true);
                if (back.ok) {
                    // Saving adds empty params to trash/report; nothing else changes.
                    const normalize = (r: Rule) => ({
                        ...r,
                        actions: r.actions.map((a) => ({ ...a, params: a.params ?? {} })),
                    });
                    expect(normalize(back.rule)).toEqual(normalize(rule));
                }
                const summary = ruleSummary(t, rule, 'ro');
                expect(summary, summary).not.toMatch(RAW);
                expect(summary).toContain('→');
            }
        });
    }
});
