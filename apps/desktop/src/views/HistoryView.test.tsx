import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import HistoryView from './HistoryView';
import type { BatchDetail, BatchInfo, OpRecord } from '../api/commands';

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

beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation(async (cmd) => {
        if (cmd === 'list_batches') return [undoBatch];
        if (cmd === 'get_batch') return detail;
        throw new Error(`unexpected command ${cmd}`);
    });
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
});
