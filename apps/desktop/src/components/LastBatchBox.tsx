import React from 'react';
import { useTranslation } from 'react-i18next';
import UndoPanel from './UndoPanel';
import { formatDateTime } from '../i18n/format';
import './LastBatchBox.css';

export interface LastBatch {
    id: string;
    createdAt: string;
}

interface LastBatchBoxProps {
    batch: LastBatch;
    /** False while another batch runs. */
    canUndo: boolean;
    onUndoRan?: () => void;
    /** The user closed the undo result; `complete`: nothing left to undo. */
    onClose: (complete: boolean) => void;
}

/** The undo box for the newest batch. Stays on screen until undone or replaced. */
const LastBatchBox: React.FC<LastBatchBoxProps> = ({ batch, canUndo, onUndoRan, onClose }) => {
    const { t } = useTranslation();
    return (
        <section className="last-batch" aria-label={t('lastBatch.title')}>
            <h2 className="last-batch-title">{t('lastBatch.title')}</h2>
            <p className="last-batch-date">
                {t('lastBatch.startedAt', { date: formatDateTime(batch.createdAt) })}
            </p>
            <UndoPanel
                key={batch.id}
                batchId={batch.id}
                canUndo={canUndo}
                onUndoRan={onUndoRan}
                onClose={onClose}
            />
        </section>
    );
};

export default LastBatchBox;
