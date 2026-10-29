import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';
import '../i18n';
import PlanBadge from './PlanBadge';
import { freePlan, proPlan, trialPlan } from '../test/plan';

const ENDING_NOTE =
    'Ce se schimbă la sfârșitul probei: regulile proprii peste 3 și folderele urmărite peste 2 devin inactive, nu se șterg. Alegi tu care rămân active, iar cu Pro revin toate.';

describe('PlanBadge', () => {
    it('shows only the days left early in the trial', () => {
        render(<PlanBadge plan={trialPlan(10)} onOpenPro={() => {}} />);
        const badge = screen.getByTestId('nav-plan');
        expect(badge.textContent).toBe('Probă Pro: 10 zile');
        expect(badge.getAttribute('data-ending')).toBeNull();
        expect(badge.getAttribute('title')).toBe('Deschide secțiunea Valet Pro din Setări');
    });

    it('says what will change in the last 3 days of the trial', () => {
        render(<PlanBadge plan={trialPlan(3)} onOpenPro={() => {}} />);
        const badge = screen.getByTestId('nav-plan');
        expect(badge.textContent).toBe('Probă Pro: 3 zile · Proba se termină curând');
        expect(badge.getAttribute('data-ending')).toBe('true');
        expect(badge.getAttribute('title')).toBe(ENDING_NOTE);
    });

    it('shows Free on Free and nothing on Pro', () => {
        const { unmount } = render(<PlanBadge plan={freePlan()} onOpenPro={() => {}} />);
        expect(screen.getByTestId('nav-plan').textContent).toBe('Gratuit');
        unmount();
        render(<PlanBadge plan={proPlan()} onOpenPro={() => {}} />);
        expect(screen.queryByTestId('nav-plan')).toBeNull();
    });
});
