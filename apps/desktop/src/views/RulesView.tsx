import React, { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { deleteRule, errorMessage, getConfig, getRules, upsertRule, type Rule } from '../api/commands';
import RuleEditor from '../components/RuleEditor';
import { commandErrorMessage, commandErrorText } from '../i18n/format';
import { newDraft, presetIdOf, ruleToDraft, type RuleDraft } from '../rules/model';
import { displayPath, ruleSummary } from '../rules/summary';
import './RulesView.css';

type Editing = { draft: RuleDraft; isNew: boolean } | null;

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
                <button className="btn-primary" onClick={startNew}>
                    {t('rules.add')}
                </button>
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
