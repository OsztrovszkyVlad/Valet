import React from 'react';
import { useTranslation } from 'react-i18next';
import type { PlanLimitError } from '../api/commands';

interface PlanLimitNoticeProps {
    limit: PlanLimitError;
    /** Opens the Valet Pro section; the link is hidden without it. */
    onOpenPro?: () => void;
    onClose?: () => void;
}

/** A change refused by a Free plan limit (`plan.limit`), with a link to the Pro section. */
const PlanLimitNotice: React.FC<PlanLimitNoticeProps> = ({ limit, onOpenPro, onClose }) => {
    const { t } = useTranslation();
    return (
        <div className="error-message plan-limit-notice" role="alert" data-testid="plan-limit">
            <span>
                {limit.kind === 'folders'
                    ? t('limits.folders', { limit: limit.limit })
                    : t('limits.rules', { limit: limit.limit })}
            </span>
            {onOpenPro && (
                <button type="button" className="btn-secondary" onClick={onOpenPro}>
                    {t('limits.openPro')}
                </button>
            )}
            {onClose && (
                <button type="button" className="error-close" onClick={onClose} aria-label={t('common.close')}>
                    ×
                </button>
            )}
        </div>
    );
};

export default PlanLimitNotice;
