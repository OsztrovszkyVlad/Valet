import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import i18n from '../i18n';
import { invoke } from '@tauri-apps/api/core';
import HistoryView from './HistoryView';
import type { BatchDetail, BatchInfo, OpRecord } from '../api/commands';
import { freePlan, trialPlan } from '../test/plan';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
const invokeMock = vi.mocked(invoke);

const undoBatch: BatchInfo = {
    id: 'u1',
    kind: 'undo',
    created_at: '2026-10-01T11:00:00Z',
    finished_at: '2026-10-01T11:00:01Z',
    summary: null,
    counts: { planned: 0, in_progress: 0, done: 1, failed: 1, rolled_back: 0 },
};

function op(id: number, partial: Partial<OpRecord>): OpRecord {
    return {
        id,
        batch_id: 'u1',
        seq: id,
        op_type: 'move',
        source_path: 'C:\\in\\a.exe',
        dest_path: null,
        size: 1,
        hash_before: null,
        hash_after: null,
        state: 'done',
        error: null,
        created_at: '2026-10-01T11:00:00Z',
        finished_at: '2026-10-01T11:00:01Z',
        undo_of: null,
        ...partial,
    };
}

const detail: BatchDetail = {
    batch: undoBatch,
    operations: [
        op(5, { op_type: 'restore', dest_path: 'C:\\in\\a.exe', undo_of: 1 }),
        op(6, {
            op_type: 'restore',
            source_path: 'C:\\in\\b.msi',
            dest_path: 'C:\\in\\b.msi',
            state: 'failed',
            error: 'RestoreOccupied: already exists',
            undo_of: 2,
        }),
        op(7, { op_type: 'trash', source_path: 'C:\\out\\c.pdf' }),
    ],
};

const applyBatch: BatchInfo = {
    id: 'a1',
    kind: 'apply',
    created_at: '2026-10-01T10:00:00Z',
    finished_at: '2026-10-01T10:00:01Z',
    summary: '5 done, 0 failed',
    counts: { planned: 0, in_progress: 0, done: 1, failed: 0, rolled_back: 4 },
};

const recoveryBatch: BatchInfo = {
    id: 'r1',
    kind: 'recovery',
    created_at: '2026-10-01T09:00:00Z',
    finished_at: '2026-10-01T09:00:01Z',
    summary: 'recovery: 1 operation(s) decided (1 done, 0 failed), 0 batch(es) closed',
    counts: { planned: 0, in_progress: 0, done: 0, failed: 0, rolled_back: 0 },
};

const details: Record<string, BatchDetail> = {
    u1: detail,
    a1: { batch: applyBatch, operations: [op(1, { batch_id: 'a1', dest_path: 'C:\\out\\a.exe', state: 'rolled_back' })] },
    r1: { batch: recoveryBatch, operations: [] },
};

beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation(async (cmd, args) => {
        if (cmd === 'list_batches') {
            return { batches: [undoBatch, applyBatch, recoveryBatch], hidden_older: false, latest_apply_batch_id: 'a1' };
        }
        if (cmd === 'get_batch') return details[(args as { batchId: string }).batchId];
        throw new Error(`unexpected command ${cmd}`);
    });
});

afterEach(async () => {
    await i18n.changeLanguage('ro');
});

describe('HistoryView', () => {
    it('labels restore and trash rows and translates RestoreOccupied', async () => {
        render(<HistoryView onBatchUndone={() => {}} />);
        fireEvent.click(await screen.findByText('Anulare'));

        expect(await screen.findAllByText('Restaurat din Coș')).toHaveLength(2);
        expect(screen.getByText('Trimis la Coș')).toBeTruthy();
        expect(
            screen.getByText('Locul inițial e ocupat; elementul a rămas în Coșul de reciclare (already exists)'),
        ).toBeTruthy();
    });

    it('marks rows with their kind', async () => {
        render(<HistoryView onBatchUndone={() => {}} />);
        const rows = await screen.findAllByTestId('history-row');
        expect(rows.map((r) => r.dataset.kind)).toEqual(['undo', 'apply', 'recovery']);
    });

    it('summarizes apply and undo batches from their counts, in the UI language', async () => {
        render(<HistoryView onBatchUndone={() => {}} />);
        fireEvent.click(await screen.findByText('Organizare'));
        expect((await screen.findByTestId('history-summary')).textContent).toBe(
            'Operații făcute: 5. Eșuate: 0. Anulate ulterior: 4.',
        );
        expect(screen.queryByText('5 done, 0 failed')).toBeNull();

        fireEvent.click(screen.getByText('Anulare'));
        expect(await screen.findByText('Operații anulate: 1. Eșuate: 1.')).toBeTruthy();
    });

    it('shows the backend text only for recovery batches, under technical details', async () => {
        render(<HistoryView onBatchUndone={() => {}} />);
        fireEvent.click(await screen.findByText('Recuperare după oprire'));
        expect(
            await screen.findByText(
                'La pornire, Valet a verificat operațiile rămase neterminate după o oprire neașteptată.',
            ),
        ).toBeTruthy();
        const raw = screen.getByText(recoveryBatch.summary as string);
        expect(raw.closest('details')?.querySelector('summary')?.textContent).toBe('Detalii tehnice');
    });

    it('formats dates in the UI language', async () => {
        await i18n.changeLanguage('en');
        render(<HistoryView onBatchUndone={() => {}} />);
        const rows = await screen.findAllByTestId('history-row');
        expect(rows[0].textContent).toContain('2026');
        expect(rows[0].textContent).toMatch(/Oct 1, 2026/);
    });
});

