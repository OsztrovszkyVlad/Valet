import React, { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
    configureAutoStart,
    errorMessage,
    getAppSettings,
    getAppVersion,
    getAutoStart,
    getConfig,
    openExternal,
    planLimitOf,
    saveAppSettings,
    saveConfig,
    type AppSettings,
    type Config,
    type ConfigView,
    type InvalidConfigPath,
    type Language,
    type PlanLimitError,
    type PlanView,
    type UpdateChannel,
} from '../api/commands';
import PlanLimitNotice from '../components/PlanLimitNotice';
import ProSection from '../components/ProSection';
import UpdateSection from '../components/UpdateSection';
import { commandErrorText } from '../i18n/format';
import './SettingsView.css';

type Status = 'idle' | 'saving' | 'saved';

const LANGUAGES: Language[] = ['ro', 'en'];

/** The stored config and the entries `get_config` reports as invalid, unavailable or inactive. */
function splitConfigView(c: ConfigView): {
    config: Config;
    invalid: InvalidConfigPath[];
    unavailable: string[];
    inactive: string[];
} {
    const {
        invalid_paths: invalidPaths,
        unavailable_paths: unavailablePaths,
        inactive_paths: inactivePaths,
        ...config
    } = c;
    return {
        config,
        invalid: Array.isArray(invalidPaths) ? invalidPaths : [],
        unavailable: Array.isArray(unavailablePaths) ? unavailablePaths : [],
        inactive: Array.isArray(inactivePaths) ? inactivePaths : [],
    };
}

interface SettingsViewProps {
    /** Opens the Preview screen (offered after a folder is added). */
    onOpenPreview?: () => void;
    /** Version announced by the backend's daily update check, if any. */
    announcedVersion?: string | null;
    /** The plan; null or absent while unknown (no Pro section, no plan notes). */
    plan?: PlanView | null;
    /** A license command returned a new plan. */
    onPlanChange?: (plan: PlanView) => void;
    /** Changes when the Valet Pro section should be scrolled into view. */
    proFocusToken?: number;
}

/**
 * Secondary screen: watched folders, general options, updates, support and about. Each change saves at once.
 * Watched folders the backend reports as invalid are marked with the reason; the list can be changed only
 * after they are removed, because `save_config` refuses a list that contains one. Unavailable folders
 * (missing right now) are marked too and stay removable one by one.
 */
