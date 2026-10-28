import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import i18n from '../i18n';
import SettingsView from './SettingsView';
import type { AppSettings, Config } from '../api/commands';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
const invokeMock = vi.mocked(invoke);

const DOWNLOADS = 'C:\\Users\\ana\\Downloads';

let settings: AppSettings;
let config: Config;
let osAutoStart: boolean | Error;

function baseSettings(): AppSettings {
    return {
        language: 'ro',
        notifications_enabled: true,
        auto_start: false,
        auto_apply_enabled: false,
        first_preview_approved: false,
        onboarding_done: true,
    };
}

beforeEach(() => {
    settings = baseSettings();
    config = { inbox_paths: [DOWNLOADS], pause_watchers: false };
    osAutoStart = false;
    invokeMock.mockReset();
    invokeMock.mockImplementation(async (cmd, args) => {
        const a = args as Record<string, unknown> | undefined;
        switch (cmd) {
            case 'get_app_settings':
                return settings;
            case 'get_config':
                return config;
            case 'save_app_settings':
                settings = a?.settings as AppSettings;
                return settings;
            case 'save_config': {
                const next = a?.config as Config;
                if (next.inbox_paths.includes('D:\\')) throw 'config.driveRoot';
                if (next.inbox_paths.includes('C:\\Windows')) throw 'config.excludedFolder: C:\\Windows';
                if (next.inbox_paths.includes('X:\\broken')) throw 'Failed to write config file: denied';
                config = next;
                return config;
            }
            case 'configure_auto_start':
                return undefined;
            case 'get_auto_start':
                if (osAutoStart instanceof Error) throw osAutoStart.message;
                return osAutoStart;
            default:
                throw new Error(`unexpected command ${cmd}`);
        }
    });
});

function callsTo(cmd: string) {
    return invokeMock.mock.calls.filter(([c]) => c === cmd);
}

async function renderLoaded() {
    render(<SettingsView />);
    expect(await screen.findByText(DOWNLOADS)).toBeTruthy();
}

function addFolder(path: string) {
    fireEvent.change(screen.getByLabelText('Cale folder'), { target: { value: path } });
    fireEvent.click(screen.getByRole('button', { name: 'Adaugă' }));
}

