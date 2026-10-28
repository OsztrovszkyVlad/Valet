import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import RulesView from './RulesView';
import type { Preset, Rule } from '../api/commands';

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
/** Error codes `upsert_rule` throws next, in order. */
let upsertErrors: string[];

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
                if (err) throw err;
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
