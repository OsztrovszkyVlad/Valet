import React, { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
    deleteRule,
    errorMessage,
    getConfig,
    getRules,
    installPresets,
    listPresets,
    upsertRule,
    type Preset,
    type Rule,
} from '../api/commands';
import RuleEditor from '../components/RuleEditor';
import { commandErrorMessage, commandErrorText } from '../i18n/format';
import { newDraft, presetIdOf, ruleToDraft, type RuleDraft } from '../rules/model';
import { displayPath, ruleSummary } from '../rules/summary';
import './RulesView.css';

type Editing = { draft: RuleDraft; isNew: boolean } | null;

/** The "Add presets" panel: closed, or the shipped presets (null while loading) and the selection. */
type PresetPanel = { presets: Preset[] | null; error: string; selected: Set<string> } | null;

/** List of rules with on/off, summary, edit and delete. Rules never touch files by themselves. */
const RulesView: React.FC = () => {
    const { t, i18n } = useTranslation();
    const [rules, setRules] = useState<Rule[]>([]);
    const [folders, setFolders] = useState<string[]>([]);
    const [loading, setLoading] = useState(true);
    const [loadError, setLoadError] = useState('');
    const [error, setError] = useState('');
    const [notice, setNotice] = useState('');
    const [editing, setEditing] = useState<Editing>(null);
    const [confirmDelete, setConfirmDelete] = useState<string | null>(null);
    const [busy, setBusy] = useState(false);
    const [presetPanel, setPresetPanel] = useState<PresetPanel>(null);
    const [installing, setInstalling] = useState(false);

    const load = useCallback(async () => {
        setLoading(true);
        try {
            setRules(await getRules());
            setLoadError('');
        } catch (err) {
            setLoadError(t('rules.loadFailed', { message: commandErrorMessage(t, err) }));
        } finally {
            setLoading(false);
        }
        try {
            setFolders((await getConfig()).inbox_paths);
        } catch (err) {
            console.error('Failed to read config:', errorMessage(err));
        }
    }, [t]);

    useEffect(() => {
        void load();
    }, [load]);

    const openPresets = async () => {
        setNotice('');
        setError('');
        setPresetPanel({ presets: null, error: '', selected: new Set() });
        try {
            const list = [...(await listPresets())].sort((a, b) => a.displayOrder - b.displayOrder);
            setPresetPanel((p) => p && { ...p, presets: list });
        } catch (err) {
            setPresetPanel(
                (p) => p && { ...p, error: t('rules.presetsLoadFailed', { message: commandErrorMessage(t, err) }) },
            );
        }
    };

    const togglePreset = (id: string, on: boolean) =>
        setPresetPanel((p) => {
            if (p === null) return p;
            const selected = new Set(p.selected);
            if (on) selected.add(id);
            else selected.delete(id);
            return { ...p, selected };
        });

    const installSelected = async (ids: string[]) => {
        setError('');
        setNotice('');
        setInstalling(true);
        try {
            await installPresets(ids);
            setPresetPanel(null);
            setNotice(t('rules.presetsDone', { count: ids.length }));
        } catch (err) {
            setError(commandErrorText(t, err));
        } finally {
            setInstalling(false);
        }
        // Preset rules are kept after the user's rules; reload for the stored order.
        await load();
    };

    const startNew = () => {
        setNotice('');
        setError('');
        setEditing({ draft: newDraft(crypto.randomUUID()), isNew: true });
    };

    const startEdit = (rule: Rule) => {
        setNotice('');
        setError('');
        const draft = ruleToDraft(rule);
        if (draft === null) {
            setError(t('rules.notEditable'));
            return;
        }
        setEditing({ draft, isNew: false });
    };

    const onSaved = (saved: Rule, isNew: boolean) => {
        setRules((list) => (isNew ? [...list, saved] : list.map((r) => (r.id === saved.id ? saved : r))));
        setEditing(null);
        setNotice(isNew ? t('rules.savedNew') : t('rules.savedEdit'));
    };

    const toggle = async (rule: Rule, enabled: boolean) => {
        setError('');
        setNotice('');
        setBusy(true);
        try {
            const saved = await upsertRule({ ...rule, enabled });
            setRules((list) => list.map((r) => (r.id === saved.id ? saved : r)));
            if (enabled) setNotice(t('rules.savedEdit'));
        } catch (err) {
            setError(commandErrorText(t, err));
        } finally {
            setBusy(false);
        }
    };

    const remove = async (rule: Rule) => {
        setError('');
        setNotice('');
        setBusy(true);
        try {
            await deleteRule(rule.id);
            setRules((list) => list.filter((r) => r.id !== rule.id));
            setNotice(t('rules.deleted'));
        } catch (err) {
            setError(commandErrorText(t, err));
            if (err === 'rule.notFound') void load();
        } finally {
            setConfirmDelete(null);
            setBusy(false);
        }
    };

    const renderPresetPanel = (panel: NonNullable<PresetPanel>) => {
        const installed = new Set(rules.map(presetIdOf).filter((id): id is string => id !== null));
        const chosen = (panel.presets ?? []).filter((p) => panel.selected.has(p.id));
        const reinstall = chosen.filter((p) => installed.has(p.id));
        return (
            <section className="rules-presets" aria-labelledby="rules-presets-title" data-testid="rules-presets">
                <h3 id="rules-presets-title">{t('rules.presetsTitle')}</h3>
                <p className="view-description">{t('rules.presetsHelp')}</p>
                <p className="rules-presets-warning">{t('rules.presetsReinstallHelp')}</p>

                {panel.error && (
                    <div className="error-message" role="alert">
                        {panel.error}
                    </div>
                )}
                {panel.presets === null && !panel.error && <p>{t('common.loading')}</p>}
                {panel.presets !== null && panel.presets.length === 0 && <p>{t('rules.presetsEmpty')}</p>}

                {panel.presets !== null && panel.presets.length > 0 && (
                    <ul className="preset-list">
                        {panel.presets.map((p) => (
                            <li key={p.id} className="preset-item">
                                <label className="checkbox-label">
                                    <input
                                        type="checkbox"
                                        className="form-checkbox"
                                        data-testid={`rules-preset-${p.id}`}
                                        checked={panel.selected.has(p.id)}
                                        disabled={installing}
                                        onChange={(e) => togglePreset(p.id, e.target.checked)}
                                    />
                                    <span className="preset-name">{p.name}</span>
                                </label>
                                {installed.has(p.id) && (
                                    <span className="badge badge-installed">{t('rules.presetInstalled')}</span>
                                )}
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

                {reinstall.length > 0 && (
                    <p className="rules-presets-warning" role="status">
                        {t('rules.presetsReinstallList', { names: reinstall.map((p) => p.name).join(', ') })}
                    </p>
                )}
                {installing && <p role="status">{t('rules.presetsInstalling')}</p>}

                <div className="editor-buttons">
                    <button
                        className="btn-primary"
                        data-testid="rules-presets-install"
                        disabled={installing || chosen.length === 0}
                        onClick={() => void installSelected(chosen.map((p) => p.id))}
                    >
                        {t('rules.presetsInstall', { count: chosen.length })}
                    </button>
                    <button className="btn-secondary" disabled={installing} onClick={() => setPresetPanel(null)}>
                        {t('common.cancel')}
                    </button>
                </div>
            </section>
        );
    };

    if (editing !== null) {
        return (
            <div className="rules-view">
                <RuleEditor
                    initial={editing.draft}
                    isNew={editing.isNew}
                    folders={folders}
                    onSaved={onSaved}
                    onCancel={() => setEditing(null)}
                />
            </div>
        );
    }

    return (
        <div className="rules-view">
            <div className="view-header rules-header">
                <div>
                    <h2 className="view-title">{t('rules.title')}</h2>
                    <p className="view-description">{t('rules.description')}</p>
                </div>
                <div className="rules-header-buttons">
                    <button
                        className="btn-secondary"
                        data-testid="rules-presets-open"
                        disabled={presetPanel !== null}
                        onClick={() => void openPresets()}
                    >
                        {t('rules.presetsOpen')}
                    </button>
                    <button className="btn-primary" onClick={startNew}>
                        {t('rules.add')}
                    </button>
                </div>
            </div>

            {loadError && (
                <div className="error-message" role="alert">
                    {loadError}
                </div>
            )}
            {error && (
                <div className="error-message" role="alert">
                    {error}
                    <button className="error-close" onClick={() => setError('')} aria-label={t('common.close')}>
                        ×
                    </button>
                </div>
            )}
            {notice && (
                <div className="notice-message" role="status">
                    {notice}
                    <button className="error-close" onClick={() => setNotice('')} aria-label={t('common.close')}>
                        ×
                    </button>
                </div>
            )}

            {presetPanel !== null && renderPresetPanel(presetPanel)}

            {loading && <p>{t('common.loading')}</p>}

            {!loading && !loadError && rules.length === 0 && (
                <div className="empty-state">
                    <h3>{t('rules.emptyTitle')}</h3>
                    <p>{t('rules.emptyText')}</p>
                    <button className="btn-primary" onClick={startNew}>
                        {t('rules.add')}
                    </button>
                </div>
            )}

            {rules.length > 0 && (
                <ul className="rule-list">
                    {rules.map((rule) => (
                        <li key={rule.id} className={rule.enabled ? 'rule-card' : 'rule-card rule-off'}>
                            <label className="rule-toggle">
                                <input
                                    type="checkbox"
                                    checked={rule.enabled}
                                    disabled={busy}
                                    aria-label={t('rules.toggleLabel', { name: rule.name })}
                                    onChange={(e) => void toggle(rule, e.target.checked)}
                                />
                                <span aria-hidden="true">{t('rules.enabled')}</span>
                            </label>
                            <div className="rule-main">
                                <div className="rule-title">
                                    <span className="rule-name">{rule.name}</span>
                                    {presetIdOf(rule) !== null && <span className="rule-preset">{t('rules.preset')}</span>}
                                </div>
                                <p className="rule-summary">{ruleSummary(t, rule, i18n.language)}</p>
                                <p className="rule-scope">
                                    {rule.scope.length === 0
                                        ? t('rules.scopeAll')
                                        : t('rules.scopeIn', {
                                              folders: rule.scope.map((s) => displayPath(t, s)).join(', '),
                                          })}
                                </p>
                                {confirmDelete === rule.id && (
                                    <div className="rule-confirm" role="alertdialog" aria-label={t('rules.deleteYes')}>
                                        <p>{t('rules.deleteConfirm', { name: rule.name })}</p>
                                        <button className="btn-danger" disabled={busy} onClick={() => void remove(rule)}>
                                            {t('rules.deleteYes')}
                                        </button>
                                        <button
                                            className="btn-secondary"
                                            disabled={busy}
                                            onClick={() => setConfirmDelete(null)}
                                        >
                                            {t('common.cancel')}
                                        </button>
                                    </div>
                                )}
                            </div>
                            <div className="rule-buttons">
                                <button className="btn-secondary" disabled={busy} onClick={() => startEdit(rule)}>
                                    {t('rules.edit')}
                                </button>
                                <button
                                    className="btn-secondary"
                                    disabled={busy}
                                    onClick={() => setConfirmDelete(rule.id)}
                                >
                                    {t('rules.delete')}
                                </button>
                            </div>
                        </li>
                    ))}
                </ul>
            )}
        </div>
    );
};

export default RulesView;
