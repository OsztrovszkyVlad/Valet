import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import App from './App';
import type { AppSettings, BatchFinished, BatchInfo, DryRunResponse } from './api/commands';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));
const invokeMock = vi.mocked(invoke);
const listenMock = vi.mocked(listen);

let settings: AppSettings;
let batches: BatchInfo[];
let batchFinished: ((e: { payload: BatchFinished }) => void) | null;
const unlisten = vi.fn();

const PREVIEW: DryRunResponse = {
    results: [{ source_path: 'C:\\D\\a.pdf', destination_path: 'C:\\Docs\\a.pdf', rule_name: 'R', operation: 'move' }],
    total_files: 1,
    total_rules_applied: 1,
    skipped: [],
    reports: [],
    plan_id: 'plan-1',
};

beforeEach(() => {
    settings = {
        language: 'ro',
        notifications_enabled: true,
        auto_start: false,
        auto_apply_enabled: false,
        first_preview_approved: false,
        onboarding_done: false,
    };
    batches = [];
    batchFinished = null;
    listenMock.mockReset();
    listenMock.mockImplementation(async (event, handler) => {
        if (event === 'batch-finished') batchFinished = handler as typeof batchFinished;
        return unlisten;
    });
    invokeMock.mockReset();
    invokeMock.mockImplementation(async (cmd) => {
        switch (cmd) {
            case 'get_app_settings':
                return settings;
            case 'list_batches':
                return batches;
            case 'list_presets':
                return [];
            case 'complete_onboarding':
                settings = { ...settings, onboarding_done: true };
                return { config: { inbox_paths: ['C:\\D'] }, rules: [] };
            case 'dry_run':
                return PREVIEW;
            case 'close_preview':
                return undefined;
            case 'get_rules':
                return [];
            case 'get_config':
                return { inbox_paths: ['C:\\D'] };
            case 'get_batch':
                return { batch: batches[0], operations: [] };
            default:
                throw new Error(`unexpected command ${cmd}`);
        }
    });
});

function calls(cmd: string) {
    return invokeMock.mock.calls.filter(([c]) => c === cmd);
}

describe('App', () => {
    it('runs onboarding first, then shows its preview and closes it when the user leaves', async () => {
        render(<App />);
        expect(await screen.findByText('Ce foldere să urmărească Valet?')).toBeTruthy();
        expect(screen.queryByRole('tab')).toBeNull();

        fireEvent.click(screen.getByRole('button', { name: 'Continuă' }));
        fireEvent.click(await screen.findByRole('button', { name: 'Sari peste' }));

        expect(await screen.findByText('Pasul 3 din 3: previzualizare')).toBeTruthy();
        expect(screen.getByRole('tab', { name: 'Previzualizare', selected: true })).toBeTruthy();
        expect(screen.getByRole('button', { name: 'Organizează fișierele (1)' })).toBeTruthy();
        expect(calls('execute_operations')).toHaveLength(0);

        // The click handler itself sends close_preview: the plan id is known from
        // the commit that showed the list, not from a later passive effect.
        fireEvent.click(screen.getByRole('tab', { name: 'Reguli' }));
        expect(calls('close_preview')).toHaveLength(1);
        expect(await screen.findByText('Nicio regulă încă')).toBeTruthy();

        // Back on the Preview: no stale plan, and leaving again closes nothing.
        fireEvent.click(screen.getByRole('tab', { name: 'Previzualizare' }));
        expect(screen.queryByText('Pasul 3 din 3: previzualizare')).toBeNull();
        expect(screen.getByText(/Rulează previzualizarea ca să vezi lista/)).toBeTruthy();
        fireEvent.click(screen.getByRole('tab', { name: 'Istoric' }));
        expect(calls('close_preview')).toHaveLength(1);
        expect(await screen.findByText('Niciun lot încă')).toBeTruthy();
    });

    it('shows auto-apply batches in the last-batch box and reloads History', async () => {
        settings.onboarding_done = true;
        render(<App />);
        fireEvent.click(await screen.findByRole('tab', { name: 'Istoric' }));
        expect(await screen.findByText('Niciun lot încă')).toBeTruthy();
        await waitFor(() => expect(batchFinished).not.toBeNull());

        batches = [
            {
                id: 'b-auto',
                kind: 'apply',
                created_at: '2026-10-28T10:00:00Z',
                finished_at: '2026-10-28T10:00:01Z',
                summary: null,
                counts: { planned: 0, in_progress: 0, done: 2, failed: 0, rolled_back: 0 },
            },
        ];
        const before = calls('list_batches').length;
        act(() => batchFinished?.({ payload: { batch_id: 'b-auto', done: 2, failed: 0, auto: true } }));

        expect(await screen.findByText('organizat automat')).toBeTruthy();
        expect(screen.getByRole('region', { name: 'Ultimul lot' })).toBeTruthy();
        await waitFor(() => expect(calls('list_batches').length).toBeGreaterThan(before));
        expect(await screen.findByText('Organizare')).toBeTruthy();
    });

    it('ignores batches that did nothing for the undo box', async () => {
        settings.onboarding_done = true;
        render(<App />);
        await screen.findByRole('tab', { name: 'Istoric' });
        await waitFor(() => expect(batchFinished).not.toBeNull());
        act(() => batchFinished?.({ payload: { batch_id: 'b-0', done: 0, failed: 1, auto: true } }));
        expect(screen.queryByRole('region', { name: 'Ultimul lot' })).toBeNull();
    });
});
