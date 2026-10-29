import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import {
    checkForUpdate,
    getAppVersion,
    installUpdate,
    onUpdateAvailable,
    onUpdateProgress,
    closePreview,
    completeOnboarding,
    configureAutoStart,
    deleteRule,
    getRules,
    installPresets,
    listPresets,
    upsertRule,
    errorMessage,
    executeOperations,
    getAppSettings,
    getAutoStart,
    getConfig,
    listBatches,
    parseCommandError,
    saveAppSettings,
    saveConfig,
    undoBatch,
    applyPlanChoice,
    checkLicenseNow,
    exportRulesFile,
    getPlanStatus,
    importLicenseFile,
    importLicenseText,
    importRulesFile,
    onPlanChanged,
    openExternal,
    planLimitOf,
    removeLicense,
    type AppSettings,
} from './commands';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));

const invokeMock = vi.mocked(invoke);
const listenMock = vi.mocked(listen);

beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(undefined);
});

describe('command wrappers', () => {
    it('pass the command name and camelCase arguments', async () => {
        await executeOperations('plan-1');
        expect(invokeMock).toHaveBeenLastCalledWith('execute_operations', { planId: 'plan-1' });

        await listBatches();
        expect(invokeMock).toHaveBeenLastCalledWith('list_batches', { limit: null });
        await listBatches(5);
        expect(invokeMock).toHaveBeenLastCalledWith('list_batches', { limit: 5 });

        await undoBatch('b1', true);
        expect(invokeMock).toHaveBeenLastCalledWith('undo_batch', { batchId: 'b1', force: true });

        await configureAutoStart(false);
        expect(invokeMock).toHaveBeenLastCalledWith('configure_auto_start', { enabled: false });

        await getAutoStart();
        expect(invokeMock).toHaveBeenLastCalledWith('get_auto_start');
    });

    it('send settings and config as single objects and return the saved value', async () => {
        const settings: AppSettings = {
            language: 'en',
            notifications_enabled: true,
            auto_start: false,
            auto_apply_enabled: false,
            first_preview_approved: false,
            onboarding_done: true,
            update_channel: 'stable',
            debug_logging: false,
            auto_update_check: true,
        };
        invokeMock.mockResolvedValueOnce(settings);
        await expect(saveAppSettings(settings)).resolves.toEqual(settings);
        expect(invokeMock).toHaveBeenLastCalledWith('save_app_settings', { settings });

        const config = { inbox_paths: ['C:\\Users\\a\\Downloads'], pause_watchers: false };
        invokeMock.mockResolvedValueOnce(config);
        await expect(saveConfig(config)).resolves.toEqual(config);
        expect(invokeMock).toHaveBeenLastCalledWith('save_config', { config });

        await getAppSettings();
        expect(invokeMock).toHaveBeenLastCalledWith('get_app_settings');
        await getConfig();
        expect(invokeMock).toHaveBeenLastCalledWith('get_config');
    });

    it('call the rule, preset and onboarding commands with their argument names', async () => {
        await getRules();
        expect(invokeMock).toHaveBeenLastCalledWith('get_rules');
        const rule = {
            id: 'id-1',
            name: 'r',
            enabled: true,
            alwaysApply: false,
            conditions: [{ type: 'ext', value: 'pdf' }],
            actions: [{ type: 'report', params: {} }],
            version: 1,
            options: {},
            scope: [],
        };
        await upsertRule(rule);
        expect(invokeMock).toHaveBeenLastCalledWith('upsert_rule', { rule });
        await deleteRule('id-1');
        expect(invokeMock).toHaveBeenLastCalledWith('delete_rule', { ruleId: 'id-1' });

        await listPresets();
        expect(invokeMock).toHaveBeenLastCalledWith('list_presets');
        await installPresets(['a', 'b']);
        expect(invokeMock).toHaveBeenLastCalledWith('install_presets', { presetIds: ['a', 'b'] });
        await completeOnboarding(['{downloads}'], ['a']);
        expect(invokeMock).toHaveBeenLastCalledWith('complete_onboarding', {
            folders: ['{downloads}'],
            presetIds: ['a'],
        });
        await closePreview();
        expect(invokeMock).toHaveBeenLastCalledWith('close_preview');
    });

    it('pass rejections through unchanged', async () => {
        invokeMock.mockRejectedValueOnce('preview_required');
        await expect(executeOperations('old')).rejects.toBe('preview_required');
    });
});

