import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import RulesView from './RulesView';
import type { Preset, Rule } from '../api/commands';
import { freePlan, trialPlan } from '../test/plan';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
const invokeMock = vi.mocked(invoke);

const NEW_ID = '11111111-1111-4111-8111-111111111111';
const DOWNLOADS = 'C:\\Users\\ana\\Downloads';

const PRESETS: Preset[] = [
    {
        id: 'old-installers',
        name: 'Instalatoare vechi',
        description: '.exe, .msi mai vechi de 30 de zile merg în Coș.',
        displayOrder: 2,
        defaultChecked: false,
        defaultScope: ['{downloads}'],
        ruleCount: 1,
        actions: ['trash'],
    },
    {
        id: 'invoices',
        name: 'Facturi',
        description: 'PDF-uri cu «factura» în nume.',
        displayOrder: 1,
        defaultChecked: true,
        defaultScope: ['{downloads}'],
        ruleCount: 1,
        actions: ['moveTo'],
    },
];

let rules: Rule[];
/** Errors `upsert_rule` throws next, in order (code strings or `{ code, ... }` objects). */
let upsertErrors: unknown[];
/** What `import_rules_file` and `export_rules_file` answer. */
let importAnswer: unknown;
let exportAnswer: unknown;

function presetRule(): Rule {
    return {
        id: '22222222-2222-4222-8222-222222222222',
        name: 'Facturi și chitanțe',
        enabled: true,
        alwaysApply: false,
        conditions: [
            { type: 'ext', value: 'pdf' },
            { type: 'nameContains', value: ['factura', 'invoice'] },
        ],
        actions: [{ type: 'moveTo', params: { path: '{documents}/Facturi/{year}' } }],
        version: 1,
        options: { presetId: 'invoices' },
        scope: ['{downloads}'],
    };
}

beforeEach(() => {
    rules = [];
    upsertErrors = [];
    importAnswer = null;
    exportAnswer = null;
    vi.spyOn(crypto, 'randomUUID').mockReturnValue(NEW_ID);
    invokeMock.mockReset();
    invokeMock.mockImplementation(async (cmd, args) => {
        const a = args as Record<string, unknown> | undefined;
        switch (cmd) {
            case 'get_rules':
                return rules;
            case 'get_config':
                return { inbox_paths: [DOWNLOADS] };
            case 'upsert_rule': {
                const err = upsertErrors.shift();
                if (err !== undefined) throw err;
                const rule = { ...(a?.rule as Rule) };
                rule.version += rules.some((r) => r.id === rule.id) ? 1 : 0;
                rules = rules.some((r) => r.id === rule.id)
                    ? rules.map((r) => (r.id === rule.id ? rule : r))
                    : [...rules, rule];
                return rule;
            }
            case 'list_presets':
                return PRESETS;
            case 'install_presets': {
                const ids = a?.presetIds as string[];
                const installed = ids.map((id) => ({
                    ...presetRule(),
                    id: `rule-${id}`,
                    name: `Regulă ${id}`,
                    options: { presetId: id },
                }));
                rules = [...rules.filter((r) => !ids.includes(r.options?.presetId as string)), ...installed];
                return installed;
            }
            case 'delete_rule':
                rules = rules.filter((r) => r.id !== a?.ruleId);
                return undefined;
            case 'import_rules_file':
                if (typeof importAnswer === 'string') throw importAnswer;
                if (Array.isArray(importAnswer)) rules = [...rules, ...(importAnswer as Rule[])];
                return importAnswer;
            case 'export_rules_file':
                if (typeof exportAnswer === 'string' && exportAnswer.startsWith('export.')) throw exportAnswer;
                return exportAnswer;
            default:
                throw new Error(`unexpected command ${cmd}`);
        }
    });
});

function lastUpsert(): Rule {
    const calls = invokeMock.mock.calls.filter(([c]) => c === 'upsert_rule');
    return (calls[calls.length - 1][1] as { rule: Rule }).rule;
}

function type(label: string, value: string) {
    fireEvent.change(screen.getByLabelText(label), { target: { value } });
}

