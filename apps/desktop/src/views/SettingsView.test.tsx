import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import i18n from '../i18n';
import SettingsView from './SettingsView';
import type { AppSettings, Config, InvalidConfigPath, UpdateCheck } from '../api/commands';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));
const invokeMock = vi.mocked(invoke);
const listenMock = vi.mocked(listen);

const DOWNLOADS = 'C:\\Users\\ana\\Downloads';

let settings: AppSettings;
let config: Config;
let invalidPaths: InvalidConfigPath[];
let unavailablePaths: string[];
let osAutoStart: boolean | Error;
let appVersion: string | Error;
let updateCheck: UpdateCheck | string;
let installResults: (string | undefined)[];
let saveSettingsError: string | null;
const eventHandlers = new Map<string, (e: { payload: unknown }) => void>();
const unlisten = vi.fn();

/** Sends a backend event to the subscribed handler. */
function emit(event: string, payload: unknown) {
    const handler = eventHandlers.get(event);
    if (!handler) throw new Error(`no listener for ${event}`);
    act(() => handler({ payload }));
}

function baseSettings(): AppSettings {
    return {
        language: 'ro',
        notifications_enabled: true,
        auto_start: false,
        auto_apply_enabled: false,
        first_preview_approved: false,
        onboarding_done: true,
        update_channel: 'stable',
        debug_logging: false,
    };
}

