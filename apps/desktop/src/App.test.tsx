import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import App from './App';
import type { AppSettings, BatchFinished, BatchInfo, DryRunResponse, PlanView, UpdateAvailable } from './api/commands';
import { freePlan, proPlan, trialPlan } from './test/plan';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));
const invokeMock = vi.mocked(invoke);
const listenMock = vi.mocked(listen);

let settings: AppSettings;
let batches: BatchInfo[];
let batchFinished: ((e: { payload: BatchFinished }) => void) | null;
let updateAvailable: ((e: { payload: UpdateAvailable }) => void) | null;
let planChanged: ((e: { payload: PlanView }) => void) | null;
/** `get_plan_status` answer; a string is thrown (the plan stays unknown). */
let planStatus: PlanView | string;
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
        update_channel: 'stable',
        debug_logging: false,
        auto_update_check: true,
    };
    batches = [];
    batchFinished = null;
    updateAvailable = null;
    planChanged = null;
    planStatus = 'not available in this test';
    listenMock.mockReset();
    listenMock.mockImplementation(async (event, handler) => {
        if (event === 'batch-finished') batchFinished = handler as typeof batchFinished;
        if (event === 'update-available') updateAvailable = handler as typeof updateAvailable;
        if (event === 'plan-changed') planChanged = handler as typeof planChanged;
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
            case 'get_auto_start':
                return false;
            case 'get_app_version':
                return '0.3.0';
            case 'get_plan_status':
                if (typeof planStatus === 'string') throw planStatus;
                return planStatus;
            case 'apply_plan_choice':
                return freePlan();
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

    it('marks the Settings button and shows a dismissible note when the daily check finds an update', async () => {
        settings.onboarding_done = true;
        render(<App />);
        await screen.findByRole('tab', { name: 'Previzualizare' });
        await waitFor(() => expect(updateAvailable).not.toBeNull());
        expect(screen.queryByTestId('nav-settings-badge')).toBeNull();
        expect(screen.queryByTestId('update-note')).toBeNull();

        act(() => updateAvailable?.({ payload: { version: '0.4.0' } }));
        expect(screen.getByRole('img', { name: 'Actualizare disponibilă' })).toBeTruthy();
        const note = screen.getByTestId('update-note');
        expect(within(note).getByText('Versiunea 0.4.0 e disponibilă.')).toBeTruthy();

        // The note goes away when closed; the badge stays.
        fireEvent.click(within(note).getByRole('button', { name: 'Închide' }));
        expect(screen.queryByTestId('update-note')).toBeNull();
        expect(screen.getByTestId('nav-settings-badge')).toBeTruthy();
        expect(calls('check_for_update')).toHaveLength(0);
        expect(calls('install_update')).toHaveLength(0);
    });

    it('opens Settings from the update note, where the update can be installed', async () => {
        settings.onboarding_done = true;
        render(<App />);
        await screen.findByRole('tab', { name: 'Previzualizare' });
        await waitFor(() => expect(updateAvailable).not.toBeNull());
        act(() => updateAvailable?.({ payload: { version: '0.4.0' } }));

        fireEvent.click(screen.getByRole('button', { name: 'Vezi în Setări' }));
        expect(await screen.findByRole('heading', { name: 'Setări' })).toBeTruthy();
        expect(await screen.findByText('Versiunea 0.4.0 e disponibilă.')).toBeTruthy();
        expect(screen.getByRole('button', { name: 'Instalează și repornește' })).toBeTruthy();
        expect(screen.queryByTestId('update-note')).toBeNull();
        expect(calls('install_update')).toHaveLength(0);
    });

    it('shows the trial days in the header, nothing on Pro, and a Free link to the Pro section', async () => {
        settings.onboarding_done = true;
        planStatus = trialPlan(5);
        render(<App />);
        expect((await screen.findByTestId('nav-plan')).textContent).toBe('Probă Pro: 5 zile');
        expect(screen.queryByRole('dialog')).toBeNull();

        await waitFor(() => expect(planChanged).not.toBeNull());
        act(() => planChanged?.({ payload: proPlan() }));
        expect(screen.queryByTestId('nav-plan')).toBeNull();

        act(() => planChanged?.({ payload: freePlan() }));
        fireEvent.click(screen.getByTestId('nav-plan'));
        expect(screen.getByTestId('nav-plan').textContent).toBe('Gratuit');
        expect(await screen.findByTestId('pro-section')).toBeTruthy();
        await waitFor(() => expect(document.activeElement).toBe(screen.getByTestId('pro-section')));
    });

    it('asks what stays active when the choice is pending, and again after decide later on a plan change', async () => {
        settings.onboarding_done = true;
        planStatus = freePlan({ choice_pending: true });
        render(<App />);
        const dialog = await screen.findByRole('dialog', { name: 'Alege ce rămâne activ în planul Gratuit' });
        fireEvent.keyDown(dialog, { key: 'Escape' });
        expect(screen.queryByRole('dialog')).toBeNull();

        await waitFor(() => expect(planChanged).not.toBeNull());
        act(() => planChanged?.({ payload: freePlan({ choice_pending: true }) }));
        expect(await screen.findByRole('dialog')).toBeTruthy();
        await screen.findByText('Nu ai reguli proprii.');
        fireEvent.click(screen.getByTestId('plan-choice-save'));
        await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
        expect(calls('apply_plan_choice')[0][1]).toEqual({ ruleIds: [], folders: ['C:\\D'] });
    });

    it('works without a plan when get_plan_status fails', async () => {
        settings.onboarding_done = true;
        render(<App />);
        await screen.findByRole('tab', { name: 'Previzualizare' });
        await waitFor(() => expect(calls('get_plan_status')).toHaveLength(1));
        expect(screen.queryByTestId('nav-plan')).toBeNull();
        expect(screen.queryByRole('dialog')).toBeNull();
    });
});