const SettingsView: React.FC<SettingsViewProps> = ({
    onOpenPreview,
    announcedVersion = null,
    plan = null,
    onPlanChange,
    proFocusToken = 0,
}) => {
    const { t, i18n } = useTranslation();
    const [settings, setSettings] = useState<AppSettings | null>(null);
    const [config, setConfig] = useState<Config | null>(null);
    const [loadError, setLoadError] = useState('');
    const [error, setError] = useState('');
    const [status, setStatus] = useState<Status>('idle');
    const [newFolder, setNewFolder] = useState('');
    // Entries of a hand-edited config.json that are not watched.
    const [invalid, setInvalid] = useState<InvalidConfigPath[]>([]);
    // Valid entries that do not exist right now (e.g. an unplugged drive).
    const [unavailable, setUnavailable] = useState<string[]>([]);
    // Null when `get_app_version` failed.
    const [version, setVersion] = useState<string | null>(null);
    // Shown after a folder is added: it needs an approved preview first.
    const [addedNotice, setAddedNotice] = useState(false);
    // Free plan: folders beyond the first 2, kept but not watched.
    const [inactive, setInactive] = useState<string[]>([]);
    // A folder change refused by the Free plan limit.
    const [limitError, setLimitError] = useState<PlanLimitError | null>(null);
    const proRef = useRef<HTMLElement>(null);

    const applyConfigView = (c: ConfigView) => {
        const split = splitConfigView(c);
        setConfig(split.config);
        setInvalid(split.invalid);
        setUnavailable(split.unavailable);
        setInactive(split.inactive);
    };

    useEffect(() => {
        let cancelled = false;
        // The auto-start entry can change outside Valet (Task Manager); show its real state.
        const autoStart = getAutoStart().catch(() => null);
        const appVersion = getAppVersion().catch(() => null);
        Promise.all([getAppSettings(), getConfig(), autoStart, appVersion])
            .then(([s, c, a, v]) => {
                if (!cancelled) {
                    setSettings(a === null ? s : { ...s, auto_start: a });
                    const split = splitConfigView(c);
                    setConfig(split.config);
                    setInvalid(split.invalid);
                    setUnavailable(split.unavailable);
                    setInactive(split.inactive);
                    setVersion(v);
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

    // A plan change (choice dialog, license) can reorder or (de)activate folders.
    const planSeen = useRef(plan);
    useEffect(() => {
        if (planSeen.current === plan) return;
        planSeen.current = plan;
        let cancelled = false;
        getConfig()
            .then((c) => {
                if (cancelled) return;
                const split = splitConfigView(c);
                setConfig(split.config);
                setInvalid(split.invalid);
                setUnavailable(split.unavailable);
                setInactive(split.inactive);
            })
            .catch((err: unknown) => console.error('Failed to read config:', errorMessage(err)));
        return () => {
            cancelled = true;
        };
    }, [plan]);

    const loaded = settings !== null && config !== null;
    useEffect(() => {
        if (proFocusToken === 0 || !loaded) return;
        const el = proRef.current;
        el?.scrollIntoView?.({ block: 'start' });
        el?.focus();
    }, [proFocusToken, loaded]);

    const focusPro = () => {
        proRef.current?.scrollIntoView?.({ block: 'start' });
        proRef.current?.focus();
    };

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
        setLimitError(null);
        setStatus('saving');
        try {
            const saved = await saveConfig({ ...config, inbox_paths: inboxPaths });
            try {
                // Which of the saved folders exist right now.
                applyConfigView(await getConfig());
            } catch {
                setConfig(saved);
                // save_config accepts only lists without invalid entries.
                setInvalid([]);
                setUnavailable((prev) => prev.filter((p) => saved.inbox_paths.includes(p)));
                setInactive((prev) => prev.filter((p) => saved.inbox_paths.includes(p)));
            }
            setStatus('saved');
            return true;
        } catch (err) {
            const limit = planLimitOf(err);
            if (limit !== null) setLimitError(limit);
            else setError(commandErrorText(t, err));
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

    const isUnavailable = (path: string) => unavailable.includes(path);
    const isInactive = (path: string) => inactive.includes(path);
    const packaged = plan?.packaged === true;

    const openStartupSettings = async () => {
        setError('');
        try {
            await openExternal('startup_settings');
        } catch (err) {
            setError(commandErrorText(t, err));
        }
    };

    const changeChannel = (update_channel: UpdateChannel) => void updateSettings({ update_channel });

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
                {limitError && (
                    <PlanLimitNotice
                        limit={limitError}
                        onOpenPro={plan !== null && onPlanChange ? focusPro : undefined}
                        onClose={() => setLimitError(null)}
                    />
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
                            const away = !code && isUnavailable(path);
                            const off = !code && !away && isInactive(path);
                            return (
                                <li
                                    key={path}
                                    className={
                                        code
                                            ? 'settings-folder-invalid'
                                            : away || off
                                              ? 'settings-folder-unavailable'
                                              : undefined
                                    }
                                >
                                    <div className="settings-folder-text">
                                        <code>{path}</code>
                                        {code && (
                                            <small className="settings-folder-reason">
                                                {t('settings.invalidReason', {
                                                    reason: t(`backendError.${code}`),
                                                })}
                                            </small>
                                        )}
                                        {away && (
                                            <small className="settings-folder-reason">
                                                {t('settings.unavailableReason')}
                                            </small>
                                        )}
                                        {off && (
                                            <small className="settings-folder-reason" data-testid="folder-inactive">
                                                {t('settings.inactiveReason')}
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

                {packaged ? (
                    <div className="form-group" data-testid="settings-autostart-store">
                        <p className="form-help">{t('settings.autoStartStore')}</p>
                        <button className="btn-secondary" onClick={() => void openStartupSettings()}>
                            {t('settings.openStartupSettings')}
                        </button>
                    </div>
                ) : (
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
                )}

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

            {plan !== null && onPlanChange && (
                <ProSection plan={plan} onPlanChange={onPlanChange} sectionRef={proRef} />
            )}

            {packaged ? (
                <section className="settings-section" data-testid="settings-updates-store">
                    <h3>{t('settings.updatesTitle')}</h3>
                    <p data-testid="settings-version">
                        {version !== null ? t('settings.currentVersion', { version }) : t('settings.versionUnknown')}
                    </p>
                    <p>{t('settings.updatesStore')}</p>
                </section>
            ) : (
                <UpdateSection
                    version={version}
                    channel={settings.update_channel}
                    saving={saving}
                    onChannelChange={changeChannel}
                    announcedVersion={announcedVersion}
                    autoCheck={settings.auto_update_check !== false}
                    onAutoCheckChange={(on) => void updateSettings({ auto_update_check: on })}
                    licenseUntil={plan?.plan.kind === 'pro' ? plan.plan.updates_until : null}
                />
            )}

            <section className="settings-section">
                <h3>{t('settings.supportTitle')}</h3>
                <div className="form-group">
                    <label className="checkbox-label">
                        <input
                            type="checkbox"
                            className="form-checkbox"
                            checked={settings.debug_logging}
                            disabled={saving}
                            aria-describedby="settings-debug-logging-help"
                            onChange={(e) => void updateSettings({ debug_logging: e.target.checked })}
                        />
                        <span className="checkbox-text">{t('settings.debugLogging')}</span>
                    </label>
                    <small id="settings-debug-logging-help" className="form-help">
                        {t('settings.debugLoggingHelp')}
                    </small>
                </div>
            </section>

            <section className="settings-section">
                <h3>{t('settings.aboutTitle')}</h3>
                <p className="settings-about" data-testid="settings-about">
                    {version !== null ? t('settings.aboutLine', { version }) : t('settings.aboutLineNoVersion')}
                </p>
            </section>
        </div>
    );
};

export default SettingsView;
