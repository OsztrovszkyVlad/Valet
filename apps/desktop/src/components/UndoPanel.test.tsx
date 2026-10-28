import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import UndoPanel from './UndoPanel';
import type { BatchDetail, OpRecord, UndoReport } from '../api/commands';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
const invokeMock = vi.mocked(invoke);

function op(id: number, source: string, dest: string): OpRecord {
    return {
        id,
        batch_id: 'b1',
        seq: id,
        op_type: 'move',
        source_path: source,
        dest_path: dest,
        size: 1,
        hash_before: null,
        hash_after: null,
        state: 'done',
        error: null,
        created_at: '2026-10-01T10:00:00Z',
        finished_at: '2026-10-01T10:00:01Z',
        undo_of: null,
    };
}

const detail: BatchDetail = {
    batch: {
        id: 'b1',
        kind: 'apply',
        created_at: '2026-10-01T10:00:00Z',
        finished_at: '2026-10-01T10:00:02Z',
        summary: null,
        counts: { planned: 0, in_progress: 0, done: 2, failed: 0, rolled_back: 0 },
    },
    operations: [op(1, 'C:\\in\\a.pdf', 'C:\\out\\a.pdf'), op(2, 'C:\\in\\b.pdf', 'C:\\out\\b.pdf')],
};

function report(partial: Partial<UndoReport>): UndoReport {
    return {
        batch_id: 'b1',
        undo_batch_id: 'u1',
        undone: [],
        restored_as: [],
        needs_confirmation: [],
        failed: [],
        skipped: [],
        restored_modified: [],
        ...partial,
    };
}

const firstReport = report({
    undone: [1],
    needs_confirmation: [
        { op_id: 2, source: 'C:\\in\\b.pdf', dest: 'C:\\out\\b.pdf', reasons: ['Modified'] },
    ],
});

beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation(async (cmd, args) => {
        if (cmd === 'get_batch') return detail;
        if (cmd === 'undo_batch') {
            const { force } = args as { force: boolean };
            return force ? report({ undone: [2] }) : firstReport;
        }
        throw new Error(`unexpected command ${cmd}`);
    });
});

async function reachConfirmation() {
    fireEvent.click(screen.getByRole('button', { name: 'Anulează lotul' }));
    expect(await screen.findByText('Fișiere de mutat înapoi: 2')).toBeTruthy();
    // Nothing is undone before the user confirms the preview.
    expect(invokeMock).not.toHaveBeenCalledWith('undo_batch', expect.anything());
    fireEvent.click(screen.getByRole('button', { name: 'Confirmă anularea' }));
    expect(await screen.findByText('Fișiere care cer confirmare: 1')).toBeTruthy();
    expect(invokeMock).toHaveBeenCalledWith('undo_batch', { batchId: 'b1', force: false });
}

describe('UndoPanel', () => {
    it('asks again for changed files and forces only after the second confirmation', async () => {
        const onClose = vi.fn();
        const onUndoRan = vi.fn();
        render(<UndoPanel batchId="b1" canUndo onClose={onClose} onUndoRan={onUndoRan} />);

        await reachConfirmation();
        expect(screen.getByText('Fișierul s-a schimbat după lot')).toBeTruthy();
        expect(screen.getByText('C:\\out\\b.pdf')).toBeTruthy();

        fireEvent.click(screen.getByRole('button', { name: 'Anulează oricum' }));
        expect(await screen.findByText('Rezultatul anulării')).toBeTruthy();
        expect(screen.getByTestId('undo-result').contains(screen.getByTestId('undo-result-close'))).toBe(true);
        expect(invokeMock).toHaveBeenCalledWith('undo_batch', { batchId: 'b1', force: true });
        expect(screen.getByText('Operații anulate: 2')).toBeTruthy();
        expect(onUndoRan).toHaveBeenCalledTimes(2);

        fireEvent.click(screen.getByRole('button', { name: 'Închide' }));
        expect(onClose).toHaveBeenCalledWith(true);
    });

    it('leaves changed files in place when the user declines', async () => {
        const onClose = vi.fn();
        render(<UndoPanel batchId="b1" canUndo onClose={onClose} />);

        await reachConfirmation();
        fireEvent.click(screen.getByRole('button', { name: 'Renunță' }));

        expect(await screen.findByText('Rămase neanulate (fără confirmare): 1')).toBeTruthy();
        expect(invokeMock).not.toHaveBeenCalledWith('undo_batch', { batchId: 'b1', force: true });

        fireEvent.click(screen.getByRole('button', { name: 'Închide' }));
        expect(onClose).toHaveBeenCalledWith(false);
    });

    it('restores trashed files and reports changed content and a taken original path', async () => {
        const trashed = { ...op(3, 'C:\\in\\old.exe', ''), op_type: 'trash' as const, dest_path: null };
        const blocked = { ...op(4, 'C:\\in\\big.msi', ''), op_type: 'trash' as const, dest_path: null };
        invokeMock.mockImplementation(async (cmd, args) => {
            if (cmd === 'get_batch') return { ...detail, operations: [trashed, blocked] };
            if (cmd === 'undo_batch') {
                const { force } = args as { force: boolean };
                if (!force) {
                    return report({
                        undone: [],
                        needs_confirmation: [
                            { op_id: 3, source: 'C:\\in\\old.exe', dest: null, reasons: ['Modified'] },
                            { op_id: 4, source: 'C:\\in\\big.msi', dest: null, reasons: ['SourceOccupied'] },
                        ],
                    });
                }
                return report({
                    undone: [3],
                    restored_modified: [{ op_id: 3, path: 'C:\\in\\old.exe' }],
                    failed: [
                        {
                            op_id: 4,
                            source: 'C:\\in\\big.msi',
                            dest: null,
                            error: { code: 'RestoreOccupied', detail: null },
                        },
                    ],
                });
            }
            throw new Error(`unexpected command ${cmd}`);
        });
        const onClose = vi.fn();
        render(<UndoPanel batchId="b1" canUndo onClose={onClose} />);

        fireEvent.click(screen.getByTestId('undo-start'));
        expect(await screen.findByText('Fișiere de mutat înapoi: 2')).toBeTruthy();
        expect(screen.getAllByText(/restaurare din Coș/)).toHaveLength(2);

        fireEvent.click(screen.getByTestId('undo-confirm'));
        expect(await screen.findByText('Fișiere care cer confirmare: 2')).toBeTruthy();
        fireEvent.click(screen.getByTestId('undo-force'));

        expect(await screen.findByText('Rezultatul anulării')).toBeTruthy();
        expect(screen.getByText('Operații anulate: 1')).toBeTruthy();
        expect(
            screen.getByText(
                'C:\\in\\old.exe: restaurat, dar conținutul diferă de cel de la momentul trimiterii la Coș',
            ),
        ).toBeTruthy();
        expect(screen.getByText('Eșuate: 1')).toBeTruthy();
        expect(screen.getByText(/Locul inițial e ocupat; elementul a rămas în Coșul de reciclare/)).toBeTruthy();

        fireEvent.click(screen.getByRole('button', { name: 'Închide' }));
        expect(onClose).toHaveBeenCalledWith(false);
    });

    it('shows no start button when undo is not allowed', () => {
        render(<UndoPanel batchId="b1" canUndo={false} />);
        expect(screen.queryByRole('button', { name: 'Anulează lotul' })).toBeNull();
    });
});
