import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import './RulesView.css';

interface Rule {
    id: number;
    name: string;
    pattern: string;
    destination: string;
    enabled: boolean;
    created_at: string;
    updated_at: string;
}

interface RuleForm {
    name: string;
    pattern: string;
    destination: string;
    enabled: boolean;
}

const RulesView: React.FC = () => {
    const [rules, setRules] = useState<Rule[]>([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    const [showForm, setShowForm] = useState(false);
    const [editingRule, setEditingRule] = useState<Rule | null>(null);
    const [formData, setFormData] = useState<RuleForm>({
        name: '',
        pattern: '',
        destination: '',
        enabled: true
    });

    // Load rules from backend
    const loadRules = async () => {
        try {
            setLoading(true);
            const rulesData = await invoke<Rule[]>('get_rules');
            setRules(rulesData);
            setError(null);
        } catch (err) {
            setError(`Failed to load rules: ${err}`);
        } finally {
            setLoading(false);
        }
    };

    // Save rule (create or update)
    const saveRule = async () => {
        try {
            const ruleData = {
                ...formData,
                id: editingRule?.id
            };

            await invoke('upsert_rule', { rule: ruleData });
            await loadRules(); // Reload rules
            closeForm();
            setError(null);
        } catch (err) {
            setError(`Failed to save rule: ${err}`);
        }
    };

    // Open form for new rule
    const openNewForm = () => {
        setFormData({
            name: '',
            pattern: '',
            destination: '',
            enabled: true
        });
        setEditingRule(null);
        setShowForm(true);
    };

    // Open form for editing existing rule
    const openEditForm = (rule: Rule) => {
        setFormData({
            name: rule.name,
            pattern: rule.pattern,
            destination: rule.destination,
            enabled: rule.enabled
        });
        setEditingRule(rule);
        setShowForm(true);
    };

    // Close form
    const closeForm = () => {
        setShowForm(false);
        setEditingRule(null);
        setFormData({
            name: '',
            pattern: '',
            destination: '',
            enabled: true
        });
    };

    // Toggle rule enabled status
    const toggleRule = async (rule: Rule) => {
        try {
            const updatedRule = { ...rule, enabled: !rule.enabled };
            await invoke('upsert_rule', { rule: updatedRule });
            await loadRules();
        } catch (err) {
            setError(`Failed to toggle rule: ${err}`);
        }
    };

    // Load rules on component mount
    useEffect(() => {
        loadRules();
    }, []);

    return (
        <div className="rules-view">
            <div className="view-header">
                <div className="header-left">
                    <h2 className="view-title">File Organization Rules</h2>
                    <p className="view-description">
                        Create and manage rules to automatically organize your files
                    </p>
                </div>
                <div className="header-actions">
                    <button
                        className="btn-primary"
                        onClick={openNewForm}
                        disabled={loading}
                    >
                        + Add Rule
                    </button>
                    <button
                        className="btn-secondary"
                        onClick={loadRules}
                        disabled={loading}
                    >
                        🔄 Refresh
                    </button>
                </div>
            </div>

            {error && (
                <div className="error-message">
                    <span className="error-icon">⚠️</span>
                    {error}
                    <button className="error-close" onClick={() => setError(null)}>×</button>
                </div>
            )}

            <div className="view-content">
                {loading ? (
                    <div className="loading-state">
                        <div className="loading-spinner">⏳</div>
                        <p>Loading rules...</p>
                    </div>
                ) : rules.length === 0 ? (
                    <div className="empty-state">
                        <div className="empty-icon">📋</div>
                        <h3>No Rules Created Yet</h3>
                        <p>Get started by creating your first file organization rule.</p>
                        <button className="btn-primary" onClick={openNewForm}>
                            Create First Rule
                        </button>
                    </div>
                ) : (
                    <div className="rules-table-container">
                        <table className="rules-table">
                            <thead>
                                <tr>
                                    <th>Status</th>
                                    <th>Name</th>
                                    <th>Pattern</th>
                                    <th>Destination</th>
                                    <th>Created</th>
                                    <th>Actions</th>
                                </tr>
                            </thead>
                            <tbody>
                                {rules.map((rule) => (
                                    <tr key={rule.id} className={rule.enabled ? 'rule-enabled' : 'rule-disabled'}>
                                        <td>
                                            <button
                                                className={`toggle-btn ${rule.enabled ? 'enabled' : 'disabled'}`}
                                                onClick={() => toggleRule(rule)}
                                                title={rule.enabled ? 'Disable rule' : 'Enable rule'}
                                            >
                                                {rule.enabled ? '✅' : '❌'}
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
                                            {new Date(rule.created_at).toLocaleDateString()}
                                        </td>
                                        <td className="rule-actions">
                                            <button
                                                className="btn-small btn-edit"
                                                onClick={() => openEditForm(rule)}
                                                title="Edit rule"
                                            >
                                                ✏️
                                            </button>
                                        </td>
                                    </tr>
                                ))}
                            </tbody>
                        </table>
                    </div>
                )}
            </div>

            {/* Rule Form Modal */}
            {showForm && (
                <div className="modal-overlay">
                    <div className="modal-content">
                        <div className="modal-header">
                            <h3>{editingRule ? 'Edit Rule' : 'Create New Rule'}</h3>
                            <button className="modal-close" onClick={closeForm}>×</button>
                        </div>

                        <div className="modal-body">
                            <div className="form-group">
                                <label htmlFor="rule-name">Rule Name</label>
                                <input
                                    id="rule-name"
                                    type="text"
                                    value={formData.name}
                                    onChange={(e) => setFormData({ ...formData, name: e.target.value })}
                                    placeholder="e.g., PDF Documents"
                                    className="form-input"
                                />
                            </div>

                            <div className="form-group">
                                <label htmlFor="rule-pattern">File Pattern</label>
                                <input
                                    id="rule-pattern"
                                    type="text"
                                    value={formData.pattern}
                                    onChange={(e) => setFormData({ ...formData, pattern: e.target.value })}
                                    placeholder="e.g., *.pdf or invoice_*.xlsx"
                                    className="form-input"
                                />
                                <small className="form-help">
                                    Use wildcards like *.pdf, *.&#123;jpg,png&#125;, or prefix_*.txt
                                </small>
                            </div>

                            <div className="form-group">
                                <label htmlFor="rule-destination">Destination Folder</label>
                                <input
                                    id="rule-destination"
                                    type="text"
                                    value={formData.destination}
                                    onChange={(e) => setFormData({ ...formData, destination: e.target.value })}
                                    placeholder="e.g., C:\\Documents\\PDFs"
                                    className="form-input"
                                />
                                <small className="form-help">
                                    Full path to destination folder
                                </small>
                            </div>

                            <div className="form-group">
                                <label className="checkbox-label">
                                    <input
                                        type="checkbox"
                                        checked={formData.enabled}
                                        onChange={(e) => setFormData({ ...formData, enabled: e.target.checked })}
                                        className="form-checkbox"
                                    />
                                    <span className="checkbox-text">Enable rule immediately</span>
                                </label>
                            </div>
                        </div>

                        <div className="modal-footer">
                            <button className="btn-secondary" onClick={closeForm}>
                                Cancel
                            </button>
                            <button
                                className="btn-primary"
                                onClick={saveRule}
                                disabled={!formData.name || !formData.pattern || !formData.destination}
                            >
                                {editingRule ? 'Update Rule' : 'Create Rule'}
                            </button>
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
};

export default RulesView;
