import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import {
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
    type AppSettings,
} from './commands';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

const invokeMock = vi.mocked(invoke);

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
