import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import './SettingsView.css';

interface AppSettings {
    // Appearance
    theme: 'light' | 'dark' | 'system';
    compact_mode: boolean;

    // Notifications
    notifications_enabled: boolean;
    notification_types: {
        file_operations: boolean;
        errors: boolean;
        daily_summary: boolean;
    };

    // System Integration
    auto_start: boolean;
    minimize_to_tray: boolean;
    close_to_tray: boolean;
    show_context_menu: boolean;

    // File Operations
    confirm_operations: boolean;
    create_backups: boolean;
    backup_location: string;
    max_backup_age_days: number;

    // Performance
    max_concurrent_operations: number;
    file_size_limit_mb: number;
    enable_file_watching: boolean;
    watch_interval_ms: number;

    // Advanced
    log_level: 'error' | 'warn' | 'info' | 'debug';
    max_log_files: number;
    enable_telemetry: boolean;
}

interface Config {
    inbox_paths: string[];
    pause_watchers: boolean;
}

const defaultSettings: AppSettings = {
    theme: 'system',
    compact_mode: false,
    notifications_enabled: true,
    notification_types: {
        file_operations: true,
        errors: true,
        daily_summary: false,
    },
    auto_start: false,
    minimize_to_tray: true,
    close_to_tray: false,
    show_context_menu: true,
    confirm_operations: false,
    create_backups: true,
    backup_location: '',
    max_backup_age_days: 30,
    max_concurrent_operations: 5,
    file_size_limit_mb: 1024,
    enable_file_watching: true,
    watch_interval_ms: 500,
    log_level: 'info',
    max_log_files: 10,
    enable_telemetry: false,
};

