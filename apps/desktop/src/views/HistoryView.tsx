import React, { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
    errorMessage,
    getBatch,
    listBatches,
    type BatchDetail,
    type BatchInfo,
    type OpState,
    type PlanView,
    type StateCounts,
} from '../api/commands';
import PathText from '../components/PathText';
import UndoPanel from '../components/UndoPanel';
import { formatDateTime, journalErrorText } from '../i18n/format';
import './HistoryView.css';

interface HistoryViewProps {
    /** The user closed an undo result for `batchId`. */
    onBatchUndone: (batchId: string, complete: boolean) => void;
    /** Changes when the journal changed elsewhere (a batch ran or was undone); triggers a reload. */
    reloadToken?: number;
    /** The plan; on Free only the most recent batch can be undone. Null or absent: no plan limits shown. */
    plan?: PlanView | null;
}

const STATES: OpState[] = ['done', 'failed', 'rolled_back', 'in_progress', 'planned'];

const HistoryView: React.FC<HistoryViewProps> = ({ onBatchUndone, reloadToken, plan = null }) => {
    const { t, i18n } = useTranslation();
    const [batches, setBatches] = useState<BatchInfo[]>([]);
    // The Free plan hid batches older than 7 days.
    const [hiddenOlder, setHiddenOlder] = useState(false);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState('');
    const [selectedId, setSelectedId] = useState<string | null>(null);
    const [detail, setDetail] = useState<BatchDetail | null>(null);
    const [detailError, setDetailError] = useState('');

    const loadBatches = useCallback(async () => {
        try {
            const list = await listBatches();
            setBatches(list.batches);
            setHiddenOlder(list.hidden_older);
            setError('');
        } catch (err) {
            setError(t('history.loadFailed', { message: errorMessage(err) }));
        } finally {
            setLoading(false);
        }
    }, [t]);

    const loadDetail = useCallback(
        async (batchId: string) => {
            try {
                setDetail(await getBatch(batchId));
                setDetailError('');
            } catch (err) {
                setDetailError(t('history.loadFailed', { message: errorMessage(err) }));
            }
        },
        [t],
    );

    // The plan decides which batches are shown.
    const planKind = plan?.plan.kind;
    useEffect(() => {
        void loadBatches();
    }, [loadBatches, reloadToken, planKind]);

    // The open batch, read by the reload below without re-running it on selection.
    const selectedRef = useRef<string | null>(null);
    useEffect(() => {
        selectedRef.current = selectedId;
    }, [selectedId]);

    useEffect(() => {
        if (selectedRef.current !== null) {
            void loadDetail(selectedRef.current);
        }
    }, [loadDetail, reloadToken]);

    const select = (batchId: string) => {
        setSelectedId(batchId);
        setDetail(null);
        setDetailError('');
        void loadDetail(batchId);
    };

    const reload = () => {
        void loadBatches();
        if (selectedId) {
            void loadDetail(selectedId);
        }
    };

    const countsText = (counts: StateCounts) =>
        STATES.filter((s) => counts[s] > 0)
            .map((s) => t('history.count', { state: t(`opState.${s}`), count: counts[s] }))
            .join(', ');

    /** What the batch did, in the UI language, from its counts. */
    const summaryText = (b: BatchInfo): string => {
        if (b.kind === 'undo') {
            return t('history.summaryUndo', { done: b.counts.done, failed: b.counts.failed });
        }
        // Rolled back operations were done first, then undone by a later batch.
        const parts = [
            t('history.summaryApply', { done: b.counts.done + b.counts.rolled_back, failed: b.counts.failed }),
        ];
        if (b.counts.rolled_back > 0) {
            parts.push(t('history.summaryUndoneLater', { count: b.counts.rolled_back }));
        }
        return parts.join(' ');
    };

    const batch = detail?.batch;
    const canUndo =
        batch !== undefined &&
        batch.kind === 'apply' &&
        batch.finished_at !== null &&
        batch.counts.done > 0;
    // Free: the backend undoes only the most recent batch (`plan.undoLatestOnly`).
    const undoBlocked =
        batch !== undefined && plan?.limits.undo_latest_only === true && batches[0]?.id !== batch.id;

    return (
        <div className="history-view">
            <div className="view-header history-header">
                <h2 className="view-title">{t('history.title')}</h2>
                <button className="btn-secondary" onClick={reload}>
                    {t('common.refresh')}
                </button>
            </div>

            {hiddenOlder && (
                <p className="notice-message history-hidden-older" role="note" data-testid="history-hidden-older">
                    {t('history.hiddenOlder')}
                </p>
            )}
            {error && <p className="history-error">{error}</p>}
            {loading && <p>{t('common.loading')}</p>}
            {!loading && !error && batches.length === 0 && (
                <div className="empty-state">
                    <h3>{t('history.emptyTitle')}</h3>
                    <p>{t('history.emptyText')}</p>
                </div>
            )}

            {batches.length > 0 && (
                <div className="results-table-container history-list">
                    <table className="results-table">
                        <colgroup>
                            <col className="col-history-kind" />
                            <col className="col-history-date" />
                            <col />
                        </colgroup>
                        <thead>
                            <tr>
                                <th>{t('history.colKind')}</th>
                                <th>{t('history.colDate')}</th>
                                <th>{t('history.colCounts')}</th>
                            </tr>
                        </thead>
                        <tbody>
                            {batches.map((b) => (
                                <tr
                                    key={b.id}
                                    className={b.id === selectedId ? 'history-row selected' : 'history-row'}
                                    data-testid="history-row"
                                    data-kind={b.kind}
                                    onClick={() => select(b.id)}
                                >
                                    <td>
                                        {t(`batchKind.${b.kind}`, { defaultValue: b.kind })}
                                        {b.finished_at === null && (
                                            <span className="history-unfinished"> ({t('history.unfinished')})</span>
                                        )}
                                    </td>
                                    <td>{formatDateTime(b.created_at, i18n.language)}</td>
                                    <td>{countsText(b.counts)}</td>
                                </tr>
                            ))}
                        </tbody>
                    </table>
                </div>
            )}

            {batches.length > 0 && selectedId === null && <p>{t('history.selectHint')}</p>}
            {detailError && <p className="history-error">{detailError}</p>}
            {selectedId !== null && !detail && !detailError && <p>{t('common.loading')}</p>}

            {detail && batch && (
                <div className="history-detail">
                    <h3 className="results-title">
                        {t('history.detailTitle', {
                            kind: t(`batchKind.${batch.kind}`, { defaultValue: batch.kind }),
                            date: formatDateTime(batch.created_at, i18n.language),
                        })}
                    </h3>
                    <p className="history-batch-id">{t('history.batchId', { id: batch.id })}</p>

                    {batch.kind === 'apply' && (
                        <UndoPanel
                            key={batch.id}
                            batchId={batch.id}
                            canUndo={canUndo}
                            blockedReason={undoBlocked ? t('undo.latestOnly') : undefined}
                            onUndoRan={reload}
                            onClose={(complete) => onBatchUndone(batch.id, complete)}
                        />
                    )}

                    {batch.kind === 'recovery' ? (
                        <>
                            <h4>{t('history.summaryTitle')}</h4>
                            <p className="history-summary-text">{t('history.summaryRecovery')}</p>
                            {batch.summary && (
                                <details className="history-technical">
                                    <summary>{t('history.technicalDetails')}</summary>
                                    <pre className="history-summary">{batch.summary}</pre>
                                </details>
                            )}
                        </>
                    ) : (
                        batch.finished_at !== null && (
                            <>
                                <h4>{t('history.summaryTitle')}</h4>
                                <p className="history-summary-text" data-testid="history-summary">
                                    {summaryText(batch)}
                                    {batch.summary?.startsWith('interrupted') && (
                                        <> {t('history.summaryInterrupted')}</>
                                    )}
                                </p>
                            </>
                        )
                    )}

                    {detail.operations.length === 0 ? (
                        <p>{t('history.noOperations')}</p>
                    ) : (
                        <div className="results-table-container">
                            <table className="results-table">
                                <colgroup>
                                    <col className="col-history-id" />
                                    <col className="col-history-type" />
                                    <col className="col-history-path" />
                                    <col className="col-history-path" />
                                    <col className="col-history-state" />
                                    <col className="col-history-error" />
                                </colgroup>
                                <thead>
                                    <tr>
                                        <th>{t('history.colId')}</th>
                                        <th>{t('history.colType')}</th>
                                        <th>{t('history.colFrom')}</th>
                                        <th>{t('history.colTo')}</th>
                                        <th>{t('history.colState')}</th>
                                        <th>{t('history.colError')}</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {detail.operations.map((op) => (
                                        <tr key={op.id}>
                                            <td>{op.id}</td>
                                            <td>{t(`historyOpType.${op.op_type}`, { defaultValue: op.op_type })}</td>
                                            <td>
                                                <PathText path={op.source_path} />
                                            </td>
                                            <td>{op.dest_path ? <PathText path={op.dest_path} /> : ''}</td>
                                            <td className="cell-wrap">
                                                {t(`opState.${op.state}`, { defaultValue: op.state })}
                                            </td>
                                            <td className="cell-wrap">{op.error ? journalErrorText(t, op.error) : ''}</td>
                                        </tr>
                                    ))}
                                </tbody>
                            </table>
                        </div>
                    )}
                </div>
            )}
        </div>
    );
};

export default HistoryView;
