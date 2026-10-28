import React, { useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
    dryRun,
    executeOperations as runExecute,
    PREVIEW_REQUIRED,
    type ExecuteResponse,
    type PlannedOperation,
    type SkippedFile,
} from '../api/commands';
import UndoPanel from '../components/UndoPanel';
import { formatDateTime } from '../i18n/format';
import './DryRunView.css';

type DryRunResult = PlannedOperation;

export interface LastBatch {
    id: string;
    createdAt: string;
}

interface DryRunViewProps {
    lastBatch: LastBatch | null;
    /** A new batch ran; `null` when it did nothing that can be undone. */
    onBatchRan: (batch: LastBatch | null) => void;
    /** The user closed the undo result; `complete`: nothing left to undo. */
    onLastBatchClosed: (complete: boolean) => void;
}

const DryRunView: React.FC<DryRunViewProps> = ({ lastBatch, onBatchRan, onLastBatchClosed }) => {
    const { t } = useTranslation();
    const [results, setResults] = useState<DryRunResult[]>([]);
    const [loading, setLoading] = useState(false);
    const [executing, setExecuting] = useState(false);
    const [error, setError] = useState<string>('');
    const [successMessage, setSuccessMessage] = useState<string>('');
    const [hasRun, setHasRun] = useState(false);
    const [totalFiles, setTotalFiles] = useState(0);
    const [totalRulesApplied, setTotalRulesApplied] = useState(0);
    const [executeResults, setExecuteResults] = useState<ExecuteResponse | null>(null);
    const [skipped, setSkipped] = useState<SkippedFile[]>([]);
    const [showSkipped, setShowSkipped] = useState(false);
    // Id of the last previewed plan; null once used, so Execute needs a new preview.
    const [planId, setPlanId] = useState<string | null>(null);

    const runDryRun = async () => {
        try {
            setLoading(true);
            setError('');

            const response = await dryRun();
            setResults(response.results);
            setPlanId(response.plan_id);
            setSkipped(response.skipped);
            setShowSkipped(false);
            setTotalFiles(response.total_files);
            setTotalRulesApplied(response.total_rules_applied);
            setHasRun(true);
        } catch (err) {
            console.error('Dry run failed:', err);
            setError('Failed to run dry run analysis. Please check your configuration and try again.');
        } finally {
            setLoading(false);
        }
    };

    const clearResults = () => {
        setResults([]);
        setPlanId(null);
        setSkipped([]);
        setHasRun(false);
        setTotalFiles(0);
        setTotalRulesApplied(0);
        setError('');
        setSuccessMessage('');
        setExecuteResults(null);
    };

    const executeOperations = async () => {
        if (planId === null) return;
        try {
            setExecuting(true);
            setError('');
            setSuccessMessage('');

            const response = await runExecute(planId);

            setExecuteResults(response);
            onBatchRan(
                response.executed_count > 0
                    ? { id: response.batch_id, createdAt: new Date().toISOString() }
                    : null,
            );

            if (response.success) {
                setSuccessMessage(`Successfully executed ${response.executed_count} file operation(s)!`);
                // Clear the dry run results since files have been moved
                setResults([]);
                setSkipped([]);
                setHasRun(false);
            } else {
                setError(`Executed ${response.executed_count} operations, but ${response.failed_operations.length} failed. See details below.`);
            }

            // Clear success message after 5 seconds
            if (response.success) {
                setTimeout(() => setSuccessMessage(''), 5000);
            }
        } catch (err) {
            console.error('Execute operations failed:', err);
            if (err === PREVIEW_REQUIRED) {
                setError(t('preview.expired'));
            } else {
                setError('Failed to execute file operations. Please try again.');
            }
        } finally {
            // The plan is single-use: success or failure, Execute needs a new preview.
            setPlanId(null);
            setExecuting(false);
        }
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

    return (
        <div className="dry-run-view">
            <div className="view-header">
                <h2 className="view-title">Dry Run Preview</h2>
                <p className="view-description">
                    Preview what file operations would be performed without actually moving files
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

                <div className="dry-run-controls">
                    <button
                        className="btn-primary"
                        onClick={runDryRun}
                        disabled={loading || executing}
                    >
                        {loading ? (
                            <>
                                <span className="loading-spinner">⟳</span>
                                Analyzing files...
                            </>
                        ) : (
                            <>
                                <span>🔍</span>
                                Run Dry Run Analysis
                            </>
                        )}
                    </button>

                    {hasRun && results.length > 0 && (
                        <button
                            className="btn-execute"
                            onClick={executeOperations}
                            disabled={loading || executing || planId === null}
                        >
                            {executing ? (
                                <>
                                    <span className="loading-spinner">⟳</span>
                                    Executing operations...
                                </>
                            ) : (
                                <>
                                    <span>⚡</span>
                                    Execute {results.length} Operation{results.length !== 1 ? 's' : ''}
                                </>
                            )}
                        </button>
                    )}

                    {hasRun && (
                        <button
                            className="btn-secondary"
                            onClick={clearResults}
                            disabled={loading || executing}
                        >
                            Clear Results
                        </button>
                    )}
                </div>

                {lastBatch && (
                    <div className="last-batch">
                        <h3 className="results-title">{t('lastBatch.title')}</h3>
                        <p className="last-batch-date">
                            {t('lastBatch.startedAt', { date: formatDateTime(lastBatch.createdAt) })}
                        </p>
                        <UndoPanel
                            key={lastBatch.id}
                            batchId={lastBatch.id}
                            canUndo={!executing}
                            onClose={onLastBatchClosed}
                        />
                    </div>
                )}

                {hasRun && !loading && (
                    <div className="dry-run-summary">
                        <div className="summary-card">
                            <div className="summary-item">
                                <span className="summary-label">Files Found</span>
                                <span className="summary-value">{totalFiles}</span>
                            </div>
                            <div className="summary-item">
                                <span className="summary-label">Rules Applied</span>
                                <span className="summary-value">{totalRulesApplied}</span>
                            </div>
                            <div className="summary-item">
                                <span className="summary-label">Operations</span>
                                <span className="summary-value">{results.length}</span>
                            </div>
                        </div>
                    </div>
                )}

                {results.length > 0 && (
                    <div className="dry-run-results">
                        <h3 className="results-title">Planned Operations</h3>
                        <div className="results-container">
                            <div className="results-table-container">
                                <table className="results-table">
                                    <thead>
                                        <tr>
                                            <th>File</th>
                                            <th>Current Location</th>
                                            <th>Will Move To</th>
                                            <th>Applied Rule</th>
                                        </tr>
                                    </thead>
                                    <tbody>
                                        {results.map((result, index) => (
                                            <tr key={index}>
                                                <td>
                                                    <div className="file-info">
                                                        <span className="file-icon">
                                                            {getFileIcon(result.source_path)}
                                                        </span>
                                                        <span className="file-name">
                                                            {result.source_path.split('\\').pop() || result.source_path.split('/').pop()}
                                                        </span>
                                                    </div>
                                                </td>
                                                <td>
                                                    <code className="path-code">
                                                        {result.source_path}
                                                    </code>
                                                </td>
                                                <td>
                                                    <code className="path-code destination">
                                                        {result.destination_path}
                                                    </code>
                                                </td>
                                                <td>
                                                    <span className="rule-badge">
                                                        {result.rule_name}
                                                    </span>
                                                </td>
                                            </tr>
                                        ))}
                                    </tbody>
                                </table>
                            </div>
                        </div>
                    </div>
                )}

                {skipped.length > 0 && (
                    <div className="skipped-files">
                        <div className="skipped-header">
                            <h3 className="results-title">{t('skipped.title', { count: skipped.length })}</h3>
                            <button className="btn-secondary" onClick={() => setShowSkipped((v) => !v)}>
                                {showSkipped ? t('skipped.hide') : t('skipped.show')}
                            </button>
                        </div>
                        {showSkipped && (
                            <div className="results-table-container">
                                <table className="results-table">
                                    <thead>
                                        <tr>
                                            <th>{t('skipped.colFile')}</th>
                                            <th>{t('skipped.colReason')}</th>
                                            <th>{t('skipped.colRule')}</th>
                                        </tr>
                                    </thead>
                                    <tbody>
                                        {skipped.map((s, index) => (
                                            <tr key={index}>
                                                <td>
                                                    <code className="path-code">{s.path}</code>
                                                </td>
                                                <td>
                                                    {t(`skipReason.${s.reason}`, {
                                                        defaultValue: t('skipReason.unknown', { reason: s.reason }),
                                                    })}
                                                </td>
                                                <td>{s.rule_name ?? ''}</td>
                                            </tr>
                                        ))}
                                    </tbody>
                                </table>
                            </div>
                        )}
                    </div>
                )}

                {executeResults && executeResults.failed_operations.length > 0 && (
                    <div className="failed-operations">
                        <h3 className="results-title">Failed Operations</h3>
                        <div className="results-container">
                            <div className="results-table-container">
                                <table className="results-table">
                                    <thead>
                                        <tr>
                                            <th>File</th>
                                            <th>Source Path</th>
                                            <th>Intended Destination</th>
                                            <th>Rule</th>
                                            <th>Error</th>
                                        </tr>
                                    </thead>
                                    <tbody>
                                        {executeResults.failed_operations.map((failure, index) => (
                                            <tr key={index} className="failure-row">
                                                <td>
                                                    <div className="file-info">
                                                        <span className="file-icon">
                                                            {getFileIcon(failure.source_path)}
                                                        </span>
                                                        <span className="file-name">
                                                            {failure.source_path.split('\\').pop() || failure.source_path.split('/').pop()}
                                                        </span>
                                                    </div>
                                                </td>
                                                <td>
                                                    <code className="path-code">
                                                        {failure.source_path}
                                                    </code>
                                                </td>
                                                <td>
                                                    <code className="path-code">
                                                        {failure.destination_path}
                                                    </code>
                                                </td>
                                                <td>
                                                    <span className="rule-badge">
                                                        {failure.rule_name}
                                                    </span>
                                                </td>
                                                <td>
                                                    <span className="error-badge">
                                                        {failure.error_message}
                                                    </span>
                                                </td>
                                            </tr>
                                        ))}
                                    </tbody>
                                </table>
                            </div>
                        </div>
                    </div>
                )}

                {hasRun && results.length === 0 && !loading && (
                    <div className="empty-state">
                        <div className="empty-icon">✨</div>
                        <h3>No Operations Needed</h3>
                        <p>All files in your watched directories are already organized correctly!</p>
                        <p className="empty-subtitle">
                            Try adding more files to your watched folders or creating new rules.
                        </p>
                    </div>
                )}

                {!hasRun && !loading && (
                    <div className="placeholder-card">
                        <div className="placeholder-icon">🔍</div>
                        <h3>Ready to Analyze</h3>
                        <p>Click "Run Dry Run Analysis" to see what file operations would be performed based on your current rules.</p>
                        <ul>
                            <li>Scans all files in watched directories</li>
                            <li>Applies rules without moving files</li>
                            <li>Shows preview of planned operations</li>
                            <li>Safe to run multiple times</li>
                        </ul>
                    </div>
                )}
            </div>
        </div>
    );
};

export default DryRunView;
