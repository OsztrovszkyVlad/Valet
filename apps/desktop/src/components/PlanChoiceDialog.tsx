import React, { useEffect, useId, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { applyPlanChoice, getConfig, getRules, type PlanView, type Rule } from '../api/commands';
import { commandErrorMessage, commandErrorText } from '../i18n/format';
import { presetIdOf } from '../rules/model';
import './PlanChoiceDialog.css';

interface PlanChoiceDialogProps {
    plan: PlanView;
    /** `apply_plan_choice` returned the new plan. */
    onDone: (plan: PlanView) => void;
    /** Escape or "Decide later": the dialog comes back on the next start. */
    onLater: () => void;
}

const FOCUSABLE = 'button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [href]';

/**
 * Shown when the plan falls back to Free with more own rules or watched
 * folders than Free allows (`choice_pending`). The user picks which stay
 * active; nothing is deleted. Modal: focus stays inside, Escape = later.
 */
const PlanChoiceDialog: React.FC<PlanChoiceDialogProps> = ({ plan, onDone, onLater }) => {
    const { t } = useTranslation();
    const uid = useId();
    const ruleLimit = plan.limits.own_rules ?? 3;
    const folderLimit = plan.limits.folders ?? 2;
    const [ownRules, setOwnRules] = useState<Rule[] | null>(null);
    const [folders, setFolders] = useState<string[]>([]);
    const [pickedRules, setPickedRules] = useState<Set<string>>(new Set());
    const [pickedFolders, setPickedFolders] = useState<Set<string>>(new Set());
    const [loadError, setLoadError] = useState('');
    const [error, setError] = useState('');
    const [saving, setSaving] = useState(false);
    const dialogRef = useRef<HTMLDivElement>(null);

    useEffect(() => {
        let cancelled = false;
        Promise.all([getRules(), getConfig()])
            .then(([rules, config]) => {
                if (cancelled) return;
                const own = rules.filter((r) => presetIdOf(r) === null);
                setOwnRules(own);
                setPickedRules(new Set(own.filter((r) => r.enabled).slice(0, ruleLimit).map((r) => r.id)));
                setFolders(config.inbox_paths);
                setPickedFolders(new Set(config.inbox_paths.slice(0, folderLimit)));
            })
            .catch((err: unknown) => {
                if (!cancelled) setLoadError(t('planChoice.loadFailed', { message: commandErrorMessage(t, err) }));
            });
        return () => {
            cancelled = true;
        };
    }, [t, ruleLimit, folderLimit]);

    useEffect(() => {
        // Focus moves into the dialog and back to where it was when it closes.
        const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
        dialogRef.current?.focus();
        return () => previous?.focus();
    }, []);

    const onKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
        if (e.key === 'Escape') {
            e.preventDefault();
            if (!saving) onLater();
            return;
        }
        if (e.key !== 'Tab' || dialogRef.current === null) return;
        const items = [...dialogRef.current.querySelectorAll<HTMLElement>(FOCUSABLE)];
        if (items.length === 0) {
            e.preventDefault();
            return;
        }
        const first = items[0];
        const last = items[items.length - 1];
        const active = document.activeElement;
        if (e.shiftKey && (active === first || active === dialogRef.current)) {
            e.preventDefault();
            last.focus();
        } else if (!e.shiftKey && active === last) {
            e.preventDefault();
            first.focus();
        }
    };

    const toggle = (set: Set<string>, id: string, on: boolean) => {
        const next = new Set(set);
        if (on) next.add(id);
        else next.delete(id);
        return next;
    };

    const foldersNeeded = Math.min(folderLimit, folders.length);
    const showFolders = folders.length > folderLimit;
    const canSave = ownRules !== null && !saving && pickedFolders.size === foldersNeeded;

    const save = async () => {
        if (ownRules === null) return;
        setSaving(true);
        setError('');
        try {
            // Keep the rule order; folders in the order they are listed.
            const ruleIds = ownRules.filter((r) => pickedRules.has(r.id)).map((r) => r.id);
            const chosenFolders = folders.filter((f) => pickedFolders.has(f));
            onDone(await applyPlanChoice(ruleIds, chosenFolders));
        } catch (err) {
            setError(commandErrorText(t, err));
            setSaving(false);
        }
    };

    return (
        <div className="modal-backdrop">
            <div
                className="modal-dialog plan-choice"
                role="dialog"
                aria-modal="true"
                aria-labelledby={`${uid}-title`}
                aria-describedby={`${uid}-intro`}
                data-testid="plan-choice"
                ref={dialogRef}
                tabIndex={-1}
                onKeyDown={onKeyDown}
            >
                <h2 id={`${uid}-title`} className="modal-title">
                    {t('planChoice.title')}
                </h2>
                <p id={`${uid}-intro`}>{t('planChoice.intro')}</p>

                {loadError && (
                    <div className="error-message" role="alert">
                        {loadError}
                    </div>
                )}
                {ownRules === null && !loadError && <p>{t('common.loading')}</p>}

                {ownRules !== null && (
                    <>
                        <fieldset className="plan-choice-group">
                            <legend>
                                {t('planChoice.rulesTitle', { checked: pickedRules.size, limit: ruleLimit })}
                            </legend>
                            {ownRules.length === 0 ? (
                                <p>{t('planChoice.noRules')}</p>
                            ) : (
                                <ul className="plan-choice-list">
                                    {ownRules.map((r) => {
                                        const checked = pickedRules.has(r.id);
                                        return (
                                            <li key={r.id}>
                                                <label className="checkbox-label">
                                                    <input
                                                        type="checkbox"
                                                        className="form-checkbox"
                                                        data-testid={`plan-choice-rule-${r.id}`}
                                                        checked={checked}
                                                        disabled={saving || (!checked && pickedRules.size >= ruleLimit)}
                                                        onChange={(e) =>
                                                            setPickedRules((s) => toggle(s, r.id, e.target.checked))
                                                        }
                                                    />
                                                    <span className="checkbox-text">{r.name}</span>
                                                </label>
                                            </li>
                                        );
                                    })}
                                </ul>
                            )}
                            <small className="form-help">{t('planChoice.presetsNote')}</small>
                        </fieldset>

                        {showFolders && (
                            <fieldset className="plan-choice-group">
                                <legend>
                                    {t('planChoice.foldersTitle', {
                                        checked: pickedFolders.size,
                                        limit: folderLimit,
                                    })}
                                </legend>
                                <ul className="plan-choice-list">
                                    {folders.map((f) => {
                                        const checked = pickedFolders.has(f);
                                        return (
                                            <li key={f}>
                                                <label className="checkbox-label">
                                                    <input
                                                        type="checkbox"
                                                        className="form-checkbox"
                                                        data-testid="plan-choice-folder"
                                                        checked={checked}
                                                        disabled={
                                                            saving || (!checked && pickedFolders.size >= folderLimit)
                                                        }
                                                        onChange={(e) =>
                                                            setPickedFolders((s) => toggle(s, f, e.target.checked))
                                                        }
                                                    />
                                                    <code className="checkbox-text">{f}</code>
                                                </label>
                                            </li>
                                        );
                                    })}
                                </ul>
                                <small className="form-help">
                                    {t('planChoice.foldersHelp', { limit: folderLimit })}
                                </small>
                            </fieldset>
                        )}
                    </>
                )}

                {error && (
                    <div className="error-message" role="alert">
                        {error}
                    </div>
                )}

                <p className="form-help">{t('planChoice.laterHelp')}</p>
                <div className="modal-buttons">
                    <button
                        type="button"
                        className="btn-primary"
                        data-testid="plan-choice-save"
                        disabled={!canSave}
                        onClick={() => void save()}
                    >
                        {saving ? t('common.saving') : t('planChoice.save')}
                    </button>
                    <button
                        type="button"
                        className="btn-secondary"
                        data-testid="plan-choice-later"
                        disabled={saving}
                        onClick={onLater}
                    >
                        {t('planChoice.later')}
                    </button>
                </div>
            </div>
        </div>
    );
};

export default PlanChoiceDialog;