beforeEach(() => {
    settings = baseSettings();
    config = { inbox_paths: [DOWNLOADS], pause_watchers: false };
    invalidPaths = [];
    unavailablePaths = [];
    osAutoStart = false;
    appVersion = '0.3.0';
    updateCheck = { available: false };
    installResults = [];
    saveSettingsError = null;
    eventHandlers.clear();
    listenMock.mockReset();
    listenMock.mockImplementation(async (event, handler) => {
        eventHandlers.set(event, handler as (e: { payload: unknown }) => void);
        return unlisten;
    });
    invokeMock.mockReset();
    invokeMock.mockImplementation(async (cmd, args) => {
        const a = args as Record<string, unknown> | undefined;
        switch (cmd) {
            case 'get_app_settings':
                return settings;
            case 'get_config':
                return {
                    ...config,
                    invalid_paths: invalidPaths.filter((p) => config.inbox_paths.includes(p.path)),
                    unavailable_paths: unavailablePaths.filter((p) => config.inbox_paths.includes(p)),
                };
            case 'save_app_settings': {
                const next = a?.settings as AppSettings;
                if (next.update_channel !== 'stable' && next.update_channel !== 'beta') {
                    throw 'settings.invalidChannel';
                }
                if (saveSettingsError !== null) throw saveSettingsError;
                settings = next;
                return settings;
            }
            case 'save_config': {
                const next = a?.config as Config;
                if (next.inbox_paths.includes('D:\\')) throw 'config.driveRoot';
                if (next.inbox_paths.includes('C:\\Windows')) throw 'config.excludedFolder: C:\\Windows';
                if (next.inbox_paths.includes('X:\\broken')) throw 'Failed to write config file: denied';
                const bad = invalidPaths.find((p) => next.inbox_paths.includes(p.path));
                if (bad) throw `${bad.code}: ${bad.path}`;
                config = next;
                return config;
            }
            case 'configure_auto_start':
                return undefined;
            case 'get_auto_start':
                if (osAutoStart instanceof Error) throw osAutoStart.message;
                return osAutoStart;
            case 'get_app_version':
                if (appVersion instanceof Error) throw appVersion.message;
                return appVersion;
            case 'check_for_update':
                if (typeof updateCheck === 'string') throw updateCheck;
                return updateCheck;
            case 'install_update': {
                const outcome = installResults.shift();
                if (outcome !== undefined) throw outcome;
                return undefined;
            }
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

    it('offers the Preview after a folder is added', async () => {
        const onOpenPreview = vi.fn();
        render(<SettingsView onOpenPreview={onOpenPreview} />);
        await screen.findByText(DOWNLOADS);
        expect(screen.queryByText(/Folderul nou se organizează/)).toBeNull();

        addFolder('C:\\Users\\ana\\Desktop');
        expect(
            await screen.findByText('Folderul nou se organizează automat doar după o previzualizare aprobată.'),
        ).toBeTruthy();
        fireEvent.click(screen.getByRole('button', { name: 'Deschide Previzualizarea' }));
        expect(onOpenPreview).toHaveBeenCalledTimes(1);
    });

    it('does not show the new-folder note when adding fails', async () => {
        await renderLoaded();
        addFolder('D:\\');
        await screen.findByText(/Nu poți urmări un disc întreg/);
        expect(screen.queryByText(/Folderul nou se organizează/)).toBeNull();
    });

    it('marks invalid entries with the reason and removes them, saving the valid list', async () => {
        config = {
            inbox_paths: ['E:\\', DOWNLOADS, 'C:\\Users\\ana\\AppData\\x'],
            pause_watchers: false,
        };
        invalidPaths = [
            { path: 'E:\\', code: 'config.driveRoot' },
            { path: 'C:\\Users\\ana\\AppData\\x', code: 'config.excludedFolder' },
        ];
        await renderLoaded();
        expect(screen.getByText('E:\\')).toBeTruthy();
        expect(
            screen.getByText('Nu e urmărit: Nu poți urmări un disc întreg. Alege un folder, de exemplu Descărcări.'),
        ).toBeTruthy();
        expect(screen.getByText(/^Nu e urmărit: Folderul e exclus implicit/)).toBeTruthy();
        // The backend refuses any list with an invalid entry, so other edits wait.
        expect((screen.getByRole('button', { name: `Elimină ${DOWNLOADS}` }) as HTMLButtonElement).disabled).toBe(true);
        fireEvent.change(screen.getByLabelText('Cale folder'), { target: { value: 'C:\\Users\\ana\\Desktop' } });
        expect((screen.getByRole('button', { name: 'Adaugă' }) as HTMLButtonElement).disabled).toBe(true);

        fireEvent.click(screen.getByRole('button', { name: 'Elimină folderele neurmărite' }));
        await screen.findByText('Salvat.');
        expect(callsTo('save_config').at(-1)?.[1]).toEqual({
            config: { inbox_paths: [DOWNLOADS], pause_watchers: false },
        });
        expect(screen.queryByText('E:\\')).toBeNull();
        expect(screen.queryByText(/Nu e urmărit/)).toBeNull();
        expect((screen.getByRole('button', { name: `Elimină ${DOWNLOADS}` }) as HTMLButtonElement).disabled).toBe(false);
        expect((screen.getByRole('button', { name: 'Adaugă' }) as HTMLButtonElement).disabled).toBe(false);
    });

    it('shows invalid entries in English', async () => {
        await i18n.changeLanguage('en');
        try {
            config = { inbox_paths: ['C:\\Windows\\Temp'], pause_watchers: false };
            invalidPaths = [{ path: 'C:\\Windows\\Temp', code: 'config.excludedFolder' }];
            render(<SettingsView />);
            expect(await screen.findByText(/^Not watched: This folder is excluded by default/)).toBeTruthy();
            expect(screen.getByRole('button', { name: 'Remove the folders not watched' })).toBeTruthy();
        } finally {
            await i18n.changeLanguage('ro');
        }
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

const USB = 'E:\\Photos';
const UNAVAILABLE = 'Indisponibil acum (de exemplu, disc deconectat). Se reia automat când apare.';

describe('SettingsView unavailable folders', () => {
    it('marks unavailable folders and removes one without touching the others', async () => {
        config = { inbox_paths: [USB, DOWNLOADS], pause_watchers: false };
        unavailablePaths = [USB];
        await renderLoaded();
        const item = screen.getByText(USB).closest('li') as HTMLElement;
        expect(within(item).getByText(UNAVAILABLE)).toBeTruthy();
        expect(screen.getAllByText(UNAVAILABLE)).toHaveLength(1);
        // Not the remove-all flow of invalid entries: every entry stays editable.
        expect(screen.queryByRole('button', { name: 'Elimină folderele neurmărite' })).toBeNull();
        expect((screen.getByRole('button', { name: `Elimină ${DOWNLOADS}` }) as HTMLButtonElement).disabled).toBe(false);

        fireEvent.click(within(item).getByRole('button', { name: `Elimină ${USB}` }));
        await screen.findByText('Salvat.');
        expect(callsTo('save_config').at(-1)?.[1]).toEqual({
            config: { inbox_paths: [DOWNLOADS], pause_watchers: false },
        });
        expect(screen.queryByText(USB)).toBeNull();
        expect(screen.queryByText(UNAVAILABLE)).toBeNull();
    });

    it('saves a folder that does not exist and shows it as unavailable', async () => {
        await renderLoaded();
        unavailablePaths = [USB];
        addFolder(USB);
        const path = await screen.findByText(USB);
        expect(within(path.closest('li') as HTMLElement).getByText(UNAVAILABLE)).toBeTruthy();
        expect(config.inbox_paths).toEqual([DOWNLOADS, USB]);
    });

    it('keeps an unavailable folder marked while another one is added', async () => {
        config = { inbox_paths: [USB, DOWNLOADS], pause_watchers: false };
        unavailablePaths = [USB];
        await renderLoaded();
        addFolder('C:\\Users\\ana\\Desktop');
        await screen.findByText('C:\\Users\\ana\\Desktop');
        expect(screen.getAllByText(UNAVAILABLE)).toHaveLength(1);
        expect(within(screen.getByText(USB).closest('li') as HTMLElement).getByText(UNAVAILABLE)).toBeTruthy();
    });
});

function updatesSection() {
    return screen.getByRole('region', { name: 'Actualizări' });
}

describe('SettingsView updates', () => {
    it('shows the installed version, the channel and what goes over the network', async () => {
        await renderLoaded();
        const section = updatesSection();
        expect(within(section).getByText('Versiunea instalată: 0.3.0')).toBeTruthy();
        expect(within(section).getByText(/doar când apeși „Verifică acum” și o dată pe zi/)).toBeTruthy();
        expect((within(section).getByLabelText('Canal de actualizare') as HTMLSelectElement).value).toBe('stable');
        expect(within(section).getByRole('option', { name: 'Stabil' })).toBeTruthy();
        expect(within(section).getByRole('option', { name: 'Beta' })).toBeTruthy();
        // Nothing reaches the network until the user clicks.
        expect(callsTo('check_for_update')).toHaveLength(0);
        expect(within(section).queryByRole('button', { name: 'Instalează și repornește' })).toBeNull();
    });

    it('saves the channel', async () => {
        await renderLoaded();
        fireEvent.change(screen.getByLabelText('Canal de actualizare'), { target: { value: 'beta' } });
        await screen.findByText('Salvat.');
        expect(callsTo('save_app_settings').at(-1)?.[1]).toEqual({
            settings: { ...baseSettings(), update_channel: 'beta' },
        });
        expect((screen.getByLabelText('Canal de actualizare') as HTMLSelectElement).value).toBe('beta');
    });

    it('translates settings.invalidChannel', async () => {
        await renderLoaded();
        saveSettingsError = 'settings.invalidChannel';
        fireEvent.change(screen.getByLabelText('Canal de actualizare'), { target: { value: 'beta' } });
        expect(
            await screen.findByText('Canalul de actualizare nu e valid. Alege Stabil sau Beta.'),
        ).toBeTruthy();
        expect((screen.getByLabelText('Canal de actualizare') as HTMLSelectElement).value).toBe('stable');
    });

    it('checks on click and reports that there is no update', async () => {
        await renderLoaded();
        fireEvent.click(screen.getByRole('button', { name: 'Verifică acum' }));
        expect(await screen.findByText('Ai cea mai nouă versiune.')).toBeTruthy();
        expect(callsTo('check_for_update')).toHaveLength(1);
        expect(screen.queryByRole('button', { name: 'Instalează și repornește' })).toBeNull();
    });

    it('shows the version found with its date and notes as plain text', async () => {
        updateCheck = {
            available: true,
            version: '0.4.0',
            notes: 'Fixed: <b>undo</b> of copies.',
            date: '2026-10-29T10:00:00Z',
        };
        await renderLoaded();
        fireEvent.click(screen.getByRole('button', { name: 'Verifică acum' }));
        const section = updatesSection();
        expect(await within(section).findByText('Versiunea 0.4.0 e disponibilă.')).toBeTruthy();
        expect(within(section).getByText('Ce s-a schimbat')).toBeTruthy();
        expect(within(section).getByText('Fixed: <b>undo</b> of copies.')).toBeTruthy();
        expect(section.querySelector('b')).toBeNull();
        expect(within(section).getByText(/^Publicată: .*2026/)).toBeTruthy();
        expect(within(section).getByRole('button', { name: 'Instalează și repornește' })).toBeTruthy();
    });

    it('translates updater errors from the check', async () => {
        updateCheck = 'update.notConfigured';
        await renderLoaded();
        fireEvent.click(screen.getByRole('button', { name: 'Verifică acum' }));
        expect(await screen.findByText('Actualizările nu sunt configurate în această versiune.')).toBeTruthy();

        updateCheck = 'update.network';
        fireEvent.click(screen.getByRole('button', { name: 'Verifică acum' }));
        expect(
            await screen.findByText(
                'Serverul de actualizări nu a putut fi contactat. Verifică conexiunea la internet.',
            ),
        ).toBeTruthy();
    });

    it('installs only after the confirmation and shows the download progress', async () => {
        updateCheck = { available: true, version: '0.4.0' };
        await renderLoaded();
        fireEvent.click(screen.getByRole('button', { name: 'Verifică acum' }));
        fireEvent.click(await screen.findByRole('button', { name: 'Instalează și repornește' }));
        const dialog = screen.getByRole('alertdialog');
        expect(
            within(dialog).getByText(
                'Valet se va reporni. Nicio operație nu e întreruptă: instalarea așteaptă lotul în curs.',
            ),
        ).toBeTruthy();
        expect(callsTo('install_update')).toHaveLength(0);

        // Cancel, then confirm for real. install_update stays pending until progress arrives.
        fireEvent.click(within(dialog).getByRole('button', { name: 'Renunță' }));
        expect(screen.queryByRole('alertdialog')).toBeNull();
        expect(callsTo('install_update')).toHaveLength(0);

        let finish: () => void = () => {};
        invokeMock.mockImplementationOnce(
            () =>
                new Promise((resolve) => {
                    finish = () => resolve(undefined);
                }),
        );
        fireEvent.click(screen.getByRole('button', { name: 'Instalează și repornește' }));
        fireEvent.click(screen.getByRole('button', { name: 'Confirmă instalarea' }));
        expect(await screen.findByText('Se descarcă actualizarea...')).toBeTruthy();
        expect(callsTo('install_update')).toHaveLength(1);

        emit('update-progress', { downloaded: 1024 * 1024, total: 4 * 1024 * 1024 });
        const bar = screen.getByRole('progressbar', { name: 'Progresul descărcării' }) as HTMLProgressElement;
        expect(bar.max).toBe(4 * 1024 * 1024);
        expect(bar.value).toBe(1024 * 1024);
        expect(screen.getByText('Descărcat: 1 MB din 4 MB')).toBeTruthy();

        emit('update-progress', { downloaded: 2048, total: null });
        expect(screen.getByText('Descărcat: 2 KB')).toBeTruthy();

        await act(async () => finish());
        expect(await screen.findByText('Se repornește Valet...')).toBeTruthy();
    });

    it('waits for the running batch when the backend is busy, then installs', async () => {
        updateCheck = { available: true, version: '0.4.0' };
        installResults = ['update.busy'];
        await renderLoaded();
        fireEvent.click(screen.getByRole('button', { name: 'Verifică acum' }));
        fireEvent.click(await screen.findByRole('button', { name: 'Instalează și repornește' }));
        fireEvent.click(screen.getByRole('button', { name: 'Confirmă instalarea' }));
        expect(await screen.findByText('Aștept să se termine lotul în curs.')).toBeTruthy();
        expect(callsTo('install_update')).toHaveLength(1);

        await waitFor(() => expect(eventHandlers.has('batch-finished')).toBe(true));
        emit('batch-finished', { batch_id: 'b1', done: 1, failed: 0, auto: true });
        expect(await screen.findByText('Se repornește Valet...')).toBeTruthy();
        expect(callsTo('install_update')).toHaveLength(2);
    });

    it('shows a translated error when the install fails and offers it again', async () => {
        updateCheck = { available: true, version: '0.4.0' };
        installResults = ['update.failed'];
        await renderLoaded();
        fireEvent.click(screen.getByRole('button', { name: 'Verifică acum' }));
        fireEvent.click(await screen.findByRole('button', { name: 'Instalează și repornește' }));
        fireEvent.click(screen.getByRole('button', { name: 'Confirmă instalarea' }));
        expect(await screen.findByText('Actualizarea nu a reușit. Detaliile sunt în jurnal.')).toBeTruthy();
        expect(screen.getByRole('button', { name: 'Instalează și repornește' })).toBeTruthy();
    });

    it('offers the version announced by the daily check without checking again', async () => {
        render(<SettingsView announcedVersion="0.4.0" />);
        await screen.findByText(DOWNLOADS);
        expect(screen.getByText('Versiunea 0.4.0 e disponibilă.')).toBeTruthy();
        expect(screen.getByRole('button', { name: 'Instalează și repornește' })).toBeTruthy();
        expect(callsTo('check_for_update')).toHaveLength(0);
    });

    it('still loads when the version cannot be read', async () => {
        appVersion = new Error('no version');
        await renderLoaded();
        expect(screen.getByText('Versiunea instalată nu a putut fi citită.')).toBeTruthy();
        expect(screen.getByTestId('settings-about').textContent).toBe(
            'Valet · Licență MIT · github.com/OsztrovszkyVlad/Valet',
        );
    });

    it('shows the updates section in English', async () => {
        await i18n.changeLanguage('en');
        try {
            render(<SettingsView />);
            expect(await screen.findByText('Installed version: 0.3.0')).toBeTruthy();
            expect(screen.getByRole('option', { name: 'Stable' })).toBeTruthy();
            fireEvent.click(screen.getByRole('button', { name: 'Check now' }));
            expect(await screen.findByText('You have the newest version.')).toBeTruthy();
            expect(screen.getByTestId('settings-about').textContent).toBe(
                'Valet 0.3.0 · MIT license · github.com/OsztrovszkyVlad/Valet',
            );
        } finally {
            await i18n.changeLanguage('ro');
        }
    });
});

describe('SettingsView support and about', () => {
    it('saves the detailed log switch and says what it contains', async () => {
        await renderLoaded();
        const box = screen.getByLabelText('Jurnal detaliat (pentru depanare)') as HTMLInputElement;
        expect(box.checked).toBe(false);
        expect(
            screen.getByText(
                'Poate conține căi de fișiere. Rămâne pe calculatorul tău. Se aplică după repornire.',
            ),
        ).toBeTruthy();
        fireEvent.click(box);
        await screen.findByText('Salvat.');
        expect(callsTo('save_app_settings').at(-1)?.[1]).toEqual({
            settings: { ...baseSettings(), debug_logging: true },
        });
        expect(box.checked).toBe(true);
    });

    it('shows version, license and repository as plain text', async () => {
        await renderLoaded();
        expect(screen.getByRole('heading', { name: 'Despre' })).toBeTruthy();
        const about = screen.getByTestId('settings-about');
        expect(about.textContent).toBe('Valet 0.3.0 · Licență MIT · github.com/OsztrovszkyVlad/Valet');
        expect(about.querySelector('a')).toBeNull();
    });
});
