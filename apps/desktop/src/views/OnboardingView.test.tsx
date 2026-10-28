import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import OnboardingView from './OnboardingView';
import type { DryRunResponse, Preset } from '../api/commands';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
const invokeMock = vi.mocked(invoke);

const PRESETS: Preset[] = [
    {
        id: 'old-installers',
        name: 'Instalatoare vechi',
        description: '.exe, .msi mai vechi de 30 de zile merg în Coș.',
        displayOrder: 3,
        defaultChecked: false,
        defaultScope: ['{downloads}'],
        ruleCount: 1,
        actions: ['trash'],
    },
    {
        id: 'downloads-by-type',
        name: 'Descărcări după tip',
        description: 'Imagini, documente, arhive în subfoldere.',
        displayOrder: 1,
        defaultChecked: true,
        defaultScope: ['{downloads}'],
        ruleCount: 6,
        actions: ['moveTo'],
    },
    {
        id: 'big-files',
        name: 'Fișiere mari',
        description: 'Fișiere peste 1 GB, doar în listă.',
        displayOrder: 2,
        defaultChecked: true,
        defaultScope: ['{downloads}', '{documents}'],
        ruleCount: 1,
        actions: ['report'],
    },
];

const PREVIEW: DryRunResponse = {
    results: [],
    total_files: 0,
    total_rules_applied: 0,
    skipped: [],
    reports: [],
    plan_id: 'p-1',
};

let completeError: string | null;

beforeEach(() => {
    completeError = null;
    invokeMock.mockReset();
    invokeMock.mockImplementation(async (cmd) => {
        switch (cmd) {
            case 'list_presets':
                return PRESETS;
            case 'complete_onboarding':
                if (completeError) {
                    const e = completeError;
                    completeError = null;
                    throw e;
                }
                return { config: { inbox_paths: [] }, rules: [] };
            case 'dry_run':
                return PREVIEW;
            default:
                throw new Error(`unexpected command ${cmd}`);
        }
    });
});

function checkbox(name: string) {
    return screen.getByRole('checkbox', { name }) as HTMLInputElement;
}

function calls(cmd: string) {
    return invokeMock.mock.calls.filter(([c]) => c === cmd);
}