describe('updater wrappers', () => {
    it('call the updater commands without arguments', async () => {
        invokeMock.mockResolvedValueOnce('0.3.0');
        await expect(getAppVersion()).resolves.toBe('0.3.0');
        expect(invokeMock).toHaveBeenLastCalledWith('get_app_version');

        const found = { available: true, version: '0.4.0', notes: 'n', date: '2026-10-29T10:00:00Z' };
        invokeMock.mockResolvedValueOnce(found);
        await expect(checkForUpdate()).resolves.toEqual(found);
        expect(invokeMock).toHaveBeenLastCalledWith('check_for_update');

        await installUpdate();
        expect(invokeMock).toHaveBeenLastCalledWith('install_update');

        invokeMock.mockRejectedValueOnce('update.busy');
        await expect(installUpdate()).rejects.toBe('update.busy');
    });

    it('subscribe to update-progress and update-available with the payload', async () => {
        const handlers = new Map<string, (e: { payload: unknown }) => void>();
        const unlisten = vi.fn();
        listenMock.mockImplementation(async (event, handler) => {
            handlers.set(event, handler as (e: { payload: unknown }) => void);
            return unlisten;
        });
        const progress = vi.fn();
        const available = vi.fn();
        await expect(onUpdateProgress(progress)).resolves.toBe(unlisten);
        await expect(onUpdateAvailable(available)).resolves.toBe(unlisten);
        handlers.get('update-progress')?.({ payload: { downloaded: 10, total: null } });
        handlers.get('update-available')?.({ payload: { version: '0.4.0' } });
        expect(progress).toHaveBeenCalledWith({ downloaded: 10, total: null });
        expect(available).toHaveBeenCalledWith({ version: '0.4.0' });
    });
});

describe('parseCommandError', () => {
    it('reads a bare code', () => {
        expect(parseCommandError('config.driveRoot')).toEqual({ code: 'config.driveRoot', detail: null });
    });

    it('reads a code with a detail', () => {
        expect(parseCommandError('config.missing: C:\\nope')).toEqual({
            code: 'config.missing',
            detail: 'C:\\nope',
        });
    });

    it('reads an object with code and detail', () => {
        expect(parseCommandError({ code: 'config.excludedFolder', detail: 'C:\\Windows' })).toEqual({
            code: 'config.excludedFolder',
            detail: 'C:\\Windows',
        });
        expect(parseCommandError({ code: 'config.driveRoot' })).toEqual({
            code: 'config.driveRoot',
            detail: null,
        });
    });

    it('returns null for free text and other values', () => {
        expect(parseCommandError('Failed to read config file: denied')).toBeNull();
        expect(parseCommandError('preview_required')).toBeNull();
        expect(parseCommandError(42)).toBeNull();
        expect(parseCommandError({ code: 7 })).toBeNull();
        expect(parseCommandError(null)).toBeNull();
    });
});

describe('errorMessage', () => {
    it('handles strings, errors and objects with a message', () => {
        expect(errorMessage('x')).toBe('x');
        expect(errorMessage(new Error('boom'))).toBe('boom');
        expect(errorMessage({ message: 'm' })).toBe('m');
        expect(errorMessage(3)).toBe('3');
    });
});

