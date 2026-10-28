import React, { useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
    dryRun,
    errorMessage,
    executeOperations as runExecute,
    PREVIEW_REQUIRED,
    type ExecuteResponse,
    type PlannedOperation,
    type SkippedFile,
} from '../api/commands';
import type { LastBatch } from '../components/LastBatchBox';
import { journalErrorText, skipReasonText } from '../i18n/format';
import './DryRunView.css';

interface DryRunViewProps {
    /** A new batch ran; `null` when it did nothing that can be undone. */
    onBatchRan: (batch: LastBatch | null) => void;
    /** True while a batch runs, so other screens can hold back undo. */
    onBusyChange?: (busy: boolean) => void;
}

function fileName(path: string): string {
    const parts = path.split(/[\\/]/);
    return parts[parts.length - 1] || path;
}

/**
 * Preview, then the user's approval ("Organize") runs exactly the previewed plan.
 * Nothing here starts a file operation without a plan id from a preview.
 */
const DryRunView: React.FC<DryRunViewProps> = ({ onBatchRan, onBusyChange }) => {
    const { t } = useTranslation();
    const [results, setResults] = useState<PlannedOperation[]>([]);
    const [skipped, setSkipped] = useState<SkippedFile[]>([]);
    const [showSkipped, setShowSkipped] = useState(false);
    const [loading, setLoading] = useState(false);
    const [executing, setExecuting] = useState(false);
    const [error, setError] = useState('');
    const [successMessage, setSuccessMessage] = useState('');
    const [hasRun, setHasRun] = useState(false);
    const [executeResults, setExecuteResults] = useState<ExecuteResponse | null>(null);
    // Id of the last previewed plan; null once used, so Organize needs a new preview.
    const [planId, setPlanId] = useState<string | null>(null);

    const runDryRun = async () => {
        setLoading(true);
        setError('');
        setSuccessMessage('');
        setExecuteResults(null);
        try {
            const response = await dryRun();
            setResults(response.results);
            setPlanId(response.plan_id);
            setSkipped(response.skipped);
            setShowSkipped(false);
            setHasRun(true);
        } catch (err) {
            setError(t('preview.failed', { message: errorMessage(err) }));
        } finally {
            setLoading(false);
        }
    };

    const clearResults = () => {
        setResults([]);
        setPlanId(null);
        setSkipped([]);
        setHasRun(false);
        setError('');
        setSuccessMessage('');
        setExecuteResults(null);
    };

    const executeOperations = async () => {
        if (planId === null) return;
        setExecuting(true);
        onBusyChange?.(true);
        setError('');
        setSuccessMessage('');
        try {
            const response = await runExecute(planId);
            setExecuteResults(response);
            onBatchRan(
                response.executed_count > 0
                    ? { id: response.batch_id, createdAt: new Date().toISOString() }
                    : null,
            );
            if (response.success) {
                setSuccessMessage(t('preview.done', { count: response.executed_count }));
                setResults([]);
                setSkipped([]);
                setHasRun(false);
            } else {
                setError(
                    t('preview.partial', {
                        done: response.executed_count,
                        failed: response.failed_operations.length,
                    }),
                );
            }
        } catch (err) {
            setError(
                err === PREVIEW_REQUIRED
                    ? t('preview.expired')
                    : t('preview.executeFailed', { message: errorMessage(err) }),
            );
        } finally {
            // The plan is single-use: success or failure, Organize needs a new preview.
            setPlanId(null);
            setExecuting(false);
            onBusyChange?.(false);
        }
    };

    return (
        <div className="dry-run-view">
            <div className="view-header">
                <h2 className="view-title">{t('preview.title')}</h2>
                <p className="view-description">{t('preview.description')}</p>
            </div>

            <div className="view-content">
                {error && (
                    <div className="error-message" role="alert">
                        {error}
                        <button
                            className="error-close"
                            onClick={() => setError('')}
                            aria-label={t('common.close')}
                        >
                            ×
                        </button>
                    </div>
                )}

                {successMessage && (
                    <div className="success-message" role="status">
                        {successMessage}
                        <button
                            className="success-close"
                            onClick={() => setSuccessMessage('')}
                            aria-label={t('common.close')}
                        >
                            ×
                        </button>
                    </div>
                )}

                <div className="dry-run-controls">
                    <button className="btn-primary" onClick={runDryRun} disabled={loading || executing}>
                        {loading ? t('preview.running') : t('preview.run')}
                    </button>

                    {hasRun && results.length > 0 && (
                        <button
                            className="btn-execute"
                            onClick={executeOperations}
                            disabled={loading || executing || planId === null}
                        >
                            {executing
                                ? t('preview.executing')
                                : t('preview.execute', { count: results.length })}
                        </button>
                    )}

                    {hasRun && (
                        <button className="btn-secondary" onClick={clearResults} disabled={loading || executing}>
                            {t('preview.clear')}
                        </button>
                    )}
                </div>

                {hasRun && results.length > 0 && planId !== null && (
                    <p className="view-description">{t('preview.approveHint')}</p>
                )}

                {hasRun && !loading && (
                    <div className="dry-run-summary">
                        <div className="summary-card">
                            <div className="summary-item">
                                <span className="summary-label">{t('preview.summaryOps')}</span>
                                <span className="summary-value">{results.length}</span>
                            </div>
                            <div className="summary-item">
                                <span className="summary-label">{t('preview.summarySkipped')}</span>
                                <span className="summary-value">{skipped.length}</span>
                            </div>
                        </div>
                    </div>
                )}

                {results.length > 0 && (
                    <div className="dry-run-results">
                        <h3 className="results-title">{t('preview.plannedTitle')}</h3>
                        <div className="results-container">
                            <div className="results-table-container">
                                <table className="results-table">
                                    <thead>
                                        <tr>
                                            <th>{t('preview.colFile')}</th>
                                            <th>{t('preview.colOperation')}</th>
                                            <th>{t('preview.colFrom')}</th>
                                            <th>{t('preview.colTo')}</th>
                                            <th>{t('preview.colRule')}</th>
                                        </tr>
                                    </thead>
                                    <tbody>
                                        {results.map((result, index) => (
                                            <tr key={index}>
                                                <td>
                                                    <span className="file-name">{fileName(result.source_path)}</span>
                                                </td>
                                                <td>{t(`opType.${result.operation}`)}</td>
                                                <td>
                                                    <code className="path-code">{result.source_path}</code>
                                                </td>
                                                <td>
                                                    <code className="path-code destination">
                                                        {result.destination_path}
                                                    </code>
                                                </td>
                                                <td>
                                                    <span className="rule-badge">{result.rule_name}</span>
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
                                                <td>{skipReasonText(t, s)}</td>
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
                        <h3 className="results-title">{t('preview.failedTitle')}</h3>
                        <div className="results-container">
                            <div className="results-table-container">
                                <table className="results-table">
                                    <thead>
                                        <tr>
                                            <th>{t('preview.colFile')}</th>
                                            <th>{t('preview.colFrom')}</th>
                                            <th>{t('preview.colTo')}</th>
                                            <th>{t('preview.colRule')}</th>
                                            <th>{t('preview.colError')}</th>
                                        </tr>
                                    </thead>
                                    <tbody>
                                        {executeResults.failed_operations.map((failure, index) => (
                                            <tr key={index} className="failure-row">
                                                <td>
                                                    <span className="file-name">{fileName(failure.source_path)}</span>
                                                </td>
                                                <td>
                                                    <code className="path-code">{failure.source_path}</code>
                                                </td>
                                                <td>
                                                    <code className="path-code">{failure.destination_path}</code>
                                                </td>
                                                <td>
                                                    <span className="rule-badge">{failure.rule_name}</span>
                                                </td>
                                                <td>
                                                    <span className="error-badge">
                                                        {journalErrorText(t, failure.error_message)}
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
                        <h3>{t('preview.emptyTitle')}</h3>
                        <p>{t('preview.emptyText')}</p>
                    </div>
                )}

                {!hasRun && !loading && !executeResults && (
                    <div className="placeholder-card">
                        <p>{t('preview.idleText')}</p>
                    </div>
                )}
            </div>
        </div>
    );
};

export default DryRunView;
