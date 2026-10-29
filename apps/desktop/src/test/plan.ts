// PlanView fixtures for tests.
import type { PlanView } from '../api/commands';

const DAY = 86_400;

/** Unix seconds `days` from now (negative: in the past). */
export function inDays(days: number): number {
    return Math.floor(Date.now() / 1000) + days * DAY;
}

const FREE_LIMITS = { own_rules: 3, folders: 2, history_days: 7, undo_latest_only: true, export_import: false };
const PRO_LIMITS = { own_rules: null, folders: null, history_days: null, undo_latest_only: false, export_import: true };

export function freePlan(overrides: Partial<PlanView> = {}): PlanView {
    return {
        plan: { kind: 'free' },
        reasons: ['license.missing'],
        limits: FREE_LIMITS,
        licensing_configured: true,
        checkout_available: true,
        choice_pending: false,
        license: null,
        trial_ends_at: inDays(-1),
        last_check_at: null,
        packaged: false,
        ...overrides,
    };
}

export function trialPlan(daysLeft: number, overrides: Partial<PlanView> = {}): PlanView {
    // Half a day less, so "days left" rounds up to `daysLeft`.
    const endsAt = inDays(daysLeft) - DAY / 2;
    return {
        ...freePlan(),
        plan: { kind: 'trial', ends_at: endsAt },
        limits: PRO_LIMITS,
        trial_ends_at: endsAt,
        ...overrides,
    };
}

export const LICENSE = {
    license_id: '6f1c2a9e-1111-4222-8333-444455556666',
    email: 'ana@example.com',
    updates_until: Date.UTC(2027, 9, 29) / 1000,
};

export function proPlan(overrides: Partial<PlanView> = {}): PlanView {
    return {
        ...freePlan(),
        plan: { kind: 'pro', ...LICENSE },
        reasons: [],
        limits: PRO_LIMITS,
        license: LICENSE,
        last_check_at: Date.UTC(2026, 9, 20, 9, 30) / 1000,
        ...overrides,
    };
}
