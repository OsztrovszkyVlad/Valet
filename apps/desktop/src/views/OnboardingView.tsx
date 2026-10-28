import React, { useCallback, useEffect, useId, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
    completeOnboarding,
    dryRun,
    listPresets,
    parseCommandError,
    type Preset,
} from '../api/commands';
import { commandErrorMessage, commandErrorText } from '../i18n/format';
import { displayPath } from '../rules/summary';
import type { InitialPreview } from './DryRunView';
import './OnboardingView.css';

/** Folders offered on step 1, as tokens the backend expands. */
const KNOWN_FOLDERS: { token: string; checked: boolean }[] = [
    { token: '{downloads}', checked: true },
    { token: '{desktop}', checked: true },
    { token: '{documents}', checked: false },
    { token: '{pictures}', checked: false },
];

interface OnboardingViewProps {
    /** Onboarding is saved; `result` is the first preview (or why it failed). */
    onFinished: (result: InitialPreview) => void;
}

/**
 * First run, PLAN 2.3: (1) folders, (2) presets, then (3) the preview,
 * shown by the Preview screen. Nothing moves here: the last step only
 * saves the choices and runs a preview.
 */
const OnboardingView: React.FC<OnboardingViewProps> = ({ onFinished }) => {
    const { t } = useTranslation();
    const uid = useId();
    const [step, setStep] = useState<1 | 2>(1);
    const [checked, setChecked] = useState<Record<string, boolean>>(() =>
        Object.fromEntries(KNOWN_FOLDERS.map((f) => [f.token, f.checked])),
    );
    const [custom, setCustom] = useState<string[]>([]);
    const [newFolder, setNewFolder] = useState('');
    const [folderError, setFolderError] = useState('');

    const [presets, setPresets] = useState<Preset[] | null>(null);
    const [presetsError, setPresetsError] = useState('');
    const [selected, setSelected] = useState<Set<string>>(new Set());
    const [finishing, setFinishing] = useState(false);
    const [finishError, setFinishError] = useState('');

    const folders = [...KNOWN_FOLDERS.map((f) => f.token).filter((tok) => checked[tok]), ...custom];

    const loadPresets = useCallback(async () => {
        setPresetsError('');
        try {
            const list = [...(await listPresets())].sort((a, b) => a.displayOrder - b.displayOrder);
            setPresets(list);
            setSelected(new Set(list.filter((p) => p.defaultChecked).map((p) => p.id)));
        } catch (err) {
            setPresetsError(t('onboarding.presetsLoadFailed', { message: commandErrorMessage(t, err) }));
        }
    }, [t]);

    useEffect(() => {
        if (step === 2 && presets === null) void loadPresets();
    }, [step, presets, loadPresets]);

    const addFolder = (e: React.FormEvent) => {
        e.preventDefault();
        const path = newFolder.trim();
        if (path === '') return;
        if (custom.some((p) => p.toLowerCase() === path.toLowerCase())) {
            setFolderError(t('onboarding.folderDuplicate'));
            return;
        }
        setFolderError('');
        setCustom([...custom, path]);
        setNewFolder('');
    };

    const togglePreset = (id: string, on: boolean) =>
        setSelected((s) => {
            const next = new Set(s);
            if (on) next.add(id);
            else next.delete(id);
            return next;
        });

    const finish = async (presetIds: string[]) => {
        setFinishing(true);
        setFinishError('');
        try {
            await completeOnboarding(folders, presetIds);
        } catch (err) {
            setFinishing(false);
            const parsed = parseCommandError(err);
            if (parsed?.code.startsWith('config.')) {
                // A folder cannot be watched: back to step 1 with the reason,
                // naming a folder token (`{pictures}`) as the UI does.
                const detail = parsed.detail === null ? null : displayPath(t, parsed.detail);
                setFolderError(commandErrorText(t, detail === null ? parsed.code : `${parsed.code}: ${detail}`));
                setStep(1);
            } else {
                setFinishError(commandErrorText(t, err));
            }
            return;
        }
        let result: InitialPreview;
        try {
            result = { preview: await dryRun() };
        } catch (err) {
            result = { error: err };
        }
        onFinished(result);
    };

    const orderedPresetIds = () => (presets ?? []).filter((p) => selected.has(p.id)).map((p) => p.id);

    return (
        <div className="onboarding">
            <h1 className="onboarding-app">{t('app.title')}</h1>
            <p className="onboarding-step">{t('onboarding.stepOf', { step })}</p>

            {step === 1 && (
                <section aria-labelledby={`${uid}-folders`}>
                    <h2 id={`${uid}-folders`} className="view-title">
                        {t('onboarding.foldersTitle')}
                    </h2>
                    <p className="view-description">{t('onboarding.foldersHelp')}</p>

                    {folderError && (
                        <div className="error-message" role="alert">
                            {folderError}
                        </div>
                    )}

                    <div className="onboarding-folders">
                        {KNOWN_FOLDERS.map(({ token }) => (
                            <label key={token} className="checkbox-label">
                                <input
                                    type="checkbox"
                                    className="form-checkbox"
                                    data-testid={`folder-${token.replace(/[{}]/g, '')}`}
                                    checked={checked[token]}
                                    onChange={(e) => setChecked({ ...checked, [token]: e.target.checked })}
                                />
                                <span className="checkbox-text">{displayPath(t, token)}</span>
                            </label>
                        ))}
                        {custom.map((path) => (
                            <label key={path} className="checkbox-label">
                                <input
                                    type="checkbox"
                                    className="form-checkbox"
                                    checked
                                    onChange={() => setCustom(custom.filter((p) => p !== path))}
                                />
                                <span className="checkbox-text">{path}</span>
                            </label>
                        ))}
                    </div>

                    <form className="onboarding-add" onSubmit={addFolder}>
                        <label htmlFor={`${uid}-new`}>{t('onboarding.otherFolder')}</label>
                        <div className="onboarding-add-row">
                            <input
                                id={`${uid}-new`}
                                className="form-input"
                                type="text"
                                value={newFolder}
                                placeholder={t('onboarding.otherFolderPlaceholder')}
                                onChange={(e) => setNewFolder(e.target.value)}
                            />
                            <button type="submit" className="btn-secondary" disabled={newFolder.trim() === ''}>
                                {t('onboarding.addFolder')}
                            </button>
                        </div>
                    </form>

                    {folders.length === 0 && <p className="field-error">{t('onboarding.foldersNone')}</p>}

                    <div className="onboarding-buttons">
                        <button
                            className="btn-primary"
                            data-testid="onboarding-next"
                            disabled={folders.length === 0}
                            onClick={() => {
                                setFolderError('');
                                setStep(2);
                            }}
                        >
                            {t('onboarding.next')}
                        </button>
                    </div>
                </section>
            )}

            {step === 2 && (
                <section aria-labelledby={`${uid}-presets`}>
                    <h2 id={`${uid}-presets`} className="view-title">
                        {t('onboarding.presetsTitle')}
                    </h2>
                    <p className="view-description">{t('onboarding.presetsHelp')}</p>

                    {presetsError && (
                        <div className="error-message" role="alert">
                            {presetsError}
                            <button className="btn-secondary" onClick={() => void loadPresets()}>
                                {t('onboarding.retry')}
                            </button>
                        </div>
                    )}
                    {presets === null && !presetsError && <p>{t('common.loading')}</p>}
                    {presets !== null && presets.length === 0 && <p>{t('onboarding.presetsEmpty')}</p>}

                    {presets !== null && presets.length > 0 && (
                        <ul className="preset-list">
                            {presets.map((p) => (
                                <li key={p.id} className="preset-item">
                                    <label className="checkbox-label">
                                        <input
                                            type="checkbox"
                                            className="form-checkbox"
                                            data-testid={`preset-${p.id}`}
                                            checked={selected.has(p.id)}
                                            onChange={(e) => togglePreset(p.id, e.target.checked)}
                                        />
                                        <span className="preset-name">{p.name}</span>
                                    </label>
                                    {p.actions.includes('trash') && (
                                        <span className="badge badge-trash">{t('onboarding.badgeTrash')}</span>
                                    )}
                                    {p.actions.includes('report') && (
                                        <span className="badge badge-report">{t('onboarding.badgeReport')}</span>
                                    )}
                                    <p className="preset-description">{p.description}</p>
                                    <p className="preset-meta">{t('onboarding.ruleCount', { count: p.ruleCount })}</p>
                                </li>
                            ))}
                        </ul>
                    )}

                    {finishError && (
                        <div className="error-message" role="alert">
                            {finishError}
                        </div>
                    )}
                    {finishing && <p role="status">{t('onboarding.finishing')}</p>}

                    <div className="onboarding-buttons">
                        <button className="btn-secondary" disabled={finishing} onClick={() => setStep(1)}>
                            {t('onboarding.back')}
                        </button>
                        <button className="btn-secondary" disabled={finishing} onClick={() => void finish([])}>
                            {t('onboarding.skip')}
                        </button>
                        <button
                            className="btn-primary"
                            data-testid="onboarding-finish"
                            disabled={finishing || presets === null}
                            onClick={() => void finish(orderedPresetIds())}
                        >
                            {t('onboarding.next')}
                        </button>
                    </div>
                </section>
            )}
        </div>
    );
};

export default OnboardingView;
