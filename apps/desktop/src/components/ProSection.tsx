import React, { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
    checkLicenseNow,
    importLicenseFile,
    importLicenseText,
    openExternal,
    removeLicense,
    type PlanView,
} from '../api/commands';
import { commandErrorText, daysLeft, formatUnixDate, formatUnixDateTime } from '../i18n/format';
import './ProSection.css';

type Busy = 'import' | 'paste' | 'check' | 'remove' | 'buy' | null;

interface ProSectionProps {
    plan: PlanView;
    /** A command returned a new plan. */
    onPlanChange: (plan: PlanView) => void;
    /** The section element, so Settings can scroll to it. */
    sectionRef?: React.Ref<HTMLElement>;
}

/** Reasons shown elsewhere or obvious from the plan line. */
const HIDDEN_REASONS = new Set(['license.not_configured']);

/**
 * Settings → Valet Pro: the current plan, what Free and Pro include, buying,
 * importing a license (file or pasted text), checking and removing it.
 * The backend enforces the limits; this section only explains them.
 */
const ProSection: React.FC<ProSectionProps> = ({ plan, onPlanChange, sectionRef }) => {
    const { t, i18n } = useTranslation();
    const uid = useId();
    const [busy, setBusy] = useState<Busy>(null);
    const [error, setError] = useState('');
    const [notice, setNotice] = useState('');
    const [pasteOpen, setPasteOpen] = useState(false);
    const [pasteText, setPasteText] = useState('');
    const [confirmRemove, setConfirmRemove] = useState(false);

    const lng = i18n.language;
    const tier = plan.plan;
    const license = plan.license;
    const date = (s: number) => formatUnixDate(s, lng);

    const run = async (kind: NonNullable<Busy>, action: () => Promise<PlanView | null>, done?: (p: PlanView) => string) => {
        setBusy(kind);
        setError('');
        setNotice('');
        try {
            const next = await action();
            if (next !== null) {
                onPlanChange(next);
                if (done) setNotice(done(next));
            }
            return next;
        } catch (err) {
            setError(commandErrorText(t, err));
            return null;
        } finally {
            setBusy(null);
        }
    };

    const importedNotice = (p: PlanView) => (p.plan.kind === 'pro' ? t('pro.imported') : '');

    const importFile = () => void run('import', importLicenseFile, importedNotice);

    const submitPaste = async (e: React.FormEvent) => {
        e.preventDefault();
        const text = pasteText.trim();
        if (text === '') return;
        const next = await run('paste', () => importLicenseText(text), importedNotice);
        if (next !== null) {
            setPasteText('');
            setPasteOpen(false);
        }
    };

    const buy = async () => {
        setBusy('buy');
        setError('');
        try {
            await openExternal('checkout');
        } catch (err) {
            setError(commandErrorText(t, err));
        } finally {
            setBusy(null);
        }
    };

    const remove = async () => {
        const next = await run('remove', removeLicense, () => t('pro.removed'));
        if (next !== null) setConfirmRemove(false);
    };

    const planLine = () => {
        if (tier.kind === 'pro') {
            return (
                <>
                    <p data-testid="pro-plan-line">{t('pro.currentPro', { email: tier.email })}</p>
                    <p>{t('pro.updatesUntil', { date: date(tier.updates_until) })}</p>
                </>
            );
        }
        if (tier.kind === 'trial') {
            const endsAt = plan.trial_ends_at ?? tier.ends_at;
            return (
                <>
                    <p data-testid="pro-plan-line">
                        {t('pro.currentTrial', { count: daysLeft(endsAt), date: date(endsAt) })}
                    </p>
                    <p className="form-help">{t('pro.trialHelp')}</p>
                </>
            );
        }
        return <p data-testid="pro-plan-line">{t('pro.currentFree')}</p>;
    };

    const reasons = plan.reasons.filter(
        (r) => !HIDDEN_REASONS.has(r) && !(r === 'license.missing' && license === null),
    );
    const reasonText = (code: string) =>
        t(`license.reason.${code}`, {
            date: license ? date(license.updates_until) : '',
            defaultValue: code,
        });

    const anyBusy = busy !== null;
    const canImport = plan.licensing_configured && !anyBusy;
    const buyHelpId = `${uid}-buy-help`;

    return (
        <section
            className="settings-section pro-section"
            aria-labelledby={`${uid}-title`}
            data-testid="pro-section"
            ref={sectionRef}
            tabIndex={-1}
        >
            <h3 id={`${uid}-title`}>{t('pro.title')}</h3>
            {planLine()}

            {!plan.licensing_configured && (
                <p className="notice-message pro-note" role="note" data-testid="pro-not-configured">
                    {t('pro.notConfigured')}
                </p>
            )}

            {license !== null && tier.kind !== 'pro' && <p>{t('pro.licenseOf', { email: license.email })}</p>}

            {reasons.length > 0 && (
                <ul className="pro-reasons" data-testid="pro-reasons">
                    {reasons.map((r) => (
                        <li key={r}>{reasonText(r)}</li>
                    ))}
                </ul>
            )}

            <table className="pro-table">
                <caption>{t('pro.tableCaption')}</caption>
                <thead>
                    <tr>
                        <th scope="col">{t('pro.colFeature')}</th>
                        <th scope="col">{t('pro.colFree')}</th>
                        <th scope="col">{t('pro.colPro')}</th>
                    </tr>
                </thead>
                <tbody>
                    <tr>
                        <th scope="row">{t('pro.rowPresets')}</th>
                        <td>{t('pro.presetsFree')}</td>
                        <td>{t('pro.yes')}</td>
                    </tr>
                    <tr>
                        <th scope="row">{t('pro.rowRules')}</th>
                        <td>3</td>
                        <td>{t('pro.unlimited')}</td>
                    </tr>
                    <tr>
                        <th scope="row">{t('pro.rowFolders')}</th>
                        <td>2</td>
                        <td>{t('pro.unlimited')}</td>
                    </tr>
                    <tr>
                        <th scope="row">{t('pro.rowHistory')}</th>
                        <td>{t('pro.historyFree')}</td>
                        <td>{t('pro.historyPro')}</td>
                    </tr>
                    <tr>
                        <th scope="row">{t('pro.rowUndo')}</th>
                        <td>{t('pro.undoFree')}</td>
                        <td>{t('pro.undoPro')}</td>
                    </tr>
                    <tr>
                        <th scope="row">{t('pro.rowExport')}</th>
                        <td>{t('pro.no')}</td>
                        <td>{t('pro.yes')}</td>
                    </tr>
                    <tr>
                        <th scope="row">{t('pro.rowTypes')}</th>
                        <td>{t('pro.all')}</td>
                        <td>{t('pro.all')}</td>
                    </tr>
                </tbody>
            </table>

            <p className="pro-price" data-testid="pro-price">
                {t('pro.price')}
            </p>

            <div className="pro-actions">
                {tier.kind !== 'pro' && (
                    <button
                        type="button"
                        className="btn-primary"
                        data-testid="pro-buy"
                        disabled={!plan.checkout_available || anyBusy}
                        aria-describedby={plan.checkout_available ? undefined : buyHelpId}
                        onClick={() => void buy()}
                    >
                        {t('pro.buy')}
                    </button>
                )}
                <button
                    type="button"
                    className="btn-secondary"
                    data-testid="pro-import-file"
                    disabled={!canImport}
                    aria-describedby={`${uid}-import-help`}
                    onClick={importFile}
                >
                    {t('pro.importFile')}
                </button>
                <button
                    type="button"
                    className="btn-secondary"
                    data-testid="pro-paste-toggle"
                    aria-expanded={pasteOpen}
                    disabled={!canImport}
                    onClick={() => setPasteOpen((o) => !o)}
                >
                    {t('pro.pasteToggle')}
                </button>
            </div>
            {tier.kind !== 'pro' && plan.merchant !== null && (
                <p className="form-help" data-testid="pro-merchant">
                    {t('pro.merchant', { merchant: plan.merchant })}
                </p>
            )}
            {tier.kind !== 'pro' && !plan.checkout_available && (
                <p id={buyHelpId} className="form-help" data-testid="pro-buy-unavailable">
                    {t('pro.buyUnavailable')}
                </p>
            )}
            <p id={`${uid}-import-help`} className="form-help">
                {t('pro.importFileHelp')}
            </p>

            {pasteOpen && (
                <form className="pro-paste" onSubmit={(e) => void submitPaste(e)}>
                    <label htmlFor={`${uid}-paste`}>{t('pro.pasteLabel')}</label>
                    <textarea
                        id={`${uid}-paste`}
                        className="form-input pro-paste-text"
                        data-testid="pro-paste-text"
                        rows={5}
                        spellCheck={false}
                        value={pasteText}
                        aria-describedby={`${uid}-paste-help`}
                        onChange={(e) => setPasteText(e.target.value)}
                    />
                    <small id={`${uid}-paste-help`} className="form-help">
                        {t('pro.pasteHelp')}
                    </small>
                    <div>
                        <button
                            type="submit"
                            className="btn-primary"
                            data-testid="pro-paste-submit"
                            disabled={!canImport || pasteText.trim() === ''}
                        >
                            {t('pro.pasteSubmit')}
                        </button>
                    </div>
                </form>
            )}

            {license !== null && (
                <div className="pro-license" data-testid="pro-license">
                    <p className="form-help" data-testid="pro-license-id">
                        {t('pro.licenseId', { id: license.license_id })}
                    </p>
                    <p className="form-help">
                        {plan.last_check_at !== null
                            ? t('pro.lastCheck', { date: formatUnixDateTime(plan.last_check_at, lng) })
                            : t('pro.lastCheckNever')}
                    </p>
                    <div className="pro-actions">
                        <button
                            type="button"
                            className="btn-secondary"
                            data-testid="pro-check"
                            disabled={anyBusy || !plan.licensing_configured}
                            onClick={() => void run('check', checkLicenseNow, () => t('pro.checked'))}
                        >
                            {busy === 'check' ? t('pro.checking') : t('pro.checkNow')}
                        </button>
                        <button
                            type="button"
                            className="btn-secondary"
                            data-testid="pro-remove"
                            disabled={anyBusy || confirmRemove}
                            onClick={() => setConfirmRemove(true)}
                        >
                            {t('pro.remove')}
                        </button>
                    </div>
                    {confirmRemove && (
                        <div
                            className="notice-message pro-remove-confirm"
                            role="alertdialog"
                            aria-labelledby={`${uid}-remove-text`}
                        >
                            <span id={`${uid}-remove-text`}>{t('pro.removeConfirm')}</span>
                            <button
                                type="button"
                                className="btn-danger"
                                data-testid="pro-remove-yes"
                                disabled={anyBusy}
                                onClick={() => void remove()}
                            >
                                {t('pro.removeYes')}
                            </button>
                            <button
                                type="button"
                                className="btn-secondary"
                                disabled={anyBusy}
                                onClick={() => setConfirmRemove(false)}
                            >
                                {t('common.cancel')}
                            </button>
                        </div>
                    )}
                </div>
            )}

            <div role="status" className="pro-status">
                {busy === 'import' || busy === 'paste'
                    ? t('pro.importing')
                    : busy === 'remove'
                      ? t('pro.removing')
                      : notice}
            </div>
            {error && (
                <div className="error-message" role="alert">
                    {error}
                    <button type="button" className="error-close" onClick={() => setError('')} aria-label={t('common.close')}>
                        ×
                    </button>
                </div>
            )}
        </section>
    );
};

export default ProSection;