describe('RulesView', () => {
    it('shows the empty state and creates a rule with an inline regex error from upsert_rule', async () => {
        render(<RulesView />);
        expect(await screen.findByText('Nicio regulă încă')).toBeTruthy();
        fireEvent.click(screen.getAllByRole('button', { name: 'Adaugă regulă' })[0]);

        expect(screen.getByRole('heading', { name: 'Regulă nouă' })).toBeTruthy();
        type('Nume', 'Scanări');
        type('Tip condiție', 'nameMatches');
        type('Expresie regulată pentru numele fișierului', '[scan');
        type('Ce face regula', 'moveTo');
        type('Folder destinație', '{documents}/Scanări/');
        fireEvent.click(screen.getByRole('button', { name: 'Adaugă {year}' }));
        fireEvent.click(screen.getByRole('checkbox', { name: DOWNLOADS }));
        fireEvent.click(screen.getByRole('checkbox', { name: 'Doar fișierele direct în folder, fără subfoldere' }));

        upsertErrors = ['rule.invalidRegex'];
        fireEvent.click(screen.getByRole('button', { name: 'Salvează regula' }));
        expect(await screen.findByText('Expresia regulată nu e validă.')).toBeTruthy();
        expect(
            screen.getByLabelText('Expresie regulată pentru numele fișierului').getAttribute('aria-invalid'),
        ).toBe('true');
        expect(lastUpsert()).toEqual({
            id: NEW_ID,
            name: 'Scanări',
            enabled: true,
            alwaysApply: false,
            conditions: [{ type: 'nameMatches', value: '[scan' }],
            actions: [{ type: 'moveTo', params: { path: '{documents}/Scanări/{year}' } }],
            version: 1,
            options: { topLevelOnly: true },
            scope: [DOWNLOADS],
        });

        type('Expresie regulată pentru numele fișierului', '^scan');
        expect(screen.queryByText('Expresia regulată nu e validă.')).toBeNull();
        fireEvent.click(screen.getByRole('button', { name: 'Salvează regula' }));

        expect(
            await screen.findByText('Regula nouă se aplică automat doar după o previzualizare aprobată.'),
        ).toBeTruthy();
        expect(screen.getByText('Nume potrivit cu ^scan → Documente/Scanări/{year}')).toBeTruthy();
        expect(screen.getByText(`În: ${DOWNLOADS}`)).toBeTruthy();
    });

    it('checks fields before calling upsert_rule', async () => {
        render(<RulesView />);
        fireEvent.click((await screen.findAllByRole('button', { name: 'Adaugă regulă' }))[0]);
        type('Folder destinație', '');
        fireEvent.click(screen.getByRole('button', { name: 'Salvează regula' }));
        expect(screen.getByText('Scrie un nume.')).toBeTruthy();
        expect(screen.getByText('Adaugă cel puțin o extensie.')).toBeTruthy();
        expect(screen.getByText('Scrie folderul destinație.')).toBeTruthy();
        expect(screen.getByText('Corectează câmpurile marcate.')).toBeTruthy();
        expect(invokeMock.mock.calls.some(([c]) => c === 'upsert_rule')).toBe(false);
    });

    it('serializes extension chips, sizes and the trash action', async () => {
        render(<RulesView />);
        fireEvent.click((await screen.findAllByRole('button', { name: 'Adaugă regulă' }))[0]);
        type('Nume', 'Instalatoare mari');
        const ext = screen.getByLabelText('Extensie');
        fireEvent.change(ext, { target: { value: '.EXE' } });
        fireEvent.keyDown(ext, { key: 'Enter' });
        fireEvent.change(screen.getByLabelText('Extensie'), { target: { value: 'msi' } });
        fireEvent.blur(screen.getByLabelText('Extensie'));
        expect(screen.getByRole('button', { name: 'Elimină exe' })).toBeTruthy();

        fireEvent.click(screen.getByRole('button', { name: 'Adaugă condiție' }));
        const types = screen.getAllByLabelText('Tip condiție');
        fireEvent.change(types[1], { target: { value: 'sizeGt' } });
        type('Mărime', '1,5');
        type('Unitate', 'GB');
        type('Ce face regula', 'trash');
        expect(screen.getByText(/Fișierele merg în Coșul de reciclare/)).toBeTruthy();

        fireEvent.click(screen.getByRole('button', { name: 'Salvează regula' }));
        await screen.findByText('Regula nouă se aplică automat doar după o previzualizare aprobată.');
        expect(lastUpsert().conditions).toEqual([
            { type: 'extIn', value: ['exe', 'msi'] },
            { type: 'sizeGt', value: 1.5 * 1024 ** 3 },
        ]);
        expect(lastUpsert().actions).toEqual([{ type: 'trash', params: {} }]);
        expect(screen.getByText('EXE, MSI, peste 1,5 GB → Coșul de reciclare')).toBeTruthy();
    });

    it('translates combination errors from upsert_rule', async () => {
        render(<RulesView />);
        fireEvent.click((await screen.findAllByRole('button', { name: 'Adaugă regulă' }))[0]);
        type('Nume', 'Dubluri');
        type('Tip condiție', 'duplicate');
        type('Folder destinație', 'Dubluri');
        upsertErrors = ['rule.duplicateAction'];
        fireEvent.click(screen.getByRole('button', { name: 'Salvează regula' }));
        expect(
            await screen.findByText('Duplicatele pot merge doar la Coș sau în raport; nu se mută și nu se copiază.'),
        ).toBeTruthy();
    });

    it('lists preset rules with a summary, toggles, edits and deletes them', async () => {
        rules = [presetRule()];
        render(<RulesView />);
        const card = (await screen.findByText('Facturi și chitanțe')).closest('li') as HTMLElement;
        expect(within(card).getByText('presetare')).toBeTruthy();
        expect(within(card).getByText('PDF, cu «factura» sau «invoice» în nume → Documente/Facturi/{year}')).toBeTruthy();
        expect(within(card).getByText('În: Descărcări')).toBeTruthy();

        fireEvent.click(within(card).getByRole('checkbox', { name: 'Regula «Facturi și chitanțe» e activă' }));
        await waitFor(() => expect(lastUpsert().enabled).toBe(false));
        await waitFor(() => expect(card.className).toContain('rule-off'));

        // Edit keeps the preset id and shows the token scope entry.
        fireEvent.click(within(card).getByRole('button', { name: 'Editează' }));
        expect((screen.getByLabelText('Nume') as HTMLInputElement).value).toBe('Facturi și chitanțe');
        expect((screen.getByRole('checkbox', { name: 'Descărcări' }) as HTMLInputElement).checked).toBe(true);
        expect((screen.getByRole('checkbox', { name: DOWNLOADS }) as HTMLInputElement).checked).toBe(false);
        // An unchecked token entry stays in the list.
        fireEvent.click(screen.getByRole('checkbox', { name: 'Descărcări' }));
        expect((screen.getByRole('checkbox', { name: 'Descărcări' }) as HTMLInputElement).checked).toBe(false);
        fireEvent.click(screen.getByRole('checkbox', { name: 'Descărcări' }));
        fireEvent.click(screen.getByRole('button', { name: 'Salvează regula' }));
        expect(
            await screen.findByText('Regula modificată se aplică automat doar după o previzualizare aprobată.'),
        ).toBeTruthy();
        expect(lastUpsert().options).toEqual({ presetId: 'invoices' });
        expect(lastUpsert().scope).toEqual(['{downloads}']);

        const card2 = screen.getByText('Facturi și chitanțe').closest('li') as HTMLElement;
        fireEvent.click(within(card2).getByRole('button', { name: 'Șterge' }));
        expect(within(card2).getByText('Ștergi regula «Facturi și chitanțe»? Fișierele nu sunt atinse.')).toBeTruthy();
        fireEvent.click(within(card2).getByRole('button', { name: 'Șterge regula' }));
        expect(await screen.findByText('Regula a fost ștearsă.')).toBeTruthy();
        expect(invokeMock).toHaveBeenCalledWith('delete_rule', { ruleId: presetRule().id });
        expect(screen.getByText('Nicio regulă încă')).toBeTruthy();
    });

    it('lists presets with installed ones marked and installs or reinstalls the selected ones', async () => {
        rules = [presetRule()];
        render(<RulesView />);
        await screen.findByText('Facturi și chitanțe');
        expect(screen.queryByTestId('rules-presets')).toBeNull();
        expect(invokeMock.mock.calls.some(([c]) => c === 'list_presets')).toBe(false);

        fireEvent.click(screen.getByRole('button', { name: 'Adaugă presetări' }));
        const panel = await screen.findByTestId('rules-presets');
        const items = await within(panel).findAllByRole('listitem');
        expect(items.map((li) => within(li).getByRole('checkbox').closest('label')?.textContent)).toEqual([
            'Facturi',
            'Instalatoare vechi',
        ]);
        expect(within(items[0]).getByText('instalată')).toBeTruthy();
        expect(within(items[1]).queryByText('instalată')).toBeNull();
        expect(within(items[1]).getByText('trimite la Coș')).toBeTruthy();
        expect(within(panel).getByText(/readuce regulile la versiunea livrată/)).toBeTruthy();

        const install = within(panel).getByTestId('rules-presets-install') as HTMLButtonElement;
        expect(install.disabled).toBe(true);
        expect(install.textContent).toBe('Instalează (0)');

        fireEvent.click(screen.getByTestId('rules-preset-invoices'));
        fireEvent.click(screen.getByTestId('rules-preset-old-installers'));
        expect(within(panel).getByText('Se reinstalează: Facturi')).toBeTruthy();
        expect(install.textContent).toBe('Instalează (2)');
        // Nothing is installed before the button is pressed.
        expect(invokeMock.mock.calls.some(([c]) => c === 'install_presets')).toBe(false);

        fireEvent.click(install);
        expect(
            await screen.findByText(
                'Presetări instalate: 2. Regulile noi sau schimbate se aplică automat doar după o previzualizare aprobată.',
            ),
        ).toBeTruthy();
        expect(invokeMock).toHaveBeenCalledWith('install_presets', { presetIds: ['invoices', 'old-installers'] });
        expect(screen.queryByTestId('rules-presets')).toBeNull();
        expect(await screen.findByText('Regulă old-installers')).toBeTruthy();
        expect(screen.getByText('Regulă invoices')).toBeTruthy();
        expect(screen.queryByText('Facturi și chitanțe')).toBeNull();
    });

    it('shows install errors and keeps the panel open', async () => {
        render(<RulesView />);
        await screen.findByText('Nicio regulă încă');
        fireEvent.click(screen.getByRole('button', { name: 'Adaugă presetări' }));
        fireEvent.click(await screen.findByTestId('rules-preset-old-installers'));
        const base = invokeMock.getMockImplementation();
        invokeMock.mockImplementation(async (cmd, args) => {
            if (cmd === 'install_presets') throw 'preset.storage';
            return base?.(cmd, args);
        });
        fireEvent.click(screen.getByTestId('rules-presets-install'));
        expect(await screen.findByText('Presetările nu au putut fi instalate.')).toBeTruthy();
        expect(screen.getByTestId('rules-presets')).toBeTruthy();
    });
});

