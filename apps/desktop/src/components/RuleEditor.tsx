import React, { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
    ACTION_TYPES,
    CONDITION_TYPES,
    TYPE_GROUPS,
    parseCommandError,
    upsertRule,
    type ActionType,
    type ConditionType,
    type Rule,
    type TypeGroup,
} from '../api/commands';
import { commandErrorText } from '../i18n/format';
import {
    SIZE_UNITS,
    defaultCondition,
    draftToRule,
    hasErrors,
    normalizeExt,
    type ActionDraft,
    type ConditionDraft,
    type DraftErrors,
    type RuleDraft,
    type SizeUnit,
} from '../rules/model';
import { actionSummary, displayPath } from '../rules/summary';
import ChipInput from './ChipInput';

/** Tokens offered under the destination field. */
const DEST_TOKENS = ['downloads', 'documents', 'pictures', 'desktop', 'home', 'year', 'month', 'day', 'ext'] as const;

const REGEX_ERROR = 'rule.invalidRegex';

interface RuleEditorProps {
    initial: RuleDraft;
    isNew: boolean;
    /** Watched folders from the config, offered as scope. */
    folders: string[];
    onSaved: (rule: Rule, isNew: boolean) => void;
    onCancel: () => void;
}

function defaultAction(type: ActionType, from: ActionDraft): ActionDraft {
    if (type === 'moveTo' || type === 'copyTo') {
        return { type, path: 'path' in from ? from.path : '' };
    }
    return { type };
}

