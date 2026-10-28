// Keys built at run time (t(`conditionLabel.${type}`)) are not seen by
// scripts/check-i18n.mjs; this checks them, and that every error code the
// Rust commands return has a translation.
import { describe, expect, it } from 'vitest';
import i18n from '.';
import ro from './ro.json';
import en from './en.json';
import { ACTION_TYPES, CONDITION_TYPES, TYPE_GROUPS } from '../api/commands';
import { FOLDER_TOKENS } from '../rules/summary';
import { commandErrorText } from './format';

type Tree = { [k: string]: string | Tree };

function has(tree: Tree, key: string): boolean {
    let cur: string | Tree | undefined = tree;
    for (const part of key.split('.')) {
        if (typeof cur !== 'object' || cur === null) return false;
        cur = cur[part];
    }
    return typeof cur === 'string' && cur.trim() !== '';
}

function missing(keys: string[]): string[] {
    const out: string[] = [];
    for (const [name, tree] of [
        ['ro', ro],
        ['en', en],
    ] as const) {
        for (const k of keys) if (!has(tree as Tree, k)) out.push(`${name}: ${k}`);
    }
    return out;
}

/** Rust command sources, read at test time; modules that land later are picked up. */
const RUST_SOURCES = import.meta.glob('../../src-tauri/src/commands/*.rs', {
    query: '?raw',
    import: 'default',
    eager: true,
}) as Record<string, string>;

/** `"area.code"` values of the `ERR_*` constants in the Rust command modules. */
function rustCodes(prefixes: string[]): string[] {
    const codes = new Set<string>();
    for (const text of Object.values(RUST_SOURCES)) {
        for (const m of text.matchAll(/const ERR_[A-Z_]+: &str = "([a-z]+\.[A-Za-z]+)"/g)) {
            if (prefixes.some((p) => m[1].startsWith(p))) codes.add(m[1]);
        }
    }
    return [...codes].sort();
}

const PLANNER = import.meta.glob('../../../../crates/valet-core/src/planner.rs', {
    query: '?raw',
    import: 'default',
    eager: true,
}) as Record<string, string>;

/** Variants of `valet_core::planner::SkipReason`, camelCase as serialized. */
function skipReasons(): string[] {
    const text = Object.values(PLANNER)[0] ?? '';
    const body = /pub enum SkipReason \{([\s\S]*?)\n\}/.exec(text)?.[1] ?? '';
    return [...body.matchAll(/^\s{4}([A-Z][A-Za-z]+),/gm)].map((m) => m[1][0].toLowerCase() + m[1].slice(1));
}

describe('run-time i18n keys', () => {
    it('explain every skip reason the planner returns', () => {
        const reasons = skipReasons();
        expect(reasons).toContain('excluded');
        expect(reasons).toContain('ruleDestination');
        expect(missing(reasons.map((r) => `skipReason.${r}`))).toEqual([]);
    });

    it('exist in ro.json and en.json', () => {
        const keys = [
            ...CONDITION_TYPES.map((c) => `conditionLabel.${c}`),
            ...ACTION_TYPES.map((a) => `actionLabel.${a}`),
            ...TYPE_GROUPS.flatMap((g) => [`typeGroup.${g}`, `typeGroupPlural.${g}`]),
            ...FOLDER_TOKENS.map((f) => `folder.${f}`),
            ...['downloads', 'documents', 'pictures', 'desktop', 'home', 'year', 'month', 'day', 'ext'].map(
                (tok) => `tokenHelp.${tok}`,
            ),
            'actionHelp.trash',
            'actionHelp.report',
            'conditionHelp.emptyFolder',
            'conditionHelp.duplicate',
            ...[
                'nameEmpty',
                'extOne',
                'extEmpty',
                'textEmpty',
                'regexEmpty',
                'size',
                'days',
                'destinationEmpty',
                'noConditions',
            ].map((e) => `editorError.${e}`),
            ...['move', 'copy', 'trash'].map((o) => `opType.${o}`),
        ];
        expect(missing(keys)).toEqual([]);
    });

    it('cover every rule.*, config.*, settings.* and preset.* code in the Rust commands', () => {
        const codes = rustCodes(['rule.', 'config.', 'settings.', 'preset.']);
        expect(codes).toContain('rule.invalidRegex');
        expect(codes).toContain('config.driveRoot');
        expect(codes.length).toBeGreaterThanOrEqual(22);
        expect(missing(codes.map((c) => `backendError.${c}`))).toEqual([]);
    });
});

describe('error translation', () => {
    it('translates rule codes in both languages', async () => {
        const t = i18n.t.bind(i18n);
        expect(commandErrorText(t, 'rule.emptyFolderAction')).toBe(
            'Folderele goale pot merge doar la Coș sau în raport; nu se mută și nu se copiază.',
        );
        expect(commandErrorText(t, 'rule.notFound')).toBe('Regula nu mai există.');
        await i18n.changeLanguage('en');
        expect(commandErrorText(t, 'rule.invalidScope')).toBe('One of the folders is not valid.');
        expect(commandErrorText(t, 'rule.somethingNew')).toBe('Error: rule.somethingNew');
    });
});
