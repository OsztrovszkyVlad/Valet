import React, { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
    errorMessage,
    getBatch,
    listBatches,
    type BatchDetail,
    type BatchInfo,
    type OpState,
    type StateCounts,
} from '../api/commands';
import UndoPanel from '../components/UndoPanel';
import { formatDateTime, journalErrorText } from '../i18n/format';
import './HistoryView.css';

interface HistoryViewProps {
    /** The user closed an undo result for `batchId`. */
    onBatchUndone: (batchId: string, complete: boolean) => void;
    /** Changes when the journal changed elsewhere (a batch ran or was undone); triggers a reload. */
    reloadToken?: number;
}

const STATES: OpState[] = ['done', 'failed', 'rolled_back', 'in_progress', 'planned'];

const HistoryView: React.FC<HistoryViewProps> = ({ onBatchUndone, reloadToken }) => {
    const { t } = useTranslation();
    const [batches, setBatches] = useState<BatchInfo[]>([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState('');
    const [selectedId, setSelectedId] = useState<string | null>(null);
    const [detail, setDetail] = useState<BatchDetail | null>(null);
    const [detailError, setDetailError] = useState('');

    const loadBatches = useCallback(async () => {
        try {
            setBatches(await listBatches());
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

    useEffect(() => {
        void loadBatches();
    }, [loadBatches, reloadToken]);

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

    const batch = detail?.batch;
    const canUndo =
        batch !== undefined &&
        batch.kind === 'apply' &&
        batch.finished_at !== null &&
        batch.counts.done > 0;

    return (
        <div className="history-view">
            <div className="view-header history-header">
                <h2 className="view-title">{t('history.title')}</h2>
                <button className="btn-secondary" onClick={reload}>
                    {t('common.refresh')}
                </button>
            </div>

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
                                    onClick={() => select(b.id)}
                                >
                                    <td>
                                        {t(`batchKind.${b.kind}`, { defaultValue: b.kind })}
                                        {b.finished_at === null && (
                                            <span className="history-unfinished"> ({t('history.unfinished')})</span>
                                        )}
                                    </td>
                                    <td>{formatDateTime(b.created_at)}</td>
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
                            date: formatDateTime(batch.created_at),
                        })}
                    </h3>
                    <p className="history-batch-id">{t('history.batchId', { id: batch.id })}</p>

                    {batch.kind === 'apply' && (
                        <UndoPanel
                            key={batch.id}
                            batchId={batch.id}
                            canUndo={canUndo}
                            onUndoRan={reload}
                            onClose={(complete) => onBatchUndone(batch.id, complete)}
                        />
                    )}

                    {batch.summary && (
                        <>
                            <h4>{t('history.summaryTitle')}</h4>
                            <pre className="history-summary">{batch.summary}</pre>
                        </>
                    )}

                    {detail.operations.length === 0 ? (
                        <p>{t('history.noOperations')}</p>
                    ) : (
                        <div className="results-table-container">
                            <table className="results-table">
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
                                                <code className="path-code">{op.source_path}</code>
                                            </td>
                                            <td>
                                                <code className="path-code">{op.dest_path ?? ''}</code>
                                            </td>
                                            <td>{t(`opState.${op.state}`, { defaultValue: op.state })}</td>
                                            <td>{op.error ? journalErrorText(t, op.error) : ''}</td>
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
