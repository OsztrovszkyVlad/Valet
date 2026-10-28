import React, { useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
    getBatch,
    undoBatch,
    type OpRecord,
    type UndoReport,
} from '../api/commands';
import { opErrorText } from '../i18n/format';
import './UndoPanel.css';

interface UndoPanelProps {
    batchId: string;
    /** Show the start button. The result stays visible either way. */
    canUndo: boolean;
    /** Called after each undo call returns, so the caller can reload. */
    onUndoRan?: () => void;
    /** Called when the user closes the result. `complete`: nothing left to confirm or retry. */
    onClose?: (complete: boolean) => void;
}

type Step =
    | { kind: 'idle' }
    | { kind: 'busy' }
    | { kind: 'preview'; ops: OpRecord[] }
    | { kind: 'confirm'; first: UndoReport }
    | { kind: 'result'; first: UndoReport; forced: UndoReport | null };

function errorMessage(err: unknown): string {
    return typeof err === 'string' ? err : err instanceof Error ? err.message : String(err);
}

/**
 * Undo of one apply batch: preview of the files that go back, confirm,
 * then a second confirm for files that changed or whose place is taken.
 */
const UndoPanel: React.FC<UndoPanelProps> = ({ batchId, canUndo, onUndoRan, onClose }) => {
    const { t } = useTranslation();
    const [step, setStep] = useState<Step>({ kind: 'idle' });
    const [error, setError] = useState('');

    const loadPreview = async () => {
        setError('');
        setStep({ kind: 'busy' });
        try {
            const detail = await getBatch(batchId);
            setStep({
                kind: 'preview',
                ops: detail.operations.filter((op) => op.state === 'done'),
            });
        } catch (err) {
            setError(t('undo.failedToStart', { message: errorMessage(err) }));
            setStep({ kind: 'idle' });
        }
    };

    const runUndo = async (force: boolean, first: UndoReport | null) => {
        setError('');
        setStep({ kind: 'busy' });
        try {
            const report = await undoBatch(batchId, force);
            onUndoRan?.();
            if (first === null && report.needs_confirmation.length > 0) {
                setStep({ kind: 'confirm', first: report });
            } else if (first === null) {
                setStep({ kind: 'result', first: report, forced: null });
            } else {
                setStep({ kind: 'result', first, forced: report });
            }
        } catch (err) {
            setError(t('undo.failedToStart', { message: errorMessage(err) }));
            setStep(first === null ? { kind: 'idle' } : { kind: 'confirm', first });
        }
    };

    const close = (complete: boolean) => {
        setStep({ kind: 'idle' });
        onClose?.(complete);
    };

    const renderResult = (first: UndoReport, forced: UndoReport | null) => {
        // Failed operations stay `done` and are retried by the forced call,
        // so failures and pending files come from the last call only.
        const last = forced ?? first;
        const undone = first.undone.length + (forced?.undone.length ?? 0);
        const restoredAs = [...first.restored_as, ...(forced?.restored_as ?? [])];
        const complete = last.failed.length === 0 && last.needs_confirmation.length === 0;
        const sourceOf = (opId: number) =>
            [...first.needs_confirmation, ...last.failed].find((x) => x.op_id === opId)?.source;
        return (
            <div className="undo-result">
                <h4>{t('undo.resultTitle')}</h4>
                <p>{t('undo.undone', { count: undone })}</p>
                {restoredAs.length > 0 && (
                    <ul className="undo-list">
                        {restoredAs.map((r) => (
                            <li key={r.op_id}>
                                {t('undo.restoredAs', {
                                    original: sourceOf(r.op_id) ?? '',
                                    path: r.path,
                                })}
                            </li>
                        ))}
                    </ul>
                )}
                {last.needs_confirmation.length > 0 && (
                    <p>{t('undo.pending', { count: last.needs_confirmation.length })}</p>
                )}
                {first.skipped.length > 0 && (
                    <p>{t('undo.skipped', { count: first.skipped.length })}</p>
                )}
                {last.failed.length > 0 && (
                    <>
                        <p className="undo-failed">{t('undo.failed', { count: last.failed.length })}</p>
                        <ul className="undo-list">
                            {last.failed.map((f) => (
                                <li key={f.op_id}>
                                    <code>{f.dest ?? f.source}</code>: {opErrorText(t, f.error)}
                                </li>
                            ))}
                        </ul>
                    </>
                )}
                <button className="btn-secondary" onClick={() => close(complete)}>
                    {t('common.close')}
                </button>
            </div>
        );
    };

    return (
        <div className="undo-panel">
            {error && <p className="undo-error">{error}</p>}

            {step.kind === 'idle' && canUndo && (
                <button className="btn-undo" onClick={loadPreview}>
                    {t('undo.button')}
                </button>
            )}

            {step.kind === 'busy' && <p>{t('undo.running')}</p>}

            {step.kind === 'preview' && (
                <div className="undo-preview">
                    {step.ops.length === 0 ? (
                        <p>{t('undo.previewNothing')}</p>
                    ) : (
                        <>
                            <h4>{t('undo.previewTitle', { count: step.ops.length })}</h4>
                            <ul className="undo-list">
                                {step.ops.map((op) => (
                                    <li key={op.id}>
                                        {t(`opType.${op.op_type}`)}: <code>{op.dest_path ?? ''}</code> →{' '}
                                        <code>{op.source_path}</code>
                                    </li>
                                ))}
                            </ul>
                        </>
                    )}
                    <div className="undo-actions">
                        {step.ops.length > 0 && (
                            <button className="btn-undo" onClick={() => runUndo(false, null)}>
                                {t('undo.confirm')}
                            </button>
                        )}
                        <button className="btn-secondary" onClick={() => setStep({ kind: 'idle' })}>
                            {t('common.cancel')}
                        </button>
                    </div>
                </div>
            )}

            {step.kind === 'confirm' && (
                <div className="undo-confirm">
                    <p>{t('undo.undone', { count: step.first.undone.length })}</p>
                    <h4>
                        {t('undo.needsConfirmationTitle', {
                            count: step.first.needs_confirmation.length,
                        })}
                    </h4>
                    <ul className="undo-list">
                        {step.first.needs_confirmation.map((n) => (
                            <li key={n.op_id}>
                                <code>{n.dest ?? n.source}</code>
                                <ul>
                                    {n.reasons.map((r) => (
                                        <li key={r}>{t(`confirmReason.${r}`, { defaultValue: r })}</li>
                                    ))}
                                </ul>
                            </li>
                        ))}
                    </ul>
                    <div className="undo-actions">
                        <button className="btn-undo" onClick={() => runUndo(true, step.first)}>
                            {t('undo.force')}
                        </button>
                        <button
                            className="btn-secondary"
                            onClick={() => setStep({ kind: 'result', first: step.first, forced: null })}
                        >
                            {t('common.cancel')}
                        </button>
                    </div>
                </div>
            )}

            {step.kind === 'result' && renderResult(step.first, step.forced)}
        </div>
    );
};

export default UndoPanel;
