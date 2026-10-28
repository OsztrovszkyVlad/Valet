import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import DryRunView from './DryRunView';
import type { DryRunResponse } from '../api/commands';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
const invokeMock = vi.mocked(invoke);

const GB = 1024 ** 3;

function preview(): DryRunResponse {
    return {
        results: [
            {
                source_path: 'C:\\D\\setup.exe',
                destination_path: '',
                rule_name: 'Instalatoare vechi',
                operation: 'trash',
            },
            {
                source_path: 'C:\\D\\a.pdf',
                destination_path: 'C:\\Docs\\a.pdf',
                rule_name: 'Facturi',
                operation: 'move',
            },
            {
                source_path: 'C:\\D\\poza (1).jpg',
                destination_path: '',
                rule_name: 'Duplicate la Coș',
                operation: 'trash',
            },
        ],
        total_files: 3,
        total_rules_applied: 3,
        skipped: [],
        reports: [
            { path: 'C:\\D\\film.mkv', rule_id: 'r1', rule_name: 'Fișiere mari', size: 2.5 * GB, group: null },
            { path: 'C:\\D\\win.iso', rule_id: 'r1', rule_name: 'Fișiere mari', size: 5 * GB, group: null },
            { path: 'C:\\D\\poza.jpg', rule_id: 'r2', rule_name: 'Duplicate la Coș', size: 2048, group: 'aa11' },
            { path: 'C:\\D\\x.txt', rule_id: 'r3', rule_name: 'Duplicate', size: 10, group: 'bb22' },
            { path: 'C:\\D\\y.txt', rule_id: 'r3', rule_name: 'Duplicate', size: 10, group: 'bb22' },
        ],
        plan_id: 'plan-1',
    };
}

let response: DryRunResponse | string;

beforeEach(() => {
    response = preview();
    invokeMock.mockReset();
    invokeMock.mockImplementation(async (cmd) => {
        switch (cmd) {
            case 'dry_run':
                if (typeof response === 'string') throw response;
                return response;
            case 'close_preview':
                return undefined;
            default:
                throw new Error(`unexpected command ${cmd}`);
        }
    });
});