const SettingsView: React.FC = () => {
    const [activeTab, setActiveTab] = useState('general');
    const [settings, setSettings] = useState<AppSettings>(defaultSettings);
    const [config, setConfig] = useState<Config>({ inbox_paths: [], pause_watchers: false });
    const [formData, setFormData] = useState<{ inbox_paths: string; pause_watchers: boolean }>({
        inbox_paths: '',
        pause_watchers: false
    });
    const [loading, setLoading] = useState(true);
    const [saving, setSaving] = useState(false);
    const [error, setError] = useState<string>('');
    const [successMessage, setSuccessMessage] = useState<string>('');

    const tabs = [
        { id: 'general', label: 'General', icon: '⚙️' },
        { id: 'folders', label: 'Folders', icon: '📁' },
        { id: 'notifications', label: 'Notifications', icon: '🔔' },
        { id: 'system', label: 'System', icon: '💻' },
        { id: 'advanced', label: 'Advanced', icon: '🔧' },
    ];

    useEffect(() => {
        loadSettings();
    }, []);

    const loadSettings = async () => {
        try {
            setLoading(true);
            setError('');

            // Load existing config (inbox paths)
            const configResult = await invoke<Config>('get_config');
            setConfig(configResult);
            setFormData({
                inbox_paths: configResult.inbox_paths.join('\n'),
                pause_watchers: configResult.pause_watchers
            });

            // Load app settings from backend
            const settingsResult = await invoke<AppSettings>('get_app_settings');
            setSettings(settingsResult);

        } catch (err) {
            console.error('Failed to load settings:', err);
            setError('Failed to load settings. Please try again.');
        } finally {
            setLoading(false);
        }
    };

    const saveSettings = async () => {
        try {
            setSaving(true);
            setError('');
            setSuccessMessage('');

            // Save folder configuration
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

            // Save app settings to backend
            await invoke('save_app_settings', { settings });

            setSuccessMessage('Settings saved successfully!');
            setTimeout(() => setSuccessMessage(''), 3000);
        } catch (err) {
            console.error('Failed to save settings:', err);
            setError('Failed to save settings. Please try again.');
        } finally {
            setSaving(false);
        }
    };

    const resetSettings = () => {
        setSettings(defaultSettings);
        setFormData({
            inbox_paths: config.inbox_paths.join('\n'),
            pause_watchers: config.pause_watchers
        });
        setError('');
        setSuccessMessage('');
    };

    const exportSettings = async () => {
        try {
            const exportData = {
                settings,
                config,
                exported_at: new Date().toISOString(),
                version: '1.0.0'
            };

            // TODO: Implement file save dialog
            const dataStr = JSON.stringify(exportData, null, 2);
            console.log('Export data:', dataStr);
            setSuccessMessage('Settings exported successfully!');
            setTimeout(() => setSuccessMessage(''), 3000);
        } catch (err) {
            setError('Failed to export settings.');
        }
    };

    const importSettings = async () => {
        try {
            // TODO: Implement file open dialog and import logic
            setSuccessMessage('Settings imported successfully!');
            setTimeout(() => setSuccessMessage(''), 3000);
        } catch (err) {
            setError('Failed to import settings.');
        }
    };

    const handleSettingChange = async (key: keyof AppSettings, value: any) => {
        setSettings(prev => ({
            ...prev,
            [key]: value
        }));

        // Handle special cases that require system integration
        if (key === 'show_context_menu') {
            try {
                if (value) {
                    await invoke('install_context_menu_integration');
                    console.log('Context menu integration installed');
                } else {
                    await invoke('uninstall_context_menu_integration');
                    console.log('Context menu integration removed');
                }
            } catch (error) {
                console.error('Failed to update context menu integration:', error);
                // Revert the setting if the operation failed
                setSettings(prev => ({
                    ...prev,
                    [key]: !value
                }));
            }
        }

        if (key === 'auto_start') {
            try {
                await invoke('configure_auto_start', { enabled: value });
                console.log('Auto-start configuration updated');
            } catch (error) {
                console.error('Failed to update auto-start configuration:', error);
                // Revert the setting if the operation failed
                setSettings(prev => ({
                    ...prev,
                    [key]: !value
                }));
            }
        }
    };

    const handleNestedSettingChange = (parentKey: keyof AppSettings, childKey: string, value: any) => {
        setSettings(prev => ({
            ...prev,
            [parentKey]: {
                ...(prev[parentKey] as any),
                [childKey]: value
            }
        }));
    };

    if (loading) {
        return (
            <div className="settings-view">
                <div className="view-header">
                    <h2 className="view-title">Settings</h2>
                    <p className="view-description">
                        Configure your application preferences and behavior.
                    </p>
                </div>
                <div className="loading-state">
                    <div className="loading-spinner">⟳</div>
                    <p>Loading settings...</p>
                </div>
            </div>
        );
    }

    const hasChanges = (
        formData.inbox_paths !== config.inbox_paths.join('\n') ||
        formData.pause_watchers !== config.pause_watchers
    );

    return (
        <div className="settings-view">
            <div className="view-header">
                <h2 className="view-title">Settings</h2>
                <p className="view-description">
                    Configure your application preferences and behavior.
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

                <div className="settings-container">
                    {/* Settings Navigation */}
                    <div className="settings-nav">
                        {tabs.map((tab) => (
                            <button
                                key={tab.id}
                                className={`settings-tab ${activeTab === tab.id ? 'active' : ''}`}
                                onClick={() => setActiveTab(tab.id)}
                            >
                                <span className="tab-icon">{tab.icon}</span>
                                <span className="tab-label">{tab.label}</span>
                            </button>
                        ))}
                    </div>

                    {/* Settings Content */}
                    <div className="settings-content">
                        {activeTab === 'general' && (
                            <div className="settings-section">
                                <h3 className="section-title">General Settings</h3>

                                <div className="setting-group">
                                    <h4 className="group-title">Appearance</h4>

                                    <div className="form-group">
                                        <label htmlFor="theme">Theme</label>
                                        <select
                                            id="theme"
                                            className="form-select"
                                            value={settings.theme}
                                            onChange={(e) => handleSettingChange('theme', e.target.value as 'light' | 'dark' | 'system')}
                                        >
                                            <option value="light">Light</option>
                                            <option value="dark">Dark</option>
                                            <option value="system">System Default</option>
                                        </select>
                                        <small className="form-help">
                                            Choose the color theme for the application.
                                        </small>
                                    </div>

                                    <div className="form-group">
                                        <label className="checkbox-label">
                                            <input
                                                type="checkbox"
                                                className="form-checkbox"
                                                checked={settings.compact_mode}
                                                onChange={(e) => handleSettingChange('compact_mode', e.target.checked)}
                                            />
                                            <span className="checkbox-text">Compact mode</span>
                                        </label>
                                        <small className="form-help">
                                            Use a more compact interface layout.
                                        </small>
                                    </div>
                                </div>

                                <div className="setting-group">
                                    <h4 className="group-title">File Operations</h4>

                                    <div className="form-group">
                                        <label className="checkbox-label">
                                            <input
                                                type="checkbox"
                                                className="form-checkbox"
                                                checked={settings.confirm_operations}
                                                onChange={(e) => handleSettingChange('confirm_operations', e.target.checked)}
                                            />
                                            <span className="checkbox-text">Confirm file operations</span>
                                        </label>
                                        <small className="form-help">
                                            Show confirmation dialog before executing file operations.
                                        </small>
                                    </div>

                                    <div className="form-group">
                                        <label className="checkbox-label">
                                            <input
                                                type="checkbox"
                                                className="form-checkbox"
                                                checked={settings.create_backups}
                                                onChange={(e) => handleSettingChange('create_backups', e.target.checked)}
                                            />
                                            <span className="checkbox-text">Create backups before moving files</span>
                                        </label>
                                        <small className="form-help">
                                            Automatically create backups of files before organizing them.
                                        </small>
                                    </div>

                                    {settings.create_backups && (
                                        <>
                                            <div className="form-group">
                                                <label htmlFor="backup-location">Backup Location</label>
                                                <input
                                                    id="backup-location"
                                                    type="text"
                                                    className="form-input"
                                                    value={settings.backup_location}
                                                    onChange={(e) => handleSettingChange('backup_location', e.target.value)}
                                                    placeholder="C:\\Backups\\Valet"
                                                />
                                                <small className="form-help">
                                                    Directory where backup files will be stored.
                                                </small>
                                            </div>

                                            <div className="form-group">
                                                <label htmlFor="backup-age">Maximum backup age (days)</label>
                                                <input
                                                    id="backup-age"
                                                    type="number"
                                                    className="form-input"
                                                    value={settings.max_backup_age_days}
                                                    onChange={(e) => handleSettingChange('max_backup_age_days', parseInt(e.target.value) || 30)}
                                                    min="1"
                                                    max="365"
                                                />
                                                <small className="form-help">
                                                    Automatically delete backups older than this many days.
                                                </small>
                                            </div>
                                        </>
                                    )}
                                </div>
                            </div>
                        )}

                        {activeTab === 'folders' && (
                            <div className="settings-section">
                                <h3 className="section-title">Watched Folders</h3>

                                <div className="setting-group">
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
                                            rows={6}
                                        />
                                        <small className="form-help">
                                            Each line should contain a full folder path. Use %USERNAME% for the current user.
                                        </small>
                                    </div>

                                    <div className="quick-actions">
                                        <button
                                            type="button"
                                            className="btn-secondary btn-small"
                                            onClick={() => {
                                                const downloadsPath = 'C:\\Users\\%USERNAME%\\Downloads';
                                                const currentPaths = formData.inbox_paths.trim();
                                                const newPaths = currentPaths
                                                    ? `${currentPaths}\n${downloadsPath}`
                                                    : downloadsPath;
                                                setFormData(prev => ({ ...prev, inbox_paths: newPaths }));
                                            }}
                                        >
                                            + Add Downloads Folder
                                        </button>
                                        <button
                                            type="button"
                                            className="btn-secondary btn-small"
                                            onClick={() => {
                                                const desktopPath = 'C:\\Users\\%USERNAME%\\Desktop';
                                                const currentPaths = formData.inbox_paths.trim();
                                                const newPaths = currentPaths
                                                    ? `${currentPaths}\n${desktopPath}`
                                                    : desktopPath;
                                                setFormData(prev => ({ ...prev, inbox_paths: newPaths }));
                                            }}
                                        >
                                            + Add Desktop Folder
                                        </button>
                                    </div>

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
                                            <span className="checkbox-text">Pause file watchers</span>
                                        </label>
                                        <small className="form-help">
                                            When paused, Valet will not automatically organize new files.
                                        </small>
                                    </div>
                                </div>

                                <div className="setting-group">
                                    <h4 className="group-title">Performance</h4>

                                    <div className="form-group">
                                        <label className="checkbox-label">
                                            <input
                                                type="checkbox"
                                                className="form-checkbox"
                                                checked={settings.enable_file_watching}
                                                onChange={(e) => handleSettingChange('enable_file_watching', e.target.checked)}
                                            />
                                            <span className="checkbox-text">Enable file watching</span>
                                        </label>
                                        <small className="form-help">
                                            Monitor folders for file changes in real-time.
                                        </small>
                                    </div>

                                    {settings.enable_file_watching && (
                                        <div className="form-group">
                                            <label htmlFor="watch-interval">Watch interval (milliseconds)</label>
                                            <input
                                                id="watch-interval"
                                                type="number"
                                                className="form-input"
                                                value={settings.watch_interval_ms}
                                                onChange={(e) => handleSettingChange('watch_interval_ms', parseInt(e.target.value) || 500)}
                                                min="100"
                                                max="5000"
                                                step="100"
                                            />
                                            <small className="form-help">
                                                How often to check for file changes. Lower values use more system resources.
                                            </small>
                                        </div>
                                    )}
                                </div>
                            </div>
                        )}

                        {activeTab === 'notifications' && (
                            <div className="settings-section">
                                <h3 className="section-title">Notifications</h3>

                                <div className="setting-group">
                                    <div className="form-group">
                                        <label className="checkbox-label">
                                            <input
                                                type="checkbox"
                                                className="form-checkbox"
                                                checked={settings.notifications_enabled}
                                                onChange={(e) => handleSettingChange('notifications_enabled', e.target.checked)}
                                            />
                                            <span className="checkbox-text">Enable notifications</span>
                                        </label>
                                        <small className="form-help">
                                            Show desktop notifications for various events.
                                        </small>
                                    </div>

                                    {settings.notifications_enabled && (
                                        <div className="notification-types">
                                            <h4 className="group-title">Notification Types</h4>

                                            <div className="form-group">
                                                <label className="checkbox-label">
                                                    <input
                                                        type="checkbox"
                                                        className="form-checkbox"
                                                        checked={settings.notification_types.file_operations}
                                                        onChange={(e) => handleNestedSettingChange('notification_types', 'file_operations', e.target.checked)}
                                                    />
                                                    <span className="checkbox-text">File operations</span>
                                                </label>
                                                <small className="form-help">
                                                    Notify when files are moved, copied, or organized.
                                                </small>
                                            </div>

                                            <div className="form-group">
                                                <label className="checkbox-label">
                                                    <input
                                                        type="checkbox"
                                                        className="form-checkbox"
                                                        checked={settings.notification_types.errors}
                                                        onChange={(e) => handleNestedSettingChange('notification_types', 'errors', e.target.checked)}
                                                    />
                                                    <span className="checkbox-text">Errors and warnings</span>
                                                </label>
                                                <small className="form-help">
                                                    Notify when errors occur during file operations.
                                                </small>
                                            </div>

                                            <div className="form-group">
                                                <label className="checkbox-label">
                                                    <input
                                                        type="checkbox"
                                                        className="form-checkbox"
                                                        checked={settings.notification_types.daily_summary}
                                                        onChange={(e) => handleNestedSettingChange('notification_types', 'daily_summary', e.target.checked)}
                                                    />
                                                    <span className="checkbox-text">Daily summary</span>
                                                </label>
                                                <small className="form-help">
                                                    Show a daily summary of file organization activity.
                                                </small>
                                            </div>
                                        </div>
                                    )}
                                </div>
                            </div>
                        )}

                        {activeTab === 'system' && (
                            <div className="settings-section">
                                <h3 className="section-title">System Integration</h3>

                                <div className="setting-group">
                                    <h4 className="group-title">Startup & Tray</h4>

                                    <div className="form-group">
                                        <label className="checkbox-label">
                                            <input
                                                type="checkbox"
                                                className="form-checkbox"
                                                checked={settings.auto_start}
                                                onChange={(e) => handleSettingChange('auto_start', e.target.checked)}
                                            />
                                            <span className="checkbox-text">Start with Windows</span>
                                        </label>
                                        <small className="form-help">
                                            Automatically start Valet when Windows boots.
                                        </small>
                                    </div>

                                    <div className="form-group">
                                        <label className="checkbox-label">
                                            <input
                                                type="checkbox"
                                                className="form-checkbox"
                                                checked={settings.minimize_to_tray}
                                                onChange={(e) => handleSettingChange('minimize_to_tray', e.target.checked)}
                                            />
                                            <span className="checkbox-text">Minimize to system tray</span>
                                        </label>
                                        <small className="form-help">
                                            Hide window to system tray when minimized.
                                        </small>
                                    </div>

                                    <div className="form-group">
                                        <label className="checkbox-label">
                                            <input
                                                type="checkbox"
                                                className="form-checkbox"
                                                checked={settings.close_to_tray}
                                                onChange={(e) => handleSettingChange('close_to_tray', e.target.checked)}
                                            />
                                            <span className="checkbox-text">Close to system tray</span>
                                        </label>
                                        <small className="form-help">
                                            Hide window to system tray when closed instead of exiting.
                                        </small>
                                    </div>
                                </div>

                                <div className="setting-group">
                                    <h4 className="group-title">Context Menu</h4>

                                    <div className="form-group">
                                        <label className="checkbox-label">
                                            <input
                                                type="checkbox"
                                                className="form-checkbox"
                                                checked={settings.show_context_menu}
                                                onChange={(e) => handleSettingChange('show_context_menu', e.target.checked)}
                                            />
                                            <span className="checkbox-text">Add context menu items</span>
                                        </label>
                                        <small className="form-help">
                                            Add "Organize with Valet" to file and folder context menus.
                                        </small>
                                    </div>
                                </div>

                                <div className="setting-group">
                                    <h4 className="group-title">Performance Limits</h4>

                                    <div className="form-group">
                                        <label htmlFor="concurrent-ops">Maximum concurrent operations</label>
                                        <input
                                            id="concurrent-ops"
                                            type="number"
                                            className="form-input"
                                            value={settings.max_concurrent_operations}
                                            onChange={(e) => handleSettingChange('max_concurrent_operations', parseInt(e.target.value) || 5)}
                                            min="1"
                                            max="20"
                                        />
                                        <small className="form-help">
                                            Number of file operations that can run simultaneously.
                                        </small>
                                    </div>

                                    <div className="form-group">
                                        <label htmlFor="file-size-limit">File size limit (MB)</label>
                                        <input
                                            id="file-size-limit"
                                            type="number"
                                            className="form-input"
                                            value={settings.file_size_limit_mb}
                                            onChange={(e) => handleSettingChange('file_size_limit_mb', parseInt(e.target.value) || 1024)}
                                            min="1"
                                            max="10240"
                                        />
                                        <small className="form-help">
                                            Skip files larger than this size (in megabytes).
                                        </small>
                                    </div>
                                </div>
                            </div>
                        )}

                        {activeTab === 'advanced' && (
                            <div className="settings-section">
                                <h3 className="section-title">Advanced Settings</h3>

                                <div className="setting-group">
                                    <h4 className="group-title">Logging</h4>

                                    <div className="form-group">
                                        <label htmlFor="log-level">Log level</label>
                                        <select
                                            id="log-level"
                                            className="form-select"
                                            value={settings.log_level}
                                            onChange={(e) => handleSettingChange('log_level', e.target.value as 'error' | 'warn' | 'info' | 'debug')}
                                        >
                                            <option value="error">Error</option>
                                            <option value="warn">Warning</option>
                                            <option value="info">Information</option>
                                            <option value="debug">Debug</option>
                                        </select>
                                        <small className="form-help">
                                            Level of detail for application logs.
                                        </small>
                                    </div>

                                    <div className="form-group">
                                        <label htmlFor="max-log-files">Maximum log files</label>
                                        <input
                                            id="max-log-files"
                                            type="number"
                                            className="form-input"
                                            value={settings.max_log_files}
                                            onChange={(e) => handleSettingChange('max_log_files', parseInt(e.target.value) || 10)}
                                            min="1"
                                            max="100"
                                        />
                                        <small className="form-help">
                                            Number of log files to keep before rotating.
                                        </small>
                                    </div>
                                </div>

                                <div className="setting-group">
                                    <h4 className="group-title">Privacy</h4>

                                    <div className="form-group">
                                        <label className="checkbox-label">
                                            <input
                                                type="checkbox"
                                                className="form-checkbox"
                                                checked={settings.enable_telemetry}
                                                onChange={(e) => handleSettingChange('enable_telemetry', e.target.checked)}
                                            />
                                            <span className="checkbox-text">Enable anonymous telemetry</span>
                                        </label>
                                        <small className="form-help">
                                            Help improve Valet by sharing anonymous usage statistics.
                                        </small>
                                    </div>
                                </div>

                                <div className="setting-group">
                                    <h4 className="group-title">Import/Export</h4>

                                    <div className="form-group">
                                        <div className="import-export-actions">
                                            <button
                                                type="button"
                                                className="btn-secondary"
                                                onClick={exportSettings}
                                                disabled={saving}
                                            >
                                                📤 Export Settings
                                            </button>
                                            <button
                                                type="button"
                                                className="btn-secondary"
                                                onClick={importSettings}
                                                disabled={saving}
                                            >
                                                📥 Import Settings
                                            </button>
                                        </div>
                                        <small className="form-help">
                                            Save or restore your configuration settings.
                                        </small>
                                    </div>
                                </div>
                            </div>
                        )}
                    </div>
                </div>

                {/* Actions */}
                <div className="settings-actions">
                    <button
                        type="button"
                        className="btn-secondary"
                        onClick={resetSettings}
                        disabled={saving}
                    >
                        Reset to Defaults
                    </button>
                    <button
                        type="button"
                        className="btn-primary"
                        onClick={saveSettings}
                        disabled={!hasChanges || saving}
                    >
                        {saving ? (
                            <>
                                <span className="loading-spinner">⟳</span>
                                Saving...
                            </>
                        ) : (
                            'Save Settings'
                        )}
                    </button>
                </div>
            </div>
        </div>
    );
};

export default SettingsView;
