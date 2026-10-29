import React, { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
    checkForUpdate,
    errorMessage,
    installUpdate,
    onBatchFinished,
    onUpdateProgress,
    parseCommandError,
    UPDATE_CHANNELS,
    type UpdateChannel,
    type UpdateCheck,
    type UpdateProgress,
} from '../api/commands';
import { commandErrorText, formatBytes, formatDateTime } from '../i18n/format';

type CheckState =
    | { kind: 'idle' }
    | { kind: 'checking' }
    | { kind: 'done'; result: UpdateCheck }
    | { kind: 'error'; message: string };

/**
 * `confirm`: the user clicked install and sees what happens next.
 * `waiting`: the backend refused with `update.busy`; retried after the next `batch-finished`.
 * `restarting`: `install_update` returned; the backend restarts the app.
 */
type InstallState = 'idle' | 'confirm' | 'installing' | 'waiting' | 'restarting';

/**
 * While waiting for a batch, `install_update` is retried at this interval as well:
 * manual batches and undo send no `batch-finished`. A refused call costs no network.
 */
export const INSTALL_RETRY_MS = 3000;

interface UpdateSectionProps {
    /** Version of the running app; null when `get_app_version` failed. */
    version: string | null;
    channel: UpdateChannel;
    /** Settings are being saved; the channel select waits. */
    saving: boolean;
    onChannelChange: (channel: UpdateChannel) => void;
    /** Version announced by the backend's daily check (`update-available`). */
    announcedVersion?: string | null;
}

/**
 * Settings → Updates: installed version, channel, manual check and install.
 * Network use happens only on "Check now", "Install" or the backend's daily check.
 */
