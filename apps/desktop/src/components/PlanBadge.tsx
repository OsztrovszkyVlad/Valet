import React from 'react';
import { useTranslation } from 'react-i18next';
import type { PlanView } from '../api/commands';
import { daysLeft } from '../i18n/format';

interface PlanBadgeProps {
    plan: PlanView;
    onOpenPro: () => void;
}

/** Days before the trial end when the badge says what will change (Store policy 10.8.4, docs/STORE.md). */
const ENDING_DAYS = 3;

/** Small plan indicator in the header: the trial days left, or "Free". Nothing on Pro. */
const PlanBadge: React.FC<PlanBadgeProps> = ({ plan, onOpenPro }) => {
    const { t } = useTranslation();
    const tier = plan.plan;
    if (tier.kind === 'pro') return null;
    const left = tier.kind === 'trial' ? daysLeft(tier.ends_at) : 0;
    const ending = tier.kind === 'trial' && left <= ENDING_DAYS;
    const text =
        tier.kind === 'trial'
            ? t('nav.planTrial', { count: left }) + (ending ? ` · ${t('nav.planTrialEndingShort')}` : '')
            : t('nav.planFree');
    return (
        <button
            type="button"
            className="nav-plan"
            data-testid="nav-plan"
            data-plan={tier.kind}
            data-ending={ending ? 'true' : undefined}
            title={ending ? t('nav.planTrialEnding') : t('nav.planLinkTitle')}
            aria-description={ending ? t('nav.planTrialEnding') : undefined}
            onClick={onOpenPro}
        >
            {text}
        </button>
    );
};

export default PlanBadge;
