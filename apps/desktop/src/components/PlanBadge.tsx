import React from 'react';
import { useTranslation } from 'react-i18next';
import type { PlanView } from '../api/commands';
import { daysLeft } from '../i18n/format';

interface PlanBadgeProps {
    plan: PlanView;
    onOpenPro: () => void;
}

/** Small plan indicator in the header: the trial days left, or "Free". Nothing on Pro. */
const PlanBadge: React.FC<PlanBadgeProps> = ({ plan, onOpenPro }) => {
    const { t } = useTranslation();
    const tier = plan.plan;
    if (tier.kind === 'pro') return null;
    const text =
        tier.kind === 'trial' ? t('nav.planTrial', { count: daysLeft(tier.ends_at) }) : t('nav.planFree');
    return (
        <button
            type="button"
            className="nav-plan"
            data-testid="nav-plan"
            data-plan={tier.kind}
            title={t('nav.planLinkTitle')}
            onClick={onOpenPro}
        >
            {text}
        </button>
    );
};

export default PlanBadge;