function ownRule(id: string, enabled: boolean, options: Record<string, unknown> | null = null): Rule {
    return { ...presetRule(), id, name: `Proprie ${id}`, enabled, options };
}

describe('RulesView plan limits', () => {
    it('explains a refused toggle on Free and links to the Pro section', async () => {
        rules = [ownRule('o1', true), ownRule('o2', true), ownRule('o3', true), ownRule('o4', false)];
        upsertErrors = ['plan.limit: rules 3'];
        const onOpenPro = vi.fn();
        render(<RulesView plan={freePlan()} onOpenPro={onOpenPro} />);
        fireEvent.click(await screen.findByRole('checkbox', { name: 'Regula «Proprie o4» e activă' }));
        const notice = await screen.findByTestId('plan-limit');
        expect(notice.textContent).toContain(
            'Gratuit permite 3 reguli proprii active. Dezactivează una sau treci la Pro.',
        );
        fireEvent.click(within(notice).getByRole('button', { name: 'Vezi Valet Pro' }));
        expect(onOpenPro).toHaveBeenCalledTimes(1);
        expect((screen.getByRole('checkbox', { name: 'Regula «Proprie o4» e activă' }) as HTMLInputElement).checked).toBe(
            false,
        );
    });

    it('explains a refused save of a new enabled rule (object error)', async () => {
        render(<RulesView plan={freePlan()} onOpenPro={() => {}} />);
        fireEvent.click((await screen.findAllByRole('button', { name: 'Adaugă regulă' }))[0]);
        type('Nume', 'Facturi');
        fireEvent.change(screen.getByLabelText('Extensie'), { target: { value: 'pdf' } });
        fireEvent.blur(screen.getByLabelText('Extensie'));
        type('Folder destinație', 'Facturi');
        upsertErrors = [{ code: 'plan.limit', kind: 'rules', limit: 3 }];
        fireEvent.click(screen.getByRole('button', { name: 'Salvează regula' }));
        expect((await screen.findByTestId('plan-limit')).textContent).toContain(
            'Gratuit permite 3 reguli proprii active.',
        );
        expect(screen.getByRole('button', { name: 'Vezi Valet Pro' })).toBeTruthy();
    });

    it('labels rules the Free plan turned off', async () => {
        rules = [ownRule('o1', false, { planDisabled: true }), ownRule('o2', false)];
        render(<RulesView plan={freePlan()} />);
        const labels = await screen.findAllByTestId('rule-plan-disabled');
        expect(labels).toHaveLength(1);
        expect(labels[0].textContent).toBe('dezactivată de planul Gratuit');
        expect(labels[0].closest('li')?.textContent).toContain('Proprie o1');
    });

    it('disables export and import on Free with a Pro label', async () => {
        render(<RulesView plan={freePlan()} />);
        await screen.findByText('Nicio regulă încă');
        const exp = screen.getByTestId('rules-export') as HTMLButtonElement;
        const imp = screen.getByTestId('rules-import') as HTMLButtonElement;
        expect(exp.disabled).toBe(true);
        expect(imp.disabled).toBe(true);
        expect(exp.textContent).toBe('Exportă regulilePro');
        expect(screen.getByText('Exportul și importul de reguli sunt incluse în Pro și în probă.')).toBeTruthy();
    });

    it('hides export and import while the plan is unknown', async () => {
        render(<RulesView />);
        await screen.findByText('Nicio regulă încă');
        expect(screen.queryByTestId('rules-export')).toBeNull();
    });

    it('imports rules on Trial and says they need a preview', async () => {
        importAnswer = [ownRule('i1', true), ownRule('i2', true)];
        render(<RulesView plan={trialPlan(5)} />);
        await screen.findByText('Nicio regulă încă');
        const imp = screen.getByTestId('rules-import') as HTMLButtonElement;
        expect(imp.disabled).toBe(false);
        expect(imp.textContent).toBe('Importă reguli');
        fireEvent.click(imp);
        expect(
            await screen.findByText(
                '2 reguli importate. Regulile importate se aplică automat doar după o previzualizare aprobată.',
            ),
        ).toBeTruthy();
        expect(await screen.findByText('Proprie i1')).toBeTruthy();
    });

    it('shows import errors and the export result', async () => {
        importAnswer = 'import.unknownFormat';
        exportAnswer = 'C:\\Users\\ana\\reguli.json';
        render(<RulesView plan={trialPlan(5)} />);
        await screen.findByText('Nicio regulă încă');
        fireEvent.click(screen.getByTestId('rules-import'));
        expect(
            await screen.findByText(
                'Fișierul nu e un export de reguli Valet sau e dintr-o versiune necunoscută.',
            ),
        ).toBeTruthy();
        fireEvent.click(screen.getByTestId('rules-export'));
        expect(await screen.findByText('Regulile au fost salvate în C:\\Users\\ana\\reguli.json.')).toBeTruthy();
    });

    it('says nothing when the import dialog is cancelled', async () => {
        render(<RulesView plan={trialPlan(5)} />);
        await screen.findByText('Nicio regulă încă');
        fireEvent.click(screen.getByTestId('rules-import'));
        await waitFor(() => expect((screen.getByTestId('rules-import') as HTMLButtonElement).disabled).toBe(false));
        expect(screen.queryByRole('status')).toBeNull();
        expect(screen.queryByRole('alert')).toBeNull();
    });
});
