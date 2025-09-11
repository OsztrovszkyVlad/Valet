import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import './StatsView.css';

interface OperationStats {
    total_operations: number;
    successful_operations: number;
    failed_operations: number;
    files_moved: number;
    files_copied: number;
    rules_applied_count: Record<string, number>;
    file_types_organized: Record<string, number>;
    average_operations_per_day: number;
    last_operation_date: string | null;
}

interface RecentOperation {
    id: number;
    source_path: string;
    destination_path: string;
    operation_type: string;
    rule_name: string;
    status: string;
    error_message: string | null;
    created_at: string;
}

interface StatsResponse {
    stats: OperationStats;
    recent_operations: RecentOperation[];
}

const StatsView: React.FC = () => {
    const [stats, setStats] = useState<OperationStats | null>(null);
    const [recentOps, setRecentOps] = useState<RecentOperation[]>([]);
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState<string>('');
    const [timeRange, setTimeRange] = useState<string>('7d');

    useEffect(() => {
        loadStats();
    }, [timeRange]);

    const loadStats = async () => {
        try {
            setLoading(true);
            setError('');

            const response = await invoke<StatsResponse>('get_statistics', {
                timeRange: timeRange
            });

            setStats(response.stats);
            setRecentOps(response.recent_operations);
        } catch (err) {
            console.error('Failed to load statistics:', err);
            setError('Failed to load statistics. Please try again.');
        } finally {
            setLoading(false);
        }
    };

    const clearHistory = async () => {
        if (!confirm('Are you sure you want to clear all operation history? This action cannot be undone.')) {
            return;
        }

        try {
            setLoading(true);
            await invoke('clear_operation_history');
            await loadStats();
        } catch (err) {
            console.error('Failed to clear history:', err);
            setError('Failed to clear operation history. Please try again.');
        } finally {
            setLoading(false);
        }
    };

    const formatDate = (dateString: string): string => {
        return new Date(dateString).toLocaleString();
    };

    const getSuccessRate = (): number => {
        if (!stats || stats.total_operations === 0) return 0;
        return Math.round((stats.successful_operations / stats.total_operations) * 100);
    };

    const getFileIcon = (filePath: string) => {
        const ext = filePath.split('.').pop()?.toLowerCase();
        const iconMap: { [key: string]: string } = {
            'pdf': '📄',
            'doc': '📝', 'docx': '📝',
            'xls': '📊', 'xlsx': '📊',
            'ppt': '📰', 'pptx': '📰',
            'txt': '📃',
            'jpg': '🖼️', 'jpeg': '🖼️', 'png': '🖼️', 'gif': '🖼️',
            'mp4': '🎬', 'avi': '🎬', 'mkv': '🎬',
            'mp3': '🎵', 'wav': '🎵', 'flac': '🎵',
            'zip': '📦', 'rar': '📦', '7z': '📦',
            'exe': '⚙️', 'msi': '⚙️',
        };
        return iconMap[ext || ''] || '📁';
    };

    const getStatusIcon = (status: string): string => {
        return status === 'success' ? '✅' : '❌';
    };

    return (
        <div className="stats-view">
            <div className="view-header">
                <h2 className="view-title">Statistics & History</h2>
                <p className="view-description">
                    Track your file organization patterns and operation history
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

                <div className="stats-controls">
                    <div className="time-range-selector">
                        <label htmlFor="timeRange">Time Range:</label>
                        <select
                            id="timeRange"
                            value={timeRange}
                            onChange={(e) => setTimeRange(e.target.value)}
                            disabled={loading}
                        >
                            <option value="1d">Last 24 Hours</option>
                            <option value="7d">Last 7 Days</option>
                            <option value="30d">Last 30 Days</option>
                            <option value="90d">Last 3 Months</option>
                            <option value="all">All Time</option>
                        </select>
                    </div>

                    <button
                        className="btn-refresh"
                        onClick={loadStats}
                        disabled={loading}
                    >
                        {loading ? '⟳' : '🔄'} Refresh
                    </button>

                    <button
                        className="btn-clear-history"
                        onClick={clearHistory}
                        disabled={loading}
                    >
                        🗑️ Clear History
                    </button>
                </div>

                {stats && (
                    <>
                        <div className="stats-overview">
                            <div className="stats-grid">
                                <div className="stat-card">
                                    <div className="stat-icon">📊</div>
                                    <div className="stat-content">
                                        <div className="stat-value">{stats.total_operations}</div>
                                        <div className="stat-label">Total Operations</div>
                                    </div>
                                </div>

                                <div className="stat-card">
                                    <div className="stat-icon">✅</div>
                                    <div className="stat-content">
                                        <div className="stat-value">{getSuccessRate()}%</div>
                                        <div className="stat-label">Success Rate</div>
                                    </div>
                                </div>

                                <div className="stat-card">
                                    <div className="stat-icon">📁</div>
                                    <div className="stat-content">
                                        <div className="stat-value">{stats.files_moved}</div>
                                        <div className="stat-label">Files Moved</div>
                                    </div>
                                </div>

                                <div className="stat-card">
                                    <div className="stat-icon">📋</div>
                                    <div className="stat-content">
                                        <div className="stat-value">{stats.files_copied}</div>
                                        <div className="stat-label">Files Copied</div>
                                    </div>
                                </div>

                                <div className="stat-card">
                                    <div className="stat-icon">📈</div>
                                    <div className="stat-content">
                                        <div className="stat-value">{stats.average_operations_per_day.toFixed(1)}</div>
                                        <div className="stat-label">Avg/Day</div>
                                    </div>
                                </div>

                                <div className="stat-card">
                                    <div className="stat-icon">🕒</div>
                                    <div className="stat-content">
                                        <div className="stat-value">
                                            {stats.last_operation_date
                                                ? formatDate(stats.last_operation_date).split(' ')[0]
                                                : 'Never'
                                            }
                                        </div>
                                        <div className="stat-label">Last Operation</div>
                                    </div>
                                </div>
                            </div>
                        </div>

                        {Object.keys(stats.rules_applied_count).length > 0 && (
                            <div className="chart-section">
                                <h3 className="section-title">Most Used Rules</h3>
                                <div className="rules-chart">
                                    {Object.entries(stats.rules_applied_count)
                                        .sort(([, a], [, b]) => b - a)
                                        .slice(0, 10)
                                        .map(([ruleName, count]) => {
                                            const percentage = (count / stats.total_operations) * 100;
                                            return (
                                                <div key={ruleName} className="rule-bar">
                                                    <div className="rule-info">
                                                        <span className="rule-name">{ruleName}</span>
                                                        <span className="rule-count">{count}</span>
                                                    </div>
                                                    <div className="rule-progress">
                                                        <div
                                                            className="rule-progress-fill"
                                                            style={{ width: `${percentage}%` }}
                                                        ></div>
                                                    </div>
                                                </div>
                                            );
                                        })
                                    }
                                </div>
                            </div>
                        )}

                        {Object.keys(stats.file_types_organized).length > 0 && (
                            <div className="chart-section">
                                <h3 className="section-title">File Types Organized</h3>
                                <div className="file-types-grid">
                                    {Object.entries(stats.file_types_organized)
                                        .sort(([, a], [, b]) => b - a)
                                        .slice(0, 12)
                                        .map(([fileType, count]) => (
                                            <div key={fileType} className="file-type-card">
                                                <div className="file-type-icon">
                                                    {getFileIcon(`file.${fileType}`)}
                                                </div>
                                                <div className="file-type-info">
                                                    <div className="file-type-ext">{fileType.toUpperCase()}</div>
                                                    <div className="file-type-count">{count} files</div>
                                                </div>
                                            </div>
                                        ))
                                    }
                                </div>
                            </div>
                        )}
                    </>
                )}

                {recentOps.length > 0 && (
                    <div className="recent-operations">
                        <h3 className="section-title">Recent Operations</h3>
                        <div className="operations-table-container">
                            <table className="operations-table">
                                <thead>
                                    <tr>
                                        <th>Status</th>
                                        <th>File</th>
                                        <th>Operation</th>
                                        <th>Rule</th>
                                        <th>Date</th>
                                        <th>Details</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {recentOps.map((op) => (
                                        <tr key={op.id} className={`operation-row ${op.status}`}>
                                            <td>
                                                <span className="status-icon">
                                                    {getStatusIcon(op.status)}
                                                </span>
                                            </td>
                                            <td>
                                                <div className="file-info">
                                                    <span className="file-icon">
                                                        {getFileIcon(op.source_path)}
                                                    </span>
                                                    <span className="file-name">
                                                        {op.source_path.split('\\').pop() || op.source_path.split('/').pop()}
                                                    </span>
                                                </div>
                                            </td>
                                            <td>
                                                <span className={`operation-badge ${op.operation_type}`}>
                                                    {op.operation_type === 'move' ? '📁→' : '📋→'} {op.operation_type}
                                                </span>
                                            </td>
                                            <td>
                                                <span className="rule-badge">{op.rule_name}</span>
                                            </td>
                                            <td className="date-cell">
                                                {formatDate(op.created_at)}
                                            </td>
                                            <td className="details-cell">
                                                {op.status === 'failed' && op.error_message ? (
                                                    <span className="error-details" title={op.error_message}>
                                                        {op.error_message.length > 30
                                                            ? `${op.error_message.substring(0, 30)}...`
                                                            : op.error_message
                                                        }
                                                    </span>
                                                ) : (
                                                    <span className="success-details">
                                                        → {op.destination_path.split('\\').pop() || op.destination_path.split('/').pop()}
                                                    </span>
                                                )}
                                            </td>
                                        </tr>
                                    ))}
                                </tbody>
                            </table>
                        </div>
                    </div>
                )}

                {!loading && stats && stats.total_operations === 0 && (
                    <div className="empty-state">
                        <div className="empty-icon">📊</div>
                        <h3>No Operations Yet</h3>
                        <p>Start organizing files to see statistics and operation history here.</p>
                        <p className="empty-subtitle">
                            Try running a dry run analysis and executing some file operations.
                        </p>
                    </div>
                )}

                {loading && (
                    <div className="loading-state">
                        <div className="loading-spinner">⟳</div>
                        <p>Loading statistics...</p>
                    </div>
                )}
            </div>
        </div>
    );
};

export default StatsView;
