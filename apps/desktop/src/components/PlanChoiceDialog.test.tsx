import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import PlanChoiceDialog from './PlanChoiceDialog';
import type { Rule } from '../api/commands';
import { freePlan } from '../test/plan';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
const invokeMock = vi.mocked(invoke);

function rule(id: string, enabled: boolean, presetId?: string): Rule {
    return {
        id,
        name: `Regula ${id}`,
        enabled,
        alwaysApply: false,
        conditions: [{ type: 'ext', value: 'pdf' }],
        actions: [{ type: 'moveTo', params: { path: '{documents}' } }],
        version: 1,
        options: presetId ? { presetId } : enabled ? null : { planDisabled: true },
        scope: [],
    };
}

const FOLDERS = ['C:\\A', 'C:\\B', 'C:\\C'];
let rules: Rule[];
let choiceError: string | null;

beforeEach(() => {
    rules = [rule('r1', true), rule('r2', true), rule('r3', true), rule('r4', false), rule('r5', false), rule('p1', true, 'invoices')];
    choiceError = null;
    invokeMock.mockReset();
    invokeMock.mockImplementation(async (cmd) => {
        switch (cmd) {
            case 'get_rules':
                return rules;
            case 'get_config':
                return { inbox_paths: FOLDERS, invalid_paths: [], unavailable_paths: [], inactive_paths: ['C:\\C'] };
            case 'apply_plan_choice':
                if (choiceError) throw choiceError;
                return freePlan();
            default:
                throw new Error(`unexpected command ${cmd}`);
        }
    });
});

function box(id: string) {
    return screen.getByTestId(`plan-choice-rule-${id}`) as HTMLInputElement;
}

function folderBoxes() {
    return screen.getAllByTestId('plan-choice-folder') as HTMLInputElement[];
}

describe('PlanChoiceDialog', () => {
    it('lists own rules only, with the enabled ones checked, and the first 2 folders', async () => {
        render(<PlanChoiceDialog plan={freePlan({ choice_pending: true })} onDone={() => {}} onLater={() => {}} />);
        await screen.findByTestId('plan-choice-rule-r1');
        expect(screen.queryByTestId('plan-choice-rule-p1')).toBeNull();
        expect(['r1', 'r2', 'r3', 'r4', 'r5'].map((id) => box(id).checked)).toEqual([true, true, true, false, false]);
        expect(folderBoxes().map((b) => b.checked)).toEqual([true, true, false]);
        expect(screen.getByRole('dialog').getAttribute('aria-modal')).toBe('true');
        expect(screen.getByText(/Nimic nu se șterge/)).toBeTruthy();
    });

    it('allows at most 3 rules and 2 folders', async () => {
        render(<PlanChoiceDialog plan={freePlan({ choice_pending: true })} onDone={() => {}} onLater={() => {}} />);
        await screen.findByTestId('plan-choice-rule-r1');
        expect(box('r4').disabled).toBe(true);
        expect(box('r5').disabled).toBe(true);
        expect(folderBoxes()[2].disabled).toBe(true);
        expect(screen.getByText('Reguli proprii: 3 din 3 alese')).toBeTruthy();

        fireEvent.click(box('r2'));
        expect(box('r4').disabled).toBe(false);
        fireEvent.click(box('r4'));
        expect(box('r5').disabled).toBe(true);
        expect(box('r2').disabled).toBe(true);
    });

    it('saves the chosen rules and folders with apply_plan_choice', async () => {
        const onDone = vi.fn();
        render(<PlanChoiceDialog plan={freePlan({ choice_pending: true })} onDone={onDone} onLater={() => {}} />);
        await screen.findByTestId('plan-choice-rule-r1');
        fireEvent.click(box('r1'));
        fireEvent.click(box('r5'));
        fireEvent.click(folderBoxes()[0]);
        // One folder chosen: Save waits for the second.
        expect((screen.getByTestId('plan-choice-save') as HTMLButtonElement).disabled).toBe(true);
        fireEvent.click(folderBoxes()[2]);
        fireEvent.click(screen.getByTestId('plan-choice-save'));
        await waitFor(() => expect(onDone).toHaveBeenCalledWith(freePlan()));
        expect(invokeMock).toHaveBeenCalledWith('apply_plan_choice', {
            ruleIds: ['r2', 'r3', 'r5'],
            folders: ['C:\\B', 'C:\\C'],
        });
    });

    it('shows a backend refusal and stays open', async () => {
        choiceError = 'plan.notFree';
        const onDone = vi.fn();
        render(<PlanChoiceDialog plan={freePlan({ choice_pending: true })} onDone={onDone} onLater={() => {}} />);
        await screen.findByTestId('plan-choice-rule-r1');
        fireEvent.click(screen.getByTestId('plan-choice-save'));
        expect((await screen.findByRole('alert')).textContent).toBe('Alegerea se face doar în planul Gratuit.');
        expect(onDone).not.toHaveBeenCalled();
    });

    it('hides the folder list when there are not more folders than the limit', async () => {
        invokeMock.mockImplementation(async (cmd) => {
            if (cmd === 'get_rules') return rules;
            if (cmd === 'get_config') return { inbox_paths: ['C:\\A'] };
            if (cmd === 'apply_plan_choice') return freePlan();
            throw new Error(`unexpected command ${cmd}`);
        });
        render(<PlanChoiceDialog plan={freePlan({ choice_pending: true })} onDone={() => {}} onLater={() => {}} />);
        await screen.findByTestId('plan-choice-rule-r1');
        expect(screen.queryAllByTestId('plan-choice-folder')).toHaveLength(0);
        fireEvent.click(screen.getByTestId('plan-choice-save'));
        await waitFor(() =>
            expect(invokeMock).toHaveBeenCalledWith('apply_plan_choice', {
                ruleIds: ['r1', 'r2', 'r3'],
                folders: ['C:\\A'],
            }),
        );
    });

    it('closes with Escape (decide later) and keeps focus inside with Tab', async () => {
        const onLater = vi.fn();
        render(<PlanChoiceDialog plan={freePlan({ choice_pending: true })} onDone={() => {}} onLater={onLater} />);
        await screen.findByTestId('plan-choice-rule-r1');
        const dialog = screen.getByRole('dialog');
        expect(document.activeElement).toBe(dialog);

        const later = screen.getByTestId('plan-choice-later');
        later.focus();
        fireEvent.keyDown(dialog, { key: 'Tab' });
        expect(document.activeElement).toBe(box('r1'));
        fireEvent.keyDown(dialog, { key: 'Tab', shiftKey: true });
        expect(document.activeElement).toBe(later);

        fireEvent.keyDown(dialog, { key: 'Escape' });
        expect(onLater).toHaveBeenCalledTimes(1);
        expect(invokeMock).not.toHaveBeenCalledWith('apply_plan_choice', expect.anything());
    });
});
