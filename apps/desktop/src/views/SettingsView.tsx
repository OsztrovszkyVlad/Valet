import React, { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
    configureAutoStart,
    errorMessage,
    getAppSettings,
    getAutoStart,
    getConfig,
    saveAppSettings,
    saveConfig,
    type AppSettings,
    type Config,
    type InvalidConfigPath,
    type Language,
} from '../api/commands';
import { commandErrorText } from '../i18n/format';
import './SettingsView.css';

type Status = 'idle' | 'saving' | 'saved';

const LANGUAGES: Language[] = ['ro', 'en'];

interface SettingsViewProps {
    /** Opens the Preview screen (offered after a folder is added). */
    onOpenPreview?: () => void;
}

/**
 * Secondary screen: language, watched folders, notifications, autostart, auto-apply. Each change saves at once.
 * Watched folders the backend reports as invalid are marked with the reason; the list can be changed only
 * after they are removed, because `save_config` refuses a list that contains one.
 */
const SettingsView: React.FC<SettingsViewProps> = ({ onOpenPreview }) => {
    const { t, i18n } = useTranslation();
    const [settings, setSettings] = useState<AppSettings | null>(null);
    const [config, setConfig] = useState<Config | null>(null);
    const [loadError, setLoadError] = useState('');
    const [error, setError] = useState('');
    const [status, setStatus] = useState<Status>('idle');
    const [newFolder, setNewFolder] = useState('');
    // Entries of a hand-edited config.json that are not watched.
    const [invalid, setInvalid] = useState<InvalidConfigPath[]>([]);
    // Shown after a folder is added: it needs an approved preview first.
    const [addedNotice, setAddedNotice] = useState(false);

    useEffect(() => {
        let cancelled = false;
        // The auto-start entry can change outside Valet (Task Manager); show its real state.
        const autoStart = getAutoStart().catch(() => null);
        Promise.all([getAppSettings(), getConfig(), autoStart])
            .then(([s, c, a]) => {
                if (!cancelled) {
                    setSettings(a === null ? s : { ...s, auto_start: a });
                    const { invalid_paths: invalidPaths, ...stored } = c;
                    setConfig(stored);
                    setInvalid(Array.isArray(invalidPaths) ? invalidPaths : []);
                }
            })
            .catch((err: unknown) => {
                if (!cancelled) {
                    setLoadError(t('settings.loadFailed', { message: errorMessage(err) }));
                }
            });
        return () => {
            cancelled = true;
        };
    }, [t]);

    if (loadError) {
        return (
            <div className="settings-view">
                <h2 className="view-title">{t('settings.title')}</h2>
                <div className="error-message" role="alert">
                    {loadError}
                </div>
            </div>
        );
    }

    if (settings === null || config === null) {
        return (
            <div className="settings-view">
                <h2 className="view-title">{t('settings.title')}</h2>
                <p>{t('common.loading')}</p>
            </div>
        );
    }

    const saving = status === 'saving';
    const hasInvalid = invalid.length > 0;
    const invalidCode = (path: string) => invalid.find((p) => p.path === path)?.code;

    const updateSettings = async (patch: Partial<AppSettings>): Promise<boolean> => {
        setError('');
        setStatus('saving');
        try {
            setSettings(await saveAppSettings({ ...settings, ...patch }));
            setStatus('saved');
            return true;
        } catch (err) {
            setError(commandErrorText(t, err));
            setStatus('idle');
            return false;
        }
    };

    const changeLanguage = async (language: Language) => {
        await i18n.changeLanguage(language);
        await updateSettings({ language });
    };

    const changeAutoStart = async (enabled: boolean) => {
        setError('');
        setStatus('saving');
        try {
            await configureAutoStart(enabled);
        } catch (err) {
            setError(commandErrorText(t, err));
            setStatus('idle');
            return;
        }
        await updateSettings({ auto_start: enabled });
    };

    const saveFolders = async (inboxPaths: string[]): Promise<boolean> => {
        setError('');
        setStatus('saving');
        try {
            setConfig(await saveConfig({ ...config, inbox_paths: inboxPaths }));
            // save_config accepts only lists without invalid entries.
            setInvalid([]);
            setStatus('saved');
            return true;
        } catch (err) {
            setError(commandErrorText(t, err));
            setStatus('idle');
            return false;
        }
    };

    const addFolder = async (e: React.FormEvent) => {
        e.preventDefault();
        const path = newFolder.trim();
        if (path === '') return;
        if (config.inbox_paths.some((p) => p.toLowerCase() === path.toLowerCase())) {
            setError(t('settings.folderDuplicate'));
            return;
        }
        setAddedNotice(false);
        if (await saveFolders([...config.inbox_paths, path])) {
            setNewFolder('');
            setAddedNotice(true);
        }
    };

    const removeFolder = (path: string) => saveFolders(config.inbox_paths.filter((p) => p !== path));

    const removeInvalid = () => saveFolders(config.inbox_paths.filter((p) => invalidCode(p) === undefined));

    return (
        <div className="settings-view">
            <div className="view-header">
                <h2 className="view-title">{t('settings.title')}</h2>
            </div>

            {error && (
                <div className="error-message" role="alert">
                    {error}
                    <button className="error-close" onClick={() => setError('')} aria-label={t('common.close')}>
                        ×
                    </button>
                </div>
            )}
            <p className="settings-status" role="status">
                {status === 'saving' ? t('common.saving') : status === 'saved' ? t('common.saved') : ''}
            </p>

            <section className="settings-section">
                <h3>{t('settings.foldersTitle')}</h3>
                <p className="form-help">{t('settings.foldersHelp')}</p>
                {addedNotice && (
                    <div className="notice-message settings-added-notice" role="note">
                        <span>{t('settings.newFolderNotice')}</span>
                        {onOpenPreview && (
                            <button className="btn-secondary" onClick={onOpenPreview}>
                                {t('settings.openPreview')}
                            </button>
                        )}
                        <button
                            className="error-close"
                            onClick={() => setAddedNotice(false)}
                            aria-label={t('common.close')}
                        >
                            ×
                        </button>
                    </div>
                )}
                {hasInvalid && (
                    <div className="settings-invalid-notice" role="note">
                        <span>{t('settings.invalidHelp')}</span>
                        <button className="btn-secondary" onClick={() => void removeInvalid()} disabled={saving}>
                            {t('settings.removeInvalid')}
                        </button>
                    </div>
                )}
                {config.inbox_paths.length === 0 ? (
                    <p>{t('settings.foldersEmpty')}</p>
                ) : (
                    <ul className="settings-folders">
                        {config.inbox_paths.map((path) => {
                            const code = invalidCode(path);
                            return (
                                <li key={path} className={code ? 'settings-folder-invalid' : undefined}>
                                    <div className="settings-folder-text">
                                        <code>{path}</code>
                                        {code && (
                                            <small className="settings-folder-reason">
                                                {t('settings.invalidReason', {
                                                    reason: t(`backendError.${code}`),
                                                })}
                                            </small>
                                        )}
                                    </div>
                                    {!code && (
                                        <button
                                            className="btn-secondary"
                                            onClick={() => void removeFolder(path)}
                                            disabled={saving || hasInvalid}
                                            aria-label={t('settings.removeFolderLabel', { path })}
                                        >
                                            {t('settings.removeFolder')}
                                        </button>
                                    )}
                                </li>
                            );
                        })}
                    </ul>
                )}
                <form className="settings-add-folder" onSubmit={(e) => void addFolder(e)}>
                    <label htmlFor="settings-new-folder">{t('settings.folderInput')}</label>
                    <input
                        id="settings-new-folder"
                        className="form-input"
                        type="text"
                        value={newFolder}
                        placeholder={t('settings.folderPlaceholder')}
                        onChange={(e) => setNewFolder(e.target.value)}
                    />
                    <button
                        className="btn-primary"
                        type="submit"
                        disabled={saving || hasInvalid || newFolder.trim() === ''}
                    >
                        {t('settings.addFolder')}
                    </button>
                </form>
            </section>

            <section className="settings-section">
                <h3>{t('settings.generalTitle')}</h3>

                <div className="form-group">
                    <label htmlFor="settings-language">{t('settings.language')}</label>
                    <select
                        id="settings-language"
                        className="form-input settings-select"
                        value={settings.language}
                        disabled={saving}
                        onChange={(e) => void changeLanguage(e.target.value as Language)}
                    >
                        {LANGUAGES.map((lng) => (
                            <option key={lng} value={lng}>
                                {lng === 'ro' ? t('settings.languageRo') : t('settings.languageEn')}
                            </option>
                        ))}
                    </select>
                </div>

                <div className="form-group">
                    <label className="checkbox-label">
                        <input
                            type="checkbox"
                            className="form-checkbox"
                            checked={settings.notifications_enabled}
                            disabled={saving}
                            onChange={(e) => void updateSettings({ notifications_enabled: e.target.checked })}
                        />
                        <span className="checkbox-text">{t('settings.notifications')}</span>
                    </label>
                </div>

                <div className="form-group">
                    <label className="checkbox-label">
                        <input
                            type="checkbox"
                            className="form-checkbox"
                            checked={settings.auto_start}
                            disabled={saving}
                            onChange={(e) => void changeAutoStart(e.target.checked)}
                        />
                        <span className="checkbox-text">{t('settings.autoStart')}</span>
                    </label>
                </div>

                <div className="form-group">
                    <label className="checkbox-label">
                        <input
                            type="checkbox"
                            className="form-checkbox"
                            checked={settings.auto_apply_enabled}
                            disabled={saving || !settings.first_preview_approved}
                            aria-describedby="settings-auto-apply-help"
                            onChange={(e) => void updateSettings({ auto_apply_enabled: e.target.checked })}
                        />
                        <span className="checkbox-text">{t('settings.autoApply')}</span>
                    </label>
                    <small id="settings-auto-apply-help" className="form-help">
                        {t('settings.autoApplyHelp')}
                        {!settings.first_preview_approved && <> {t('settings.autoApplyLocked')}</>}
                    </small>
                </div>
            </section>
        </div>
    );
};

export default SettingsView;