describe('OnboardingView', () => {
    it('step 1 pre-checks Downloads and Desktop and offers no skip', () => {
        render(<OnboardingView onFinished={vi.fn()} />);
        expect(screen.getByText('Pasul 1 din 3')).toBeTruthy();
        expect(checkbox('Descărcări').checked).toBe(true);
        expect(checkbox('Desktop').checked).toBe(true);
        expect(checkbox('Documente').checked).toBe(false);
        expect(checkbox('Imagini').checked).toBe(false);
        for (const [id, name] of [
            ['downloads', 'Descărcări'],
            ['desktop', 'Desktop'],
            ['documents', 'Documente'],
            ['pictures', 'Imagini'],
        ]) {
            expect(screen.getByTestId(`folder-${id}`)).toBe(checkbox(name));
        }
        expect(screen.queryByRole('button', { name: 'Sari peste' })).toBeNull();

        fireEvent.click(checkbox('Descărcări'));
        fireEvent.click(checkbox('Desktop'));
        expect(screen.getByText('Bifează cel puțin un folder.')).toBeTruthy();
        expect((screen.getByRole('button', { name: 'Continuă' }) as HTMLButtonElement).disabled).toBe(true);
    });

    it('runs folders → presets → complete_onboarding → dry_run', async () => {
        const onFinished = vi.fn();
        render(<OnboardingView onFinished={onFinished} />);

        fireEvent.click(checkbox('Imagini'));
        fireEvent.change(screen.getByLabelText('Adaugă alt folder'), { target: { value: 'D:\\Proiecte' } });
        fireEvent.click(screen.getByRole('button', { name: 'Adaugă' }));
        expect(checkbox('D:\\Proiecte').checked).toBe(true);
        fireEvent.click(screen.getByRole('button', { name: 'Continuă' }));

        expect(await screen.findByText('Pasul 2 din 3')).toBeTruthy();
        const items = await screen.findAllByRole('listitem');
        // In displayOrder, checked by defaultChecked, with badges.
        expect(items.map((li) => within(li).getByRole('checkbox').closest('label')?.textContent)).toEqual([
            'Descărcări după tip',
            'Fișiere mari',
            'Instalatoare vechi',
        ]);
        expect(within(items[0]).queryByText('trimite la Coș')).toBeNull();
        expect(within(items[0]).queryByText('doar raport')).toBeNull();
        expect(within(items[1]).getByText('doar raport')).toBeTruthy();
        expect(within(items[2]).getByText('trimite la Coș')).toBeTruthy();
        expect(checkbox('Descărcări după tip').checked).toBe(true);
        expect(checkbox('Fișiere mari').checked).toBe(true);
        expect(checkbox('Instalatoare vechi').checked).toBe(false);
        expect(screen.getByText('Imagini, documente, arhive în subfoldere.')).toBeTruthy();
        expect(screen.getByText('6 reguli')).toBeTruthy();
        expect(screen.getAllByText('o regulă')).toHaveLength(2);

        fireEvent.click(checkbox('Instalatoare vechi'));
        fireEvent.click(checkbox('Fișiere mari'));
        fireEvent.click(screen.getByRole('button', { name: 'Continuă' }));

        await waitFor(() => expect(onFinished).toHaveBeenCalledWith({ preview: PREVIEW }));
        expect(calls('complete_onboarding')).toEqual([
            [
                'complete_onboarding',
                {
                    folders: ['{downloads}', '{desktop}', '{pictures}', 'D:\\Proiecte'],
                    presetIds: ['downloads-by-type', 'old-installers'],
                },
            ],
        ]);
        expect(calls('dry_run')).toHaveLength(1);
    });

    it('skip on step 2 installs no presets', async () => {
        const onFinished = vi.fn();
        render(<OnboardingView onFinished={onFinished} />);
        fireEvent.click(screen.getByRole('button', { name: 'Continuă' }));
        fireEvent.click(await screen.findByRole('button', { name: 'Sari peste' }));
        await waitFor(() => expect(onFinished).toHaveBeenCalled());
        expect(calls('complete_onboarding')[0][1]).toEqual({
            folders: ['{downloads}', '{desktop}'],
            presetIds: [],
        });
    });

    it('goes back to step 1 with a translated config error', async () => {
        const onFinished = vi.fn();
        completeError = 'config.excludedFolder: C:\\Windows';
        render(<OnboardingView onFinished={onFinished} />);
        fireEvent.click(screen.getByRole('button', { name: 'Continuă' }));
        await screen.findByText('Descărcări după tip');
        fireEvent.click(screen.getByRole('button', { name: 'Continuă' }));

        expect(
            await screen.findByText(
                'Folderul e exclus implicit (folder de sistem, AppData, Program Files sau ascuns). Alege alt folder. (C:\\Windows)',
            ),
        ).toBeTruthy();
        expect(screen.getByText('Pasul 1 din 3')).toBeTruthy();
        expect(onFinished).not.toHaveBeenCalled();
        expect(calls('dry_run')).toHaveLength(0);
    });

    it('names a missing folder token in the error', async () => {
        completeError = 'config.missing: {pictures}';
        render(<OnboardingView onFinished={vi.fn()} />);
        fireEvent.click(checkbox('Imagini'));
        fireEvent.click(screen.getByRole('button', { name: 'Continuă' }));
        fireEvent.click(await screen.findByRole('button', { name: 'Sari peste' }));
        expect(await screen.findByText('Folderul nu există. (Imagini)')).toBeTruthy();
        expect(screen.getByText('Pasul 1 din 3')).toBeTruthy();
    });

    it('shows preset errors on step 2', async () => {
        completeError = 'preset.unknown: nope';
        render(<OnboardingView onFinished={vi.fn()} />);
        fireEvent.click(screen.getByRole('button', { name: 'Continuă' }));
        await screen.findByText('Descărcări după tip');
        fireEvent.click(screen.getByRole('button', { name: 'Continuă' }));
        expect(await screen.findByText('Presetare necunoscută. (nope)')).toBeTruthy();
        expect(screen.getByText('Pasul 2 din 3')).toBeTruthy();
    });

    it('passes a dry_run failure on to the Preview', async () => {
        const onFinished = vi.fn();
        render(<OnboardingView onFinished={onFinished} />);
        invokeMock.mockImplementation(async (cmd) => {
            if (cmd === 'list_presets') return PRESETS;
            if (cmd === 'complete_onboarding') return { config: { inbox_paths: [] }, rules: [] };
            if (cmd === 'dry_run') throw 'config.noFolders';
            throw new Error(`unexpected command ${cmd}`);
        });
        fireEvent.click(screen.getByRole('button', { name: 'Continuă' }));
        fireEvent.click(await screen.findByRole('button', { name: 'Sari peste' }));
        await waitFor(() => expect(onFinished).toHaveBeenCalledWith({ error: 'config.noFolders' }));
    });

    it('shows a preset list error with retry and still allows skip', async () => {
        let fail = true;
        invokeMock.mockImplementation(async (cmd) => {
            if (cmd === 'list_presets') {
                if (fail) throw 'preset.storage';
                return PRESETS;
            }
            throw new Error(`unexpected command ${cmd}`);
        });
        render(<OnboardingView onFinished={vi.fn()} />);
        fireEvent.click(screen.getByRole('button', { name: 'Continuă' }));
        expect(await screen.findByText(/Lista de reguli nu a putut fi citită/)).toBeTruthy();
        expect((screen.getByRole('button', { name: 'Continuă' }) as HTMLButtonElement).disabled).toBe(true);
        expect((screen.getByRole('button', { name: 'Sari peste' }) as HTMLButtonElement).disabled).toBe(false);
        fail = false;
        fireEvent.click(screen.getByRole('button', { name: 'Încearcă din nou' }));
        expect(await screen.findByText('Descărcări după tip')).toBeTruthy();
    });
});
