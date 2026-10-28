import { describe, expect, it } from 'vitest';
import i18n from '.';
import { commandErrorText, journalErrorText, skipReasonText } from './format';

const t = i18n.t.bind(i18n);

describe('format helpers', () => {
    it('explains a missing extension with and without the rule extension', () => {
        expect(skipReasonText(t, { path: 'a', reason: 'noExtension', rule_name: 'R', ext: '.pdf' })).toBe(
            'Fișierul nu are extensie; regula folosește .pdf.',
        );
        expect(skipReasonText(t, { path: 'a', reason: 'noExtension', rule_name: 'R' })).toBe(
            'Fișierul nu are extensie.',
        );
        expect(skipReasonText(t, { path: 'a', reason: 'cloudPlaceholder', rule_name: null })).toBe(
            'Fișier OneDrive/Dropbox la cerere, nedescărcat',
        );
    });

    it('translates journal fail codes', () => {
        expect(journalErrorText(t, 'NoRecycleBin')).toBe(
            'Volumul nu are Coș de reciclare; Valet nu mută și nu copiază aici.',
        );
        expect(journalErrorText(t, 'Locked: in use')).toBe('Fișierul e blocat de alt program (in use)');
        expect(journalErrorText(t, 'NewCode')).toBe('NewCode');
    });

    it('translates known command error codes and passes other text through', () => {
        expect(commandErrorText(t, 'config.missing: C:\\x')).toBe('Folderul nu există. (C:\\x)');
        expect(commandErrorText(t, 'config.unknownCode')).toBe('Eroare: config.unknownCode');
        expect(commandErrorText(t, new Error('boom'))).toBe('Eroare: boom');
    });
});
