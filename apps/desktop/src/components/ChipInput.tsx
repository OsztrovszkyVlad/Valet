import React, { useState } from 'react';
import { useTranslation } from 'react-i18next';

interface ChipInputProps {
    id: string;
    label: string;
    placeholder: string;
    values: string[];
    onChange: (values: string[]) => void;
    /** Applied to each value before it is added; an empty result is dropped. */
    normalize?: (raw: string) => string;
    /** At most this many values; the input hides when reached. */
    max?: number;
    invalid?: boolean;
    describedBy?: string;
}

/** A list of short values (extensions, words) shown as removable chips. Enter, comma or leaving the field adds. */
const ChipInput: React.FC<ChipInputProps> = ({
    id,
    label,
    placeholder,
    values,
    onChange,
    normalize = (s) => s.trim(),
    max,
    invalid,
    describedBy,
}) => {
    const { t } = useTranslation();
    const [text, setText] = useState('');

    const add = () => {
        const v = normalize(text);
        setText('');
        if (v === '' || values.some((x) => x.toLowerCase() === v.toLowerCase())) return;
        if (max !== undefined && values.length >= max) return;
        onChange([...values, v]);
    };

    const full = max !== undefined && values.length >= max;

    return (
        <div className="chip-input">
            <ul className="chip-list">
                {values.map((v) => (
                    <li key={v} className="chip">
                        <span>{v}</span>
                        <button
                            type="button"
                            className="chip-remove"
                            aria-label={t('ruleEditor.chipRemove', { value: v })}
                            onClick={() => onChange(values.filter((x) => x !== v))}
                        >
                            ×
                        </button>
                    </li>
                ))}
            </ul>
            {!full && (
                <div className="chip-entry">
                    <label htmlFor={id} className="visually-hidden">
                        {label}
                    </label>
                    <input
                        id={id}
                        className="form-input"
                        type="text"
                        value={text}
                        placeholder={placeholder}
                        aria-invalid={invalid || undefined}
                        aria-describedby={describedBy}
                        onChange={(e) => setText(e.target.value)}
                        onBlur={add}
                        onKeyDown={(e) => {
                            if (e.key === 'Enter' || e.key === ',') {
                                e.preventDefault();
                                add();
                            }
                        }}
                    />
                    <button type="button" className="btn-secondary" onClick={add} disabled={text.trim() === ''}>
                        {t('ruleEditor.chipAdd')}
                    </button>
                </div>
            )}
        </div>
    );
};

export default ChipInput;
