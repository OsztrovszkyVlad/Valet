import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import i18n from '../i18n';
import ProSection from './ProSection';
import type { PlanView } from '../api/commands';
import { LICENSE, freePlan, proPlan, trialPlan } from '../test/plan';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
const invokeMock = vi.mocked(invoke);

/** What the license commands answer next, by command name. */
let answers: Record<string, unknown>;

beforeEach(() => {
    answers = {};
    invokeMock.mockReset();
    invokeMock.mockImplementation(async (cmd) => {
        if (!(cmd in answers)) throw new Error(`unexpected command ${cmd}`);
        const a = answers[cmd];
        if (a instanceof Error) throw a.message;
        return a;
    });
});

function renderSection(plan: PlanView) {
    const onPlanChange = vi.fn();
    render(<ProSection plan={plan} onPlanChange={onPlanChange} />);
    return onPlanChange;
}

describe('ProSection', () => {
    it('describes the Free plan with the price and the plan table', () => {
        renderSection(freePlan());
        expect(screen.getByTestId('pro-plan-line').textContent).toBe('Planul curent: Gratuit.');
        expect(screen.getByTestId('pro-price').textContent).toBe(
            '29 € (TVA inclus), plată unică, actualizări 12 luni; reînnoire opțională 15 €/an.',
        );
        const rules = screen.getByText('Reguli proprii active în același timp').closest('tr');
        expect(rules?.textContent).toBe('Reguli proprii active în același timp3nelimitat');
        const folders = screen.getByText('Foldere urmărite active în același timp').closest('tr');
        expect(folders?.textContent).toBe('Foldere urmărite active în același timp2nelimitat');
        // license.missing without a license says nothing new.
        expect(screen.queryByTestId('pro-reasons')).toBeNull();
        expect(screen.queryByTestId('pro-license')).toBeNull();
    });

    it('shows the trial days left', () => {
        renderSection(trialPlan(5));
        expect(screen.getByTestId('pro-plan-line').textContent).toMatch(/^Planul curent: Probă Pro, încă 5 zile \(până la .+\)\.$/);
    });

    it('uses the plural forms for 1 and 20 trial days', () => {
        const { unmount } = render(<ProSection plan={trialPlan(1)} onPlanChange={() => {}} />);
        expect(screen.getByTestId('pro-plan-line').textContent).toContain('încă 1 zi ');
        unmount();
        render(<ProSection plan={trialPlan(20)} onPlanChange={() => {}} />);
        expect(screen.getByTestId('pro-plan-line').textContent).toContain('încă 20 de zile ');
    });

    it('shows Pro with the email, the updates date and the last check, without the buy button', () => {
        renderSection(proPlan());
        expect(screen.getByTestId('pro-plan-line').textContent).toBe('Planul curent: Pro, licență pe ana@example.com.');
        expect(screen.getByText(/^Actualizări incluse până la 29 octombrie 2027\.$/)).toBeTruthy();
        expect(screen.getByText(/^Ultima verificare a licenței: 20 octombrie 2026/)).toBeTruthy();
        expect(screen.getByTestId('pro-license-id').textContent).toBe(
            `Cod licență (pentru asistență): ${LICENSE.license_id}`,
        );
        expect(screen.queryByTestId('pro-buy')).toBeNull();
        expect(screen.getByTestId('pro-check')).toBeTruthy();
        expect(screen.getByTestId('pro-remove')).toBeTruthy();
    });

    it('disables buying with a note when checkout is not available', () => {
        renderSection(freePlan({ checkout_available: false }));
        const buy = screen.getByTestId('pro-buy') as HTMLButtonElement;
        expect(buy.disabled).toBe(true);
        expect(screen.getByTestId('pro-buy-unavailable').textContent).toBe('Cumpărarea nu e deschisă încă.');
        expect(buy.getAttribute('aria-describedby')).toBe(screen.getByTestId('pro-buy-unavailable').id);
    });

    it('opens the checkout page through the backend', async () => {
        answers.open_external = undefined;
        renderSection(freePlan());
        fireEvent.click(screen.getByTestId('pro-buy'));
        await waitFor(() => expect(invokeMock).toHaveBeenCalledWith('open_external', { target: 'checkout' }));
    });

    it('imports a license file and passes the new plan up', async () => {
        const next = proPlan();
        answers.import_license_file = next;
        const onPlanChange = renderSection(freePlan());
        fireEvent.click(screen.getByTestId('pro-import-file'));
        await waitFor(() => expect(onPlanChange).toHaveBeenCalledWith(next));
        expect(invokeMock).toHaveBeenCalledWith('import_license_file');
        expect(await screen.findByText('Licența e activă pe acest calculator.')).toBeTruthy();
    });

    it('does nothing when the file dialog is cancelled', async () => {
        answers.import_license_file = null;
        const onPlanChange = renderSection(freePlan());
        fireEvent.click(screen.getByTestId('pro-import-file'));
        await waitFor(() => expect(invokeMock).toHaveBeenCalledWith('import_license_file'));
        await waitFor(() => expect((screen.getByTestId('pro-import-file') as HTMLButtonElement).disabled).toBe(false));
        expect(onPlanChange).not.toHaveBeenCalled();
        expect(screen.queryByRole('alert')).toBeNull();
    });

    it('shows a sentence for license import errors', async () => {
        answers.import_license_file = new Error('license.too_many');
        renderSection(freePlan());
        fireEvent.click(screen.getByTestId('pro-import-file'));
        expect((await screen.findByRole('alert')).textContent).toContain(
            'Licența e deja activă pe 3 calculatoare. Scoate-o de pe unul dintre ele din Setări, apoi încearcă din nou.',
        );
    });

    it('imports a pasted license text', async () => {
        const next = proPlan();
        answers.import_license_text = next;
        const onPlanChange = renderSection(trialPlan(3));
        fireEvent.click(screen.getByTestId('pro-paste-toggle'));
        const submit = screen.getByTestId('pro-paste-submit') as HTMLButtonElement;
        expect(submit.disabled).toBe(true);
        fireEvent.change(screen.getByLabelText('Textul licenței'), { target: { value: '  {"payload":"x"}\n' } });
        fireEvent.click(submit);
        await waitFor(() => expect(onPlanChange).toHaveBeenCalledWith(next));
        expect(invokeMock).toHaveBeenCalledWith('import_license_text', { text: '{"payload":"x"}' });
        expect(screen.queryByTestId('pro-paste-text')).toBeNull();
    });

    it('keeps the pasted text and shows the error when the license is refused', async () => {
        answers.import_license_text = new Error('license.invalid_signature');
        renderSection(freePlan());
        fireEvent.click(screen.getByTestId('pro-paste-toggle'));
        fireEvent.change(screen.getByLabelText('Textul licenței'), { target: { value: 'garbage' } });
        fireEvent.click(screen.getByTestId('pro-paste-submit'));
        expect((await screen.findByRole('alert')).textContent).toContain(
            'Licența nu e validă: semnătura nu se potrivește.',
        );
        expect((screen.getByTestId('pro-paste-text') as HTMLTextAreaElement).value).toBe('garbage');
    });

    it('says when licensing is not configured and disables license import', () => {
        renderSection(freePlan({ licensing_configured: false, reasons: ['license.not_configured'] }));
        expect(screen.getByTestId('pro-not-configured').textContent).toBe(
            'Licențele nu sunt încă active în această versiune. Planurile Gratuit și Probă funcționează normal.',
        );
        expect((screen.getByTestId('pro-import-file') as HTMLButtonElement).disabled).toBe(true);
        expect((screen.getByTestId('pro-paste-toggle') as HTMLButtonElement).disabled).toBe(true);
        expect(screen.queryByTestId('pro-reasons')).toBeNull();
    });

    it('explains a license that does not cover this build, naming the date', () => {
        renderSection(freePlan({ license: LICENSE, reasons: ['license.not_covering_build'] }));
        expect(screen.getByText('Licență pe acest calculator: ana@example.com.')).toBeTruthy();
        expect(screen.getByTestId('pro-reasons').textContent).toMatch(
            /^Licența acoperă versiunile publicate până la 29 octombrie 2027\. Această versiune e mai nouă/,
        );
    });

    it('has a sentence for every reason code in both languages', async () => {
        const codes = [
            'license.invalid_signature',
            'license.wrong_product',
            'license.not_covering_build',
            'activation.missing',
            'activation.other_machine',
            'activation.expired',
        ];
        renderSection(freePlan({ license: LICENSE, reasons: codes }));
        const items = [...screen.getByTestId('pro-reasons').querySelectorAll('li')].map((li) => li.textContent);
        expect(items).toHaveLength(codes.length);
        for (const text of items) expect(codes).not.toContain(text);
        await i18n.changeLanguage('en');
        expect(i18n.t('license.reason.activation.expired')).toMatch(/^The license has not been checked online/);
    });

    it('checks the license now', async () => {
        const next = proPlan({ last_check_at: Date.UTC(2026, 9, 29) / 1000 });
        answers.check_license_now = next;
        const onPlanChange = renderSection(proPlan());
        fireEvent.click(screen.getByTestId('pro-check'));
        await waitFor(() => expect(onPlanChange).toHaveBeenCalledWith(next));
        expect(await screen.findByText('Licența a fost verificată.')).toBeTruthy();
    });

    it('shows the network error of a license check', async () => {
        answers.check_license_now = new Error('license.network');
        renderSection(proPlan());
        fireEvent.click(screen.getByTestId('pro-check'));
        expect((await screen.findByRole('alert')).textContent).toContain('Serverul de licențe nu a putut fi contactat.');
    });

    it('removes the license only after a confirmation that names the freed activation', async () => {
        const next = freePlan();
        answers.remove_license = next;
        const onPlanChange = renderSection(proPlan());
        fireEvent.click(screen.getByTestId('pro-remove'));
        const confirm = screen.getByRole('alertdialog');
        expect(confirm.textContent).toContain('eliberează una dintre cele 3 activări');
        expect(invokeMock).not.toHaveBeenCalled();

        fireEvent.click(screen.getByText('Renunță'));
        expect(screen.queryByRole('alertdialog')).toBeNull();

        fireEvent.click(screen.getByTestId('pro-remove'));
        fireEvent.click(screen.getByTestId('pro-remove-yes'));
        await waitFor(() => expect(onPlanChange).toHaveBeenCalledWith(next));
        expect(invokeMock).toHaveBeenCalledWith('remove_license');
    });
});
