// Temporary (Phase 1 form): it sends a rule shape the backend does not accept.
// The F1 Rules editor replaces this file.
import React, { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
    errorMessage,
    getRulesLegacy,
    upsertRuleLegacy,
    type LegacyRule,
} from '../api/commands';
import './RulesView.css';

interface RuleForm {
    name: string;
    pattern: string;
    destination: string;
    enabled: boolean;
}

const EMPTY_FORM: RuleForm = { name: '', pattern: '', destination: '', enabled: true };

const RulesView: React.FC = () => {
    const { t } = useTranslation();
    const [rules, setRules] = useState<LegacyRule[]>([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    const [showForm, setShowForm] = useState(false);
    const [editingRule, setEditingRule] = useState<LegacyRule | null>(null);
    const [formData, setFormData] = useState<RuleForm>(EMPTY_FORM);

    const loadRules = useCallback(async () => {
        try {
            setLoading(true);
            setRules(await getRulesLegacy());
            setError(null);
        } catch (err) {
            setError(t('rulesLegacy.loadFailed', { message: errorMessage(err) }));
        } finally {
            setLoading(false);
        }
    }, [t]);

    useEffect(() => {
        void loadRules();
    }, [loadRules]);

    const closeForm = () => {
        setShowForm(false);
        setEditingRule(null);
        setFormData(EMPTY_FORM);
    };

    const saveRule = async () => {
        try {
            await upsertRuleLegacy({ ...formData, id: editingRule?.id });
            await loadRules();
            closeForm();
            setError(null);
        } catch (err) {
            setError(t('rulesLegacy.saveFailed', { message: errorMessage(err) }));
        }
    };

    const openNewForm = () => {
        setFormData(EMPTY_FORM);
        setEditingRule(null);
        setShowForm(true);
    };

    const openEditForm = (rule: LegacyRule) => {
        setFormData({
            name: rule.name,
            pattern: rule.pattern,
            destination: rule.destination,
            enabled: rule.enabled,
        });
        setEditingRule(rule);
        setShowForm(true);
    };

    const toggleRule = async (rule: LegacyRule) => {
        try {
            await upsertRuleLegacy({ ...rule, enabled: !rule.enabled });
            await loadRules();
        } catch (err) {
            setError(t('rulesLegacy.saveFailed', { message: errorMessage(err) }));
        }
    };

    return (
        <div className="rules-view">
            <div className="view-header">
                <div className="header-left">
                    <h2 className="view-title">{t('rulesLegacy.title')}</h2>
                    <p className="view-description">{t('rulesLegacy.description')}</p>
                </div>
                <div className="header-actions">
                    <button className="btn-primary" onClick={openNewForm} disabled={loading}>
                        {t('rulesLegacy.add')}
                    </button>
                    <button className="btn-secondary" onClick={() => void loadRules()} disabled={loading}>
                        {t('common.refresh')}
                    </button>
                </div>
            </div>

            {error && (
                <div className="error-message" role="alert">
                    {error}
                    <button className="error-close" onClick={() => setError(null)} aria-label={t('common.close')}>
                        ×
                    </button>
                </div>
            )}

            <div className="view-content">
                {loading ? (
                    <div className="loading-state">
                        <p>{t('common.loading')}</p>
                    </div>
                ) : rules.length === 0 ? (
                    <div className="empty-state">
                        <h3>{t('rulesLegacy.emptyTitle')}</h3>
                        <p>{t('rulesLegacy.emptyText')}</p>
                        <button className="btn-primary" onClick={openNewForm}>
                            {t('rulesLegacy.add')}
                        </button>
                    </div>
                ) : (
                    <div className="rules-table-container">
                        <table className="rules-table">
                            <thead>
                                <tr>
                                    <th>{t('rulesLegacy.colStatus')}</th>
                                    <th>{t('rulesLegacy.colName')}</th>
                                    <th>{t('rulesLegacy.colPattern')}</th>
                                    <th>{t('rulesLegacy.colDestination')}</th>
                                    <th>{t('rulesLegacy.colCreated')}</th>
                                    <th>{t('rulesLegacy.colActions')}</th>
                                </tr>
                            </thead>
                            <tbody>
                                {rules.map((rule, index) => (
                                    <tr
                                        key={rule.id ?? index}
                                        className={rule.enabled ? 'rule-enabled' : 'rule-disabled'}
                                    >
                                        <td>
                                            <button
                                                className={`toggle-btn ${rule.enabled ? 'enabled' : 'disabled'}`}
                                                onClick={() => void toggleRule(rule)}
                                                title={rule.enabled ? t('rulesLegacy.disable') : t('rulesLegacy.enable')}
                                            >
                                                {rule.enabled ? t('rulesLegacy.on') : t('rulesLegacy.off')}
                                            </button>
                                        </td>
                                        <td className="rule-name">{rule.name}</td>
                                        <td className="rule-pattern">
                                            <code>{rule.pattern}</code>
                                        </td>
                                        <td className="rule-destination">
                                            <span className="path">{rule.destination}</span>
                                        </td>
                                        <td className="rule-date">
                                            {rule.created_at ? new Date(rule.created_at).toLocaleDateString() : ''}
                                        </td>
                                        <td className="rule-actions">
                                            <button className="btn-small btn-edit" onClick={() => openEditForm(rule)}>
                                                {t('rulesLegacy.edit')}
                                            </button>
                                        </td>
                                    </tr>
                                ))}
                            </tbody>
                        </table>
                    </div>
                )}
            </div>

            {showForm && (
                <div className="modal-overlay">
                    <div className="modal-content" role="dialog" aria-modal="true">
                        <div className="modal-header">
                            <h3>{editingRule ? t('rulesLegacy.formEditTitle') : t('rulesLegacy.formNewTitle')}</h3>
                            <button className="modal-close" onClick={closeForm} aria-label={t('common.close')}>
                                ×
                            </button>
                        </div>

                        <div className="modal-body">
                            <div className="form-group">
                                <label htmlFor="rule-name">{t('rulesLegacy.name')}</label>
                                <input
                                    id="rule-name"
                                    type="text"
                                    value={formData.name}
                                    onChange={(e) => setFormData({ ...formData, name: e.target.value })}
                                    placeholder={t('rulesLegacy.namePlaceholder')}
                                    className="form-input"
                                />
                            </div>

                            <div className="form-group">
                                <label htmlFor="rule-pattern">{t('rulesLegacy.pattern')}</label>
                                <input
                                    id="rule-pattern"
                                    type="text"
                                    value={formData.pattern}
                                    onChange={(e) => setFormData({ ...formData, pattern: e.target.value })}
                                    placeholder={t('rulesLegacy.patternPlaceholder')}
                                    className="form-input"
                                />
                                <small className="form-help">{t('rulesLegacy.patternHelp')}</small>
                            </div>

                            <div className="form-group">
                                <label htmlFor="rule-destination">{t('rulesLegacy.destination')}</label>
                                <input
                                    id="rule-destination"
                                    type="text"
                                    value={formData.destination}
                                    onChange={(e) => setFormData({ ...formData, destination: e.target.value })}
                                    placeholder={t('rulesLegacy.destinationPlaceholder')}
                                    className="form-input"
                                />
                                <small className="form-help">{t('rulesLegacy.destinationHelp')}</small>
                            </div>

                            <div className="form-group">
                                <label className="checkbox-label">
                                    <input
                                        type="checkbox"
                                        checked={formData.enabled}
                                        onChange={(e) => setFormData({ ...formData, enabled: e.target.checked })}
                                        className="form-checkbox"
                                    />
                                    <span className="checkbox-text">{t('rulesLegacy.enabledNow')}</span>
                                </label>
                            </div>
                        </div>

                        <div className="modal-footer">
                            <button className="btn-secondary" onClick={closeForm}>
                                {t('common.cancel')}
                            </button>
                            <button
                                className="btn-primary"
                                onClick={() => void saveRule()}
                                disabled={!formData.name || !formData.pattern || !formData.destination}
                            >
                                {editingRule ? t('common.save') : t('rulesLegacy.create')}
                            </button>
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
};

export default RulesView;