describe('HistoryView on the Free plan', () => {
    function listAs(response: unknown) {
        invokeMock.mockImplementation(async (cmd, args) => {
            if (cmd === 'list_batches') return response;
            if (cmd === 'get_batch') return details[(args as { batchId: string }).batchId];
            throw new Error(`unexpected command ${cmd}`);
        });
    }

    it('says older batches are hidden but kept', async () => {
        listAs({ batches: [undoBatch, applyBatch], hidden_older: true, latest_apply_batch_id: 'a1' });
        render(<HistoryView onBatchUndone={() => {}} plan={freePlan()} />);
        expect((await screen.findByTestId('history-hidden-older')).textContent).toBe(
            'Planul Gratuit arată ultimele 7 zile. Istoricul complet rămâne salvat.',
        );
        expect(await screen.findAllByTestId('history-row')).toHaveLength(2);
    });

    it('shows no note when nothing is hidden', async () => {
        listAs({ batches: [undoBatch, applyBatch], hidden_older: false, latest_apply_batch_id: 'a1' });
        render(<HistoryView onBatchUndone={() => {}} plan={freePlan()} />);
        await screen.findAllByTestId('history-row');
        expect(screen.queryByTestId('history-hidden-older')).toBeNull();
    });

    const applyA: BatchInfo = { ...applyBatch, id: 'A', created_at: '2026-10-02T10:00:00Z' };
    const applyB: BatchInfo = { ...applyBatch, id: 'B', created_at: '2026-10-01T10:00:00Z' };
    const recoveryNewest: BatchInfo = { ...recoveryBatch, id: 'r0', created_at: '2026-10-03T09:00:00Z' };
    const LATEST_ONLY =
        'Planul Gratuit anulează doar ultimul lot. Anularea loturilor mai vechi e inclusă în Pro.';

    function listWithApplies(latest: string | null) {
        details.A = { batch: applyA, operations: [op(1, { batch_id: 'A', dest_path: 'C:\\out\\a.exe' })] };
        details.r0 = { batch: recoveryNewest, operations: [] };
        details.B = { batch: applyB, operations: [op(2, { batch_id: 'B', dest_path: 'C:\\out\\b.exe' })] };
        listAs({ batches: [recoveryNewest, applyA, applyB], hidden_older: false, latest_apply_batch_id: latest });
    }

    async function undoButtonOf(row: number) {
        fireEvent.click((await screen.findAllByTestId('history-row'))[row]);
        return (await screen.findByTestId('undo-start')) as HTMLButtonElement;
    }

    it('allows undo only of the latest apply batch, not of the newer recovery batch or older applies', async () => {
        listWithApplies('A');
        render(<HistoryView onBatchUndone={() => {}} plan={freePlan()} />);

        // The newest batch is a recovery batch: it has no undo at all.
        fireEvent.click((await screen.findAllByTestId('history-row'))[0]);
        expect(await screen.findByText('Lot r0')).toBeTruthy();
        expect(screen.queryByTestId('undo-start')).toBeNull();

        const a = await undoButtonOf(1);
        expect(a.disabled).toBe(false);
        expect(a.title).toBe('');

        fireEvent.click((await screen.findAllByTestId('history-row'))[2]);
        expect(await screen.findByText('Lot B')).toBeTruthy();
        const b = screen.getByTestId('undo-start') as HTMLButtonElement;
        expect(b.disabled).toBe(true);
        expect(b.title).toBe(LATEST_ONLY);
        expect(document.getElementById(b.getAttribute('aria-describedby') ?? '')?.textContent).toBe(LATEST_ONLY);
    });

    it('uses the backend id even when it is not in the visible list', async () => {
        listWithApplies('older-hidden');
        render(<HistoryView onBatchUndone={() => {}} plan={freePlan()} />);
        expect((await undoButtonOf(1)).disabled).toBe(true);
    });

    it('allows undo of any batch on Trial', async () => {
        render(<HistoryView onBatchUndone={() => {}} plan={trialPlan(4)} />);
        fireEvent.click(await screen.findByText('Organizare'));
        expect(((await screen.findByTestId('undo-start')) as HTMLButtonElement).disabled).toBe(false);
    });
});