describe('plan and license wrappers', () => {
    it('call the contract commands with their arguments', async () => {
        await getPlanStatus();
        expect(invokeMock).toHaveBeenLastCalledWith('get_plan_status');
        await importLicenseFile();
        expect(invokeMock).toHaveBeenLastCalledWith('import_license_file');
        await importLicenseText('{"payload":"p"}');
        expect(invokeMock).toHaveBeenLastCalledWith('import_license_text', { text: '{"payload":"p"}' });
        await checkLicenseNow();
        expect(invokeMock).toHaveBeenLastCalledWith('check_license_now');
        await removeLicense();
        expect(invokeMock).toHaveBeenLastCalledWith('remove_license');
        await applyPlanChoice(['r1'], ['C:\\A']);
        expect(invokeMock).toHaveBeenLastCalledWith('apply_plan_choice', { ruleIds: ['r1'], folders: ['C:\\A'] });
        await openExternal('checkout');
        expect(invokeMock).toHaveBeenLastCalledWith('open_external', { target: 'checkout' });
        await openExternal('startup_settings');
        expect(invokeMock).toHaveBeenLastCalledWith('open_external', { target: 'startup_settings' });
    });

    it('read list_batches with hidden_older and latest_apply_batch_id', async () => {
        const recovery = { id: 'r', kind: 'recovery' };
        const apply = { id: 'a', kind: 'apply' };
        invokeMock.mockResolvedValueOnce({ batches: [recovery, apply], hidden_older: true, latest_apply_batch_id: 'x' });
        await expect(listBatches()).resolves.toEqual({
            batches: [recovery, apply],
            hidden_older: true,
            latest_apply_batch_id: 'x',
        });
        invokeMock.mockResolvedValueOnce({ batches: [], hidden_older: false, latest_apply_batch_id: null });
        await expect(listBatches()).resolves.toEqual({ batches: [], hidden_older: false, latest_apply_batch_id: null });
        // A plain array (older backend): no plan limits, the newest apply batch.
        invokeMock.mockResolvedValueOnce([recovery, apply]);
        await expect(listBatches()).resolves.toEqual({
            batches: [recovery, apply],
            hidden_older: false,
            latest_apply_batch_id: 'a',
        });
    });

    it('read the rule export path and the imported rules, null being a cancelled dialog', async () => {
        invokeMock.mockResolvedValueOnce(null);
        await expect(exportRulesFile()).resolves.toBeNull();
        invokeMock.mockResolvedValueOnce('C:\\r.json');
        await expect(exportRulesFile()).resolves.toBe('C:\\r.json');
        expect(invokeMock).toHaveBeenLastCalledWith('export_rules_file');

        invokeMock.mockResolvedValueOnce(null);
        await expect(importRulesFile()).resolves.toBeNull();
        invokeMock.mockResolvedValueOnce([{ id: '1' }, { id: '2' }]);
        await expect(importRulesFile()).resolves.toEqual([{ id: '1' }, { id: '2' }]);
        expect(invokeMock).toHaveBeenLastCalledWith('import_rules_file');
    });

    it('subscribe to plan-changed with the PlanView payload', async () => {
        let handler: ((e: { payload: unknown }) => void) | null = null;
        listenMock.mockImplementationOnce(async (_event, h) => {
            handler = h as (e: { payload: unknown }) => void;
            return () => {};
        });
        const seen: unknown[] = [];
        await onPlanChanged((p) => seen.push(p));
        expect(listenMock).toHaveBeenLastCalledWith('plan-changed', expect.any(Function));
        (handler as unknown as (e: { payload: unknown }) => void)({ payload: { choice_pending: true } });
        expect(seen).toEqual([{ choice_pending: true }]);
    });

    it('read plan.limit errors as objects or strings', () => {
        expect(planLimitOf({ code: 'plan.limit', kind: 'folders', limit: 2 })).toEqual({ kind: 'folders', limit: 2 });
        expect(planLimitOf('plan.limit: rules 3')).toEqual({ kind: 'rules', limit: 3 });
        expect(planLimitOf('plan.limit')).toEqual({ kind: 'rules', limit: 3 });
        expect(planLimitOf('plan.notFree')).toBeNull();
        expect(planLimitOf(new Error('x'))).toBeNull();
    });
});