const UpdateSection: React.FC<UpdateSectionProps> = ({
    version,
    channel,
    saving,
    onChannelChange,
    announcedVersion = null,
}) => {
    const { t, i18n } = useTranslation();
    const [check, setCheck] = useState<CheckState>({ kind: 'idle' });
    const [install, setInstall] = useState<InstallState>('idle');
    const [installError, setInstallError] = useState('');
    const [progress, setProgress] = useState<UpdateProgress | null>(null);
    // The announced update was found on the previous channel.
    const [channelChanged, setChannelChanged] = useState(false);
    // One `install_update` call at a time.
    const inFlight = useRef(false);

    useEffect(() => {
        let unlisten: (() => void) | null = null;
        let stopped = false;
        onUpdateProgress(setProgress)
            .then((fn) => {
                if (stopped) fn();
                else unlisten = fn;
            })
            .catch((err) => console.error('Failed to listen for update progress:', errorMessage(err)));
        return () => {
            stopped = true;
            unlisten?.();
        };
    }, []);

    const runInstall = useCallback(async () => {
        if (inFlight.current) return;
        inFlight.current = true;
        setInstallError('');
        setProgress(null);
        setInstall('installing');
        try {
            await installUpdate();
            setInstall('restarting');
        } catch (err) {
            if (parseCommandError(err)?.code === 'update.busy') {
                setInstall('waiting');
            } else {
                setInstallError(commandErrorText(t, err));
                setInstall('idle');
            }
        } finally {
            inFlight.current = false;
        }
    }, [t]);

    useEffect(() => {
        // The backend refuses to install during a batch: try again once it finished.
        if (install !== 'waiting') return;
        let unlisten: (() => void) | null = null;
        let stopped = false;
        const retry = () => {
            if (!stopped) void runInstall();
        };
        const timer = setInterval(retry, INSTALL_RETRY_MS);
        onBatchFinished(retry)
            .then((fn) => {
                if (stopped) fn();
                else unlisten = fn;
            })
            .catch((err) => console.error('Failed to listen for batches:', errorMessage(err)));
        return () => {
            stopped = true;
            clearInterval(timer);
            unlisten?.();
        };
    }, [install, runInstall]);

    const runCheck = async () => {
        setInstallError('');
        setCheck({ kind: 'checking' });
        try {
            setCheck({ kind: 'done', result: await checkForUpdate() });
        } catch (err) {
            setCheck({ kind: 'error', message: commandErrorText(t, err) });
        }
    };

    const changeChannel = (next: UpdateChannel) => {
        // A result belongs to the channel it was checked on.
        setCheck({ kind: 'idle' });
        setChannelChanged(true);
        onChannelChange(next);
    };

    const result = check.kind === 'done' ? check.result : null;
    const announced = channelChanged ? null : announcedVersion;
    const availableVersion =
        result?.available === true ? (result.version ?? announced ?? '') : result === null ? announced : null;
    const busy = install !== 'idle' && install !== 'confirm';

    return (
        <section className="settings-section" aria-labelledby="settings-updates-title">
            <h3 id="settings-updates-title">{t('settings.updatesTitle')}</h3>
            <p data-testid="settings-version">
                {version !== null ? t('settings.currentVersion', { version }) : t('settings.versionUnknown')}
            </p>
            <p className="form-help">{t('settings.updatePrivacy')}</p>

            <div className="form-group">
                <label htmlFor="settings-update-channel">{t('settings.updateChannel')}</label>
                <select
                    id="settings-update-channel"
                    className="form-input settings-select"
                    value={channel}
                    disabled={saving || busy}
                    aria-describedby="settings-update-channel-help"
                    onChange={(e) => changeChannel(e.target.value as UpdateChannel)}
                >
                    {UPDATE_CHANNELS.map((c) => (
                        <option key={c} value={c}>
                            {c === 'stable' ? t('settings.channelStable') : t('settings.channelBeta')}
                        </option>
                    ))}
                </select>
                <small id="settings-update-channel-help" className="form-help">
                    {t('settings.channelHelp')}
                </small>
            </div>

            <div className="settings-update-actions">
                <button
                    className="btn-secondary"
                    onClick={() => void runCheck()}
                    disabled={check.kind === 'checking' || busy}
                >
                    {check.kind === 'checking' ? t('settings.checking') : t('settings.checkNow')}
                </button>
            </div>

            <div role="status" className="settings-update-result">
                {check.kind === 'error' && <p className="settings-update-error">{check.message}</p>}
                {result !== null && !result.available && <p>{t('settings.upToDate')}</p>}
                {availableVersion !== null && (
                    <div className="settings-update-found">
                        <p>
                            <strong>{t('settings.updateFound', { version: availableVersion })}</strong>
                        </p>
                        {result?.date && (
                            <p className="form-help">
                                {t('settings.updateDate', { date: formatDateTime(result.date, i18n.language) })}
                            </p>
                        )}
                        {result?.notes && (
                            <div className="settings-update-notes">
                                <h4>{t('settings.updateNotes')}</h4>
                                {/* Plain text: release notes are never rendered as HTML. */}
                                <p>{result.notes}</p>
                            </div>
                        )}
                    </div>
                )}
            </div>

            {availableVersion !== null && install === 'idle' && (
                <button className="btn-primary" onClick={() => setInstall('confirm')}>
                    {t('settings.install')}
                </button>
            )}

            {install === 'confirm' && (
                <div
                    className="notice-message settings-update-confirm"
                    role="alertdialog"
                    aria-labelledby="settings-update-confirm-text"
                >
                    <span id="settings-update-confirm-text">{t('settings.installConfirm')}</span>
                    <button className="btn-primary" onClick={() => void runInstall()}>
                        {t('settings.installConfirmButton')}
                    </button>
                    <button className="btn-secondary" onClick={() => setInstall('idle')}>
                        {t('common.cancel')}
                    </button>
                </div>
            )}

            {install === 'installing' && (
                <div className="settings-update-progress">
                    <p>{t('settings.installing')}</p>
                    {progress !== null && progress.total !== null && progress.total > 0 ? (
                        <progress
                            aria-label={t('settings.progressLabel')}
                            max={progress.total}
                            value={Math.min(progress.downloaded, progress.total)}
                        />
                    ) : (
                        <progress aria-label={t('settings.progressLabel')} />
                    )}
                    {progress !== null && (
                        <p className="form-help">
                            {progress.total !== null
                                ? t('settings.progress', {
                                      downloaded: formatBytes(progress.downloaded, i18n.language),
                                      total: formatBytes(progress.total, i18n.language),
                                  })
                                : t('settings.progressUnknown', {
                                      downloaded: formatBytes(progress.downloaded, i18n.language),
                                  })}
                        </p>
                    )}
                </div>
            )}

            {install === 'waiting' && (
                <div className="notice-message" role="status">
                    <span>{t('backendError.update.busy')}</span>
                    <button className="btn-secondary" onClick={() => setInstall('idle')}>
                        {t('common.cancel')}
                    </button>
                </div>
            )}

            {install === 'restarting' && <p role="status">{t('settings.restarting')}</p>}

            {installError && (
                <div className="error-message" role="alert">
                    {installError}
                </div>
            )}
        </section>
    );
};

export default UpdateSection;