/** Create or edit one rule. Saving calls `upsert_rule`; no file is touched. */
const RuleEditor: React.FC<RuleEditorProps> = ({ initial, isNew, folders, onSaved, onCancel }) => {
    const { t } = useTranslation();
    const uid = useId();
    const [draft, setDraft] = useState<RuleDraft>(initial);
    const [errors, setErrors] = useState<DraftErrors>({ conditions: {} });
    const [saveError, setSaveError] = useState('');
    const [regexError, setRegexError] = useState('');
    const [saving, setSaving] = useState(false);

    const patch = (p: Partial<RuleDraft>) => setDraft((d) => ({ ...d, ...p }));

    const setCondition = (i: number, c: ConditionDraft) =>
        setDraft((d) => ({ ...d, conditions: d.conditions.map((x, j) => (j === i ? c : x)) }));

    const removeCondition = (i: number) =>
        setDraft((d) => ({ ...d, conditions: d.conditions.filter((_, j) => j !== i) }));

    const addCondition = () =>
        setDraft((d) => ({ ...d, conditions: [...d.conditions, defaultCondition('nameContains')] }));

    const toggleScope = (entry: string, on: boolean) =>
        setDraft((d) => ({
            ...d,
            scope: on ? [...d.scope, entry] : d.scope.filter((e) => e !== entry),
        }));

    // Watched folders plus scope entries that are not among them (tokens from presets).
    const scopeChoices = [
        ...folders,
        ...draft.scope.filter((e) => !folders.some((f) => f.toLowerCase() === e.toLowerCase())),
    ];

    const save = async (e: React.FormEvent) => {
        e.preventDefault();
        setSaveError('');
        setRegexError('');
        const result = draftToRule(draft);
        if (!result.ok) {
            setErrors(result.errors);
            return;
        }
        setErrors({ conditions: {} });
        setSaving(true);
        try {
            const saved = await upsertRule(result.rule);
            onSaved(saved, isNew);
        } catch (err) {
            if (parseCommandError(err)?.code === REGEX_ERROR) {
                setRegexError(commandErrorText(t, err));
            } else {
                setSaveError(commandErrorText(t, err));
            }
        } finally {
            setSaving(false);
        }
    };

    const conditionInput = (c: ConditionDraft, i: number) => {
        const id = `${uid}-c${i}`;
        const errId = `${id}-err`;
        const invalid = errors.conditions[i] !== undefined || (c.type === 'nameMatches' && regexError !== '');
        const describedBy = invalid ? errId : undefined;
        switch (c.type) {
            case 'ext':
            case 'extIn':
            case 'extNotIn':
                return (
                    <ChipInput
                        id={`${id}-v`}
                        label={t('ruleEditor.extInput')}
                        placeholder={t('ruleEditor.extPlaceholder')}
                        values={c.exts}
                        normalize={normalizeExt}
                        max={c.type === 'ext' ? 1 : undefined}
                        invalid={invalid}
                        describedBy={describedBy}
                        onChange={(exts) => setCondition(i, { ...c, exts })}
                    />
                );
            case 'nameContains':
                return (
                    <ChipInput
                        id={`${id}-v`}
                        label={t('ruleEditor.wordInput')}
                        placeholder={t('ruleEditor.wordPlaceholder')}
                        values={c.words}
                        invalid={invalid}
                        describedBy={describedBy}
                        onChange={(words) => setCondition(i, { ...c, words })}
                    />
                );
            case 'nameMatches':
            case 'pathContains':
                return (
                    <>
                        <label htmlFor={`${id}-v`} className="visually-hidden">
                            {c.type === 'nameMatches' ? t('ruleEditor.regexInput') : t('ruleEditor.pathInput')}
                        </label>
                        <input
                            id={`${id}-v`}
                            className="form-input"
                            type="text"
                            value={c.text}
                            placeholder={c.type === 'nameMatches' ? t('ruleEditor.regexPlaceholder') : ''}
                            aria-invalid={invalid || undefined}
                            aria-describedby={describedBy}
                            onChange={(e) => {
                                if (c.type === 'nameMatches') setRegexError('');
                                setCondition(i, { ...c, text: e.target.value });
                            }}
                        />
                    </>
                );
            case 'sizeGt':
            case 'sizeLt':
                return (
                    <div className="inline-fields">
                        <label htmlFor={`${id}-v`} className="visually-hidden">
                            {t('ruleEditor.sizeAmount')}
                        </label>
                        <input
                            id={`${id}-v`}
                            className="form-input input-number"
                            type="text"
                            inputMode="decimal"
                            value={c.amount}
                            aria-invalid={invalid || undefined}
                            aria-describedby={describedBy}
                            onChange={(e) => setCondition(i, { ...c, amount: e.target.value })}
                        />
                        <label htmlFor={`${id}-u`} className="visually-hidden">
                            {t('ruleEditor.sizeUnit')}
                        </label>
                        <select
                            id={`${id}-u`}
                            className="form-input input-unit"
                            value={c.unit}
                            onChange={(e) => setCondition(i, { ...c, unit: e.target.value as SizeUnit })}
                        >
                            {SIZE_UNITS.map((u) => (
                                <option key={u} value={u}>
                                    {u}
                                </option>
                            ))}
                        </select>
                    </div>
                );
            case 'modifiedOlderThanDays':
            case 'createdOlderThanDays':
                return (
                    <>
                        <label htmlFor={`${id}-v`} className="visually-hidden">
                            {t('ruleEditor.daysInput')}
                        </label>
                        <input
                            id={`${id}-v`}
                            className="form-input input-number"
                            type="text"
                            inputMode="numeric"
                            value={c.days}
                            aria-invalid={invalid || undefined}
                            aria-describedby={describedBy}
                            onChange={(e) => setCondition(i, { ...c, days: e.target.value })}
                        />
                    </>
                );
            case 'typeGroup':
                return (
                    <>
                        <label htmlFor={`${id}-v`} className="visually-hidden">
                            {t('ruleEditor.groupInput')}
                        </label>
                        <select
                            id={`${id}-v`}
                            className="form-input"
                            value={c.group}
                            onChange={(e) => setCondition(i, { ...c, group: e.target.value as TypeGroup })}
                        >
                            {TYPE_GROUPS.map((g) => (
                                <option key={g} value={g}>
                                    {t(`typeGroup.${g}`)}
                                </option>
                            ))}
                        </select>
                    </>
                );
            case 'emptyFolder':
            case 'duplicate':
                return <p className="form-help">{t(`conditionHelp.${c.type}`)}</p>;
        }
    };

    const action = draft.action;

    return (
        <form className="rule-editor" onSubmit={(e) => void save(e)} noValidate>
            <h3 className="results-title">{isNew ? t('ruleEditor.newTitle') : t('ruleEditor.editTitle')}</h3>

            <div className="form-group">
                <label htmlFor={`${uid}-name`}>{t('ruleEditor.name')}</label>
                <input
                    id={`${uid}-name`}
                    className="form-input"
                    type="text"
                    value={draft.name}
                    placeholder={t('ruleEditor.namePlaceholder')}
                    aria-invalid={errors.name !== undefined || undefined}
                    aria-describedby={errors.name ? `${uid}-name-err` : undefined}
                    onChange={(e) => patch({ name: e.target.value })}
                />
                {errors.name && (
                    <small id={`${uid}-name-err`} className="field-error">
                        {t(`editorError.${errors.name}`)}
                    </small>
                )}
            </div>

            <div className="form-group">
                <label className="checkbox-label">
                    <input
                        type="checkbox"
                        className="form-checkbox"
                        checked={draft.enabled}
                        onChange={(e) => patch({ enabled: e.target.checked })}
                    />
                    <span className="checkbox-text">{t('ruleEditor.enabled')}</span>
                </label>
            </div>

            <fieldset className="editor-section">
                <legend>{t('ruleEditor.conditionsTitle')}</legend>
                <label className="checkbox-label">
                    <input
                        type="checkbox"
                        className="form-checkbox"
                        checked={draft.alwaysApply}
                        onChange={(e) => patch({ alwaysApply: e.target.checked })}
                    />
                    <span className="checkbox-text">{t('ruleEditor.alwaysApply')}</span>
                </label>
                {!draft.alwaysApply && (
                    <>
                        <p className="form-help">{t('ruleEditor.conditionsHelp')}</p>
                        <ul className="condition-list">
                            {draft.conditions.map((c, i) => {
                                const errKey = errors.conditions[i];
                                const showRegex = c.type === 'nameMatches' && regexError !== '';
                                return (
                                    <li key={i} className="condition-row">
                                        <label htmlFor={`${uid}-c${i}-type`} className="visually-hidden">
                                            {t('ruleEditor.conditionType')}
                                        </label>
                                        <select
                                            id={`${uid}-c${i}-type`}
                                            className="form-input condition-type"
                                            value={c.type}
                                            onChange={(e) =>
                                                setCondition(i, defaultCondition(e.target.value as ConditionType, c))
                                            }
                                        >
                                            {CONDITION_TYPES.map((type) => (
                                                <option key={type} value={type}>
                                                    {t(`conditionLabel.${type}`)}
                                                </option>
                                            ))}
                                        </select>
                                        <div className="condition-value">
                                            {conditionInput(c, i)}
                                            {(errKey || showRegex) && (
                                                <small id={`${uid}-c${i}-err`} className="field-error">
                                                    {errKey ? t(`editorError.${errKey}`) : regexError}
                                                </small>
                                            )}
                                        </div>
                                        <button
                                            type="button"
                                            className="btn-secondary"
                                            onClick={() => removeCondition(i)}
                                        >
                                            {t('ruleEditor.removeCondition')}
                                        </button>
                                    </li>
                                );
                            })}
                        </ul>
                        {errors.general && <p className="field-error">{t(`editorError.${errors.general}`)}</p>}
                        <button type="button" className="btn-secondary" onClick={addCondition}>
                            {t('ruleEditor.addCondition')}
                        </button>
                    </>
                )}
            </fieldset>

            <fieldset className="editor-section">
                <legend>{t('ruleEditor.actionTitle')}</legend>
                <div className="form-group">
                    <label htmlFor={`${uid}-action`}>{t('ruleEditor.actionType')}</label>
                    <select
                        id={`${uid}-action`}
                        className="form-input"
                        value={action.type}
                        onChange={(e) => patch({ action: defaultAction(e.target.value as ActionType, action) })}
                    >
                        {ACTION_TYPES.map((type) => (
                            <option key={type} value={type}>
                                {t(`actionLabel.${type}`)}
                            </option>
                        ))}
                    </select>
                </div>
                {(action.type === 'moveTo' || action.type === 'copyTo') && (
                    <div className="form-group">
                        <label htmlFor={`${uid}-dest`}>{t('ruleEditor.destination')}</label>
                        <input
                            id={`${uid}-dest`}
                            className="form-input"
                            type="text"
                            value={action.path}
                            placeholder={t('ruleEditor.destinationPlaceholder')}
                            aria-invalid={errors.action !== undefined || undefined}
                            aria-describedby={`${uid}-dest-help`}
                            onChange={(e) => patch({ action: { ...action, path: e.target.value } })}
                        />
                        {errors.action && <small className="field-error">{t(`editorError.${errors.action}`)}</small>}
                        <small id={`${uid}-dest-help`} className="form-help">
                            {t('ruleEditor.destinationHelp')}
                        </small>
                        <p className="form-help">{t('ruleEditor.tokensTitle')}</p>
                        <ul className="token-list">
                            {DEST_TOKENS.map((token) => (
                                <li key={token}>
                                    <button
                                        type="button"
                                        className="token-chip"
                                        aria-label={t('ruleEditor.tokenAdd', { token: `{${token}}` })}
                                        onClick={() => patch({ action: { ...action, path: action.path + `{${token}}` } })}
                                    >
                                        <code>{`{${token}}`}</code>
                                    </button>{' '}
                                    <span className="form-help-inline">{t(`tokenHelp.${token}`)}</span>
                                </li>
                            ))}
                        </ul>
                    </div>
                )}
                {(action.type === 'trash' || action.type === 'report') && (
                    <p className="form-help">{t(`actionHelp.${action.type}`)}</p>
                )}
                {draft.extraActions.length > 0 && (
                    <p className="form-help">
                        {t('ruleEditor.extraActions', {
                            actions: draft.extraActions.map((a) => actionSummary(t, a)).join(', '),
                        })}
                    </p>
                )}
            </fieldset>

            <fieldset className="editor-section">
                <legend>{t('ruleEditor.scopeTitle')}</legend>
                {scopeChoices.length === 0 ? (
                    <p className="form-help">{t('ruleEditor.scopeNone')}</p>
                ) : (
                    <>
                        <p className="form-help">{t('ruleEditor.scopeHelp')}</p>
                        {scopeChoices.map((entry) => (
                            <label key={entry} className="checkbox-label">
                                <input
                                    type="checkbox"
                                    className="form-checkbox"
                                    checked={draft.scope.includes(entry)}
                                    onChange={(e) => toggleScope(entry, e.target.checked)}
                                />
                                <span className="checkbox-text">{displayPath(t, entry)}</span>
                            </label>
                        ))}
                    </>
                )}
            </fieldset>

            <fieldset className="editor-section">
                <legend>{t('ruleEditor.optionsTitle')}</legend>
                <label className="checkbox-label">
                    <input
                        type="checkbox"
                        className="form-checkbox"
                        checked={draft.includeHidden}
                        onChange={(e) => patch({ includeHidden: e.target.checked })}
                    />
                    <span className="checkbox-text">{t('ruleEditor.includeHidden')}</span>
                </label>
                <label className="checkbox-label">
                    <input
                        type="checkbox"
                        className="form-checkbox"
                        checked={draft.topLevelOnly}
                        onChange={(e) => patch({ topLevelOnly: e.target.checked })}
                    />
                    <span className="checkbox-text">{t('ruleEditor.topLevelOnly')}</span>
                </label>
            </fieldset>

            {saveError && (
                <div className="error-message" role="alert">
                    {saveError}
                </div>
            )}
            {hasErrors(errors) && (
                <p className="field-error" role="alert">
                    {t('ruleEditor.fix')}
                </p>
            )}

            <div className="editor-buttons">
                <button type="submit" className="btn-primary" disabled={saving}>
                    {saving ? t('common.saving') : t('ruleEditor.save')}
                </button>
                <button type="button" className="btn-secondary" onClick={onCancel} disabled={saving}>
                    {t('common.cancel')}
                </button>
            </div>
        </form>
    );
};

export default RuleEditor;