describe('DryRunView', () => {
    it('labels trash operations and renders reports by rule and duplicate group', async () => {
        render(<DryRunView onBatchRan={vi.fn()} />);
        fireEvent.click(screen.getByRole('button', { name: 'Rulează previzualizarea' }));
        await screen.findByText('Ce se va face');

        const rows = screen.getAllByRole('row');
        const trashRow = rows.find((r) => within(r).queryByText('setup.exe')) as HTMLElement;
        expect(within(trashRow).getByText('La Coș')).toBeTruthy();
        const moveRow = rows.find((r) => within(r).queryByText('a.pdf')) as HTMLElement;
        expect(within(moveRow).queryByText('La Coș')).toBeNull();
        expect(within(moveRow).getByTitle('C:\\Docs\\a.pdf').textContent).toBe('C:\\Docs\\a.pdf');
        expect(moveRow.dataset.testid).toBe('preview-row');
        expect(moveRow.dataset.source).toBe('C:\\D\\a.pdf');
        expect(moveRow.dataset.dest).toBe('C:\\Docs\\a.pdf');

        const reports = screen.getByRole('region', { name: 'Rapoarte' });
        expect(within(reports).getByRole('heading', { name: 'Fișiere mari' })).toBeTruthy();
        expect(within(reports).getByText('2,5 GB')).toBeTruthy();
        expect(within(reports).getByText('5 GB')).toBeTruthy();
        expect(within(reports).getByRole('heading', { name: 'Duplicate' })).toBeTruthy();
        expect(within(reports).getByRole('heading', { name: 'Grupul 1, 2 KB fiecare' })).toBeTruthy();
        expect(within(reports).getByRole('heading', { name: 'Grupul 2, 10 B fiecare' })).toBeTruthy();

        // The copy the trash rule keeps is marked; report-only groups have no kept copy.
        const kept = within(reports).getAllByText('se păstrează');
        expect(kept).toHaveLength(1);
        expect(kept[0].closest('li')?.textContent).toContain('C:\\D\\poza.jpg');
        expect(within(reports).getByText('Celelalte copii sunt mai sus, cu destinația „La Coș”.')).toBeTruthy();

        // Reports are not operations.
        expect(screen.getByRole('button', { name: 'Organizează fișierele (3)' })).toBeTruthy();
    });

    it('lists as skipped only files without an operation and counts the ignored rules', async () => {
        response = {
            ...preview(),
            skipped: [
                // A later rule for a file that is already planned: not a skipped file.
                { path: 'C:\\D\\a.pdf', reason: 'conflict', rule_name: 'Documente' },
                { path: 'C:\\D\\setup.exe', reason: 'conflict', rule_name: 'Instalatoare' },
                { path: 'C:\\D\\note.txt', reason: 'beingWritten', rule_name: null },
            ],
        };
        render(<DryRunView onBatchRan={vi.fn()} />);
        fireEvent.click(screen.getByRole('button', { name: 'Rulează previzualizarea' }));
        await screen.findByText('Ce se va face');

        const skippedCount = screen.getByText('Fișiere sărite').parentElement as HTMLElement;
        expect(within(skippedCount).getByText('1')).toBeTruthy();
        expect(screen.getByText('Reguli ignorate pentru fișiere deja planificate: 2')).toBeTruthy();
        expect(screen.getByRole('heading', { name: 'Fișiere sărite: 1' })).toBeTruthy();
        fireEvent.click(screen.getByRole('button', { name: 'Arată' }));
        const rows = screen.getAllByTestId('skipped-row');
        expect(rows).toHaveLength(1);
        expect(rows[0].dataset.reason).toBe('beingWritten');
        expect(rows[0].textContent).toContain('C:\\D\\note.txt');
    });

    it('drops the stored preview when the user clears it without executing', async () => {
        const onPlanChange = vi.fn();
        render(<DryRunView onBatchRan={vi.fn()} onPlanChange={onPlanChange} />);
        fireEvent.click(screen.getByRole('button', { name: 'Rulează previzualizarea' }));
        await screen.findByText('Ce se va face');
        await waitFor(() => expect(onPlanChange).toHaveBeenLastCalledWith('plan-1'));
        fireEvent.click(screen.getByRole('button', { name: 'Golește rezultatul' }));
        expect(invokeMock).toHaveBeenCalledWith('close_preview');
        await waitFor(() => expect(onPlanChange).toHaveBeenLastCalledWith(null));
    });

    it('shows the onboarding preview with its explanation', () => {
        render(<DryRunView onBatchRan={vi.fn()} initial={{ preview: preview() }} intro />);
        expect(screen.getByText('Pasul 3 din 3: previzualizare')).toBeTruthy();
        expect(
            screen.getByText('Aceasta e lista cu ce ar face Valet. Nimic nu se mută până nu apeși „Organizează”.'),
        ).toBeTruthy();
        expect(screen.getByRole('button', { name: 'Organizează fișierele (3)' })).toBeTruthy();
        expect(invokeMock).not.toHaveBeenCalled();
    });

    it('has empty states for no folders and for nothing to do', async () => {
        const openSettings = vi.fn();
        render(<DryRunView onBatchRan={vi.fn()} initial={{ error: 'config.noFolders' }} onOpenSettings={openSettings} />);
        expect(screen.getByRole('heading', { name: 'Niciun folder urmărit' })).toBeTruthy();
        fireEvent.click(screen.getByRole('button', { name: 'Deschide Setările' }));
        expect(openSettings).toHaveBeenCalled();

        response = { ...preview(), results: [], reports: [], plan_id: 'plan-2' };
        fireEvent.click(screen.getByRole('button', { name: 'Rulează previzualizarea' }));
        expect(await screen.findByRole('heading', { name: 'Nimic de organizat' })).toBeTruthy();
        expect(screen.queryByRole('heading', { name: 'Niciun folder urmărit' })).toBeNull();
        expect(screen.queryByRole('region', { name: 'Rapoarte' })).toBeNull();
    });

    it('translates dry_run error codes', async () => {
        response = 'rule.storage';
        render(<DryRunView onBatchRan={vi.fn()} />);
        fireEvent.click(screen.getByRole('button', { name: 'Rulează previzualizarea' }));
        expect(
            await screen.findByText('Previzualizarea nu a putut rula: Regulile nu au putut fi citite sau salvate.'),
        ).toBeTruthy();
    });
});
