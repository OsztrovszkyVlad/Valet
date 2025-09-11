import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import './PreferencesView.css';

interface Config {
    inbox_paths: string[];
    pause_watchers: boolean;
}

interface FormData {
    inbox_paths: string;
    pause_watchers: boolean;
}

const PreferencesView: React.FC = () => {
    const [config, setConfig] = useState<Config>({ inbox_paths: [], pause_watchers: false });
    const [formData, setFormData] = useState<FormData>({ inbox_paths: '', pause_watchers: false });
    const [loading, setLoading] = useState(true);
    const [saving, setSaving] = useState(false);
    const [error, setError] = useState<string>('');
    const [successMessage, setSuccessMessage] = useState<string>('');

    // Load configuration on component mount
    useEffect(() => {
        loadConfig();
    }, []);

    const loadConfig = async () => {
        try {
            setLoading(true);
            setError('');
            const result = await invoke<Config>('get_config');
            setConfig(result);
            setFormData({
                inbox_paths: result.inbox_paths.join('\n'),
                pause_watchers: result.pause_watchers
            });
        } catch (err) {
            console.error('Failed to load config:', err);
            setError('Failed to load configuration. Please try again.');
        } finally {
            setLoading(false);
        }
    };

    const handleSave = async () => {
        try {
            setSaving(true);
            setError('');
            setSuccessMessage('');

            // Parse paths from textarea (split by newlines, filter empty)
            const paths = formData.inbox_paths
                .split('\n')
                .map(path => path.trim())
                .filter(path => path.length > 0);

            if (paths.length === 0) {
                setError('At least one inbox path is required.');
                return;
            }

            const newConfig: Config = {
                inbox_paths: paths,
                pause_watchers: formData.pause_watchers
            };

            await invoke('save_config', { config: newConfig });
            setConfig(newConfig);
            setSuccessMessage('Configuration saved successfully!');

            // Clear success message after 3 seconds
            setTimeout(() => setSuccessMessage(''), 3000);
        } catch (err) {
            console.error('Failed to save config:', err);
            setError('Failed to save configuration. Please try again.');
        } finally {
            setSaving(false);
        }
    };

    const handleReset = () => {
        setFormData({
            inbox_paths: config.inbox_paths.join('\n'),
            pause_watchers: config.pause_watchers
        });
        setError('');
        setSuccessMessage('');
    };

    const handleAddDownloadsFolder = () => {
        const downloadsPath = 'C:\\Users\\%USERNAME%\\Downloads';
        const currentPaths = formData.inbox_paths.trim();
        const newPaths = currentPaths
            ? `${currentPaths}\n${downloadsPath}`
            : downloadsPath;

        setFormData(prev => ({
            ...prev,
            inbox_paths: newPaths
        }));
    };

    const handleAddDesktopFolder = () => {
        const desktopPath = 'C:\\Users\\%USERNAME%\\Desktop';
        const currentPaths = formData.inbox_paths.trim();
        const newPaths = currentPaths
            ? `${currentPaths}\n${desktopPath}`
            : desktopPath;

        setFormData(prev => ({
            ...prev,
            inbox_paths: newPaths
        }));
    };

    if (loading) {
        return (
            <div className="preferences-view">
                <div className="view-header">
                    <h2 className="view-title">Preferences</h2>
                    <p className="view-description">
                        Configure your file organization settings and watched folders.
                    </p>
                </div>
                <div className="loading-state">
                    <div className="loading-spinner">⟳</div>
                    <p>Loading configuration...</p>
                </div>
            </div>
        );
    }

    const hasChanges = (
        formData.inbox_paths !== config.inbox_paths.join('\n') ||
        formData.pause_watchers !== config.pause_watchers
    );

    return (
        <div className="preferences-view">
            <div className="view-header">
                <h2 className="view-title">Preferences</h2>
                <p className="view-description">
                    Configure your file organization settings and watched folders.
                </p>
            </div>

            <div className="view-content">
                {error && (
                    <div className="error-message">
                        <span className="error-icon">⚠</span>
                        {error}
                        <button
                            className="error-close"
                            onClick={() => setError('')}
                            aria-label="Close error message"
                        >
                            ×
                        </button>
                    </div>
                )}

                {successMessage && (
                    <div className="success-message">
                        <span className="success-icon">✓</span>
                        {successMessage}
                        <button
                            className="success-close"
                            onClick={() => setSuccessMessage('')}
                            aria-label="Close success message"
                        >
                            ×
                        </button>
                    </div>
                )}

                <div className="preferences-form">
                    <div className="form-section">
                        <h3 className="section-title">Watched Folders</h3>
                        <p className="section-description">
                            Add folders that Valet should monitor for new files. Enter one path per line.
                        </p>

                        <div className="form-group">
                            <label htmlFor="inbox-paths">Folder Paths</label>
                            <textarea
                                id="inbox-paths"
                                className="form-textarea"
                                value={formData.inbox_paths}
                                onChange={(e) => setFormData(prev => ({
                                    ...prev,
                                    inbox_paths: e.target.value
                                }))}
                                placeholder="C:\Users\%USERNAME%\Downloads&#10;C:\Users\%USERNAME%\Desktop"
                                rows={4}
                            />
                            <small className="form-help">
                                Each line should contain a full folder path. Use %USERNAME% for the current user.
                            </small>
                        </div>

                        <div className="quick-actions">
                            <button
                                type="button"
                                className="btn-secondary btn-small"
                                onClick={handleAddDownloadsFolder}
                            >
                                + Add Downloads Folder
                            </button>
                            <button
                                type="button"
                                className="btn-secondary btn-small"
                                onClick={handleAddDesktopFolder}
                            >
                                + Add Desktop Folder
                            </button>
                        </div>
                    </div>

                    <div className="form-section">
                        <h3 className="section-title">File Monitoring</h3>
                        <p className="section-description">
                            Control when Valet watches for file changes.
                        </p>

                        <div className="form-group">
                            <label className="checkbox-label">
                                <input
                                    type="checkbox"
                                    className="form-checkbox"
                                    checked={formData.pause_watchers}
                                    onChange={(e) => setFormData(prev => ({
                                        ...prev,
                                        pause_watchers: e.target.checked
                                    }))}
                                />
                                <span className="checkbox-text">
                                    Pause file watchers
                                </span>
                            </label>
                            <small className="form-help">
                                When paused, Valet will not automatically organize new files.
                            </small>
                        </div>
                    </div>

                    <div className="form-actions">
                        <button
                            type="button"
                            className="btn-secondary"
                            onClick={handleReset}
                            disabled={!hasChanges || saving}
                        >
                            Reset
                        </button>
                        <button
                            type="button"
                            className="btn-primary"
                            onClick={handleSave}
                            disabled={!hasChanges || saving}
                        >
                            {saving ? (
                                <>
                                    <span className="loading-spinner">⟳</span>
                                    Saving...
                                </>
                            ) : (
                                'Save Changes'
                            )}
                        </button>
                    </div>
                </div>
            </div>
        </div>
    );
};

export default PreferencesView;