describe('SettingsView', () => {
    it('renders the saved values in Romanian', async () => {
        await renderLoaded();
        expect(screen.getByRole('heading', { name: 'Setări' })).toBeTruthy();
        expect((screen.getByLabelText('Limba') as HTMLSelectElement).value).toBe('ro');
        expect((screen.getByLabelText('Notificare scurtă după un lot') as HTMLInputElement).checked).toBe(true);
        expect((screen.getByLabelText('Pornește Valet odată cu Windows') as HTMLInputElement).checked).toBe(false);
    });

    it('keeps auto-apply disabled until a preview was approved', async () => {
        await renderLoaded();
        const autoApply = screen.getByLabelText('Aplicare automată în folderele urmărite') as HTMLInputElement;
        expect(autoApply.disabled).toBe(true);
        expect(screen.getByText(/Disponibil după ce aprobi prima previzualizare\./)).toBeTruthy();
    });

    it('saves auto-apply once a preview was approved', async () => {
        settings = { ...baseSettings(), first_preview_approved: true };
        await renderLoaded();
        const autoApply = screen.getByLabelText('Aplicare automată în folderele urmărite') as HTMLInputElement;
        expect(autoApply.disabled).toBe(false);
        expect(screen.queryByText(/Disponibil după ce aprobi/)).toBeNull();

        fireEvent.click(autoApply);
        await screen.findByText('Salvat.');
        expect(callsTo('save_app_settings').at(-1)?.[1]).toEqual({
            settings: { ...baseSettings(), first_preview_approved: true, auto_apply_enabled: true },
        });
    });

    it('adds and removes watched folders, keeping the other config fields', async () => {
        await renderLoaded();
        addFolder('  C:\\Users\\ana\\Desktop ');
        expect(await screen.findByText('C:\\Users\\ana\\Desktop')).toBeTruthy();
        expect(callsTo('save_config').at(-1)?.[1]).toEqual({
            config: { inbox_paths: [DOWNLOADS, 'C:\\Users\\ana\\Desktop'], pause_watchers: false },
        });
        expect((screen.getByLabelText('Cale folder') as HTMLInputElement).value).toBe('');

        fireEvent.click(screen.getByRole('button', { name: `Elimină ${DOWNLOADS}` }));
        await waitFor(() => expect(screen.queryByText(DOWNLOADS)).toBeNull());
        expect(config.inbox_paths).toEqual(['C:\\Users\\ana\\Desktop']);
    });

    it('shows translated backend errors and keeps the list unchanged', async () => {
        await renderLoaded();

        addFolder('D:\\');
        expect(
            await screen.findByText('Nu poți urmări un disc întreg. Alege un folder, de exemplu Descărcări.'),
        ).toBeTruthy();
        expect(screen.queryByText('D:\\')).toBeNull();
        expect((screen.getByLabelText('Cale folder') as HTMLInputElement).value).toBe('D:\\');

        addFolder('C:\\Windows');
        expect(await screen.findByText(/^Folderul e exclus implicit.*\(C:\\Windows\)$/)).toBeTruthy();

        addFolder('X:\\broken');
        expect(await screen.findByText('Eroare: Failed to write config file: denied')).toBeTruthy();
        expect(config.inbox_paths).toEqual([DOWNLOADS]);
    });

    it('rejects a folder that is already in the list without calling the backend', async () => {
        await renderLoaded();
        addFolder('c:\\users\\ana\\downloads');
        expect(await screen.findByText('Folderul e deja în listă.')).toBeTruthy();
        expect(callsTo('save_config')).toHaveLength(0);
    });

    it('switches the language at once and saves it', async () => {
        await renderLoaded();
        fireEvent.change(screen.getByLabelText('Limba'), { target: { value: 'en' } });
        expect(await screen.findByRole('heading', { name: 'Settings' })).toBeTruthy();
        expect(i18n.language).toBe('en');
        await screen.findByText('Saved.');
        expect(settings.language).toBe('en');
    });

    it('turns autostart on through the platform command, then saves it', async () => {
        await renderLoaded();
        fireEvent.click(screen.getByLabelText('Pornește Valet odată cu Windows'));
        await screen.findByText('Salvat.');
        expect(callsTo('configure_auto_start')).toEqual([['configure_auto_start', { enabled: true }]]);
        expect(settings.auto_start).toBe(true);
    });

    it('shows the real auto-start state, falling back to the saved value', async () => {
        osAutoStart = true;
        await renderLoaded();
        expect((screen.getByLabelText('Pornește Valet odată cu Windows') as HTMLInputElement).checked).toBe(true);
    });

    it('uses the saved auto-start value when the state cannot be read', async () => {
        osAutoStart = new Error('Failed to read auto-start state');
        settings = { ...baseSettings(), auto_start: true };
        await renderLoaded();
        expect((screen.getByLabelText('Pornește Valet odată cu Windows') as HTMLInputElement).checked).toBe(true);
    });

    it('does not save autostart when the platform command fails', async () => {
        await renderLoaded();
        invokeMock.mockImplementationOnce(async () => {
            throw 'Auto-start configuration not supported on this platform';
        });
        fireEvent.click(screen.getByLabelText('Pornește Valet odată cu Windows'));
        expect(
            await screen.findByText('Eroare: Auto-start configuration not supported on this platform'),
        ).toBeTruthy();
        expect(callsTo('save_app_settings')).toHaveLength(0);
        expect((screen.getByLabelText('Pornește Valet odată cu Windows') as HTMLInputElement).checked).toBe(false);
    });

    it('saves the notifications switch', async () => {
        await renderLoaded();
        fireEvent.click(screen.getByLabelText('Notificare scurtă după un lot'));
        await screen.findByText('Salvat.');
        expect(settings.notifications_enabled).toBe(false);
    });

    it('shows a translated message when settings cannot be read', async () => {
        invokeMock.mockImplementation(async () => {
            throw 'Failed to parse settings JSON';
        });
        render(<SettingsView />);
        expect(
            await screen.findByText('Setările nu au putut fi citite: Failed to parse settings JSON'),
        ).toBeTruthy();
    });
});
