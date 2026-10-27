---
name: phase
description: Run one phase of docs/PLAN.md end to end as project manager — plan, dispatch subagents, verify, write the phase report, stop. Invoke as /phase N.
disable-model-invocation: true
---

# /phase $ARGUMENTS

You are the project manager for phase $ARGUMENTS of Valet. Follow `docs/PLAN.md` section 5 for this phase and `CLAUDE.md` for working rules.

1. Read `docs/PLAN.md` (sections 2.6, 4 and the phase entry) and the previous phase report if any. Write `.claude/state/tests-ran.txt` with a single line `phase $ARGUMENTS started` (this arms the stop gate).
2. Write a task list for the phase: one task per subagent deliverable, each with a mechanically checkable acceptance criterion (test name, command and expected exit code, number, file). Show it, then proceed without waiting.
3. Dispatch subagents with precise delegation prompts: area, files they may touch, acceptance criteria, the invariants that apply. Wait for each one that others depend on; run independent ones in parallel only on disjoint file areas.
4. After each subagent returns, verify yourself with the real commands (do not trust the summary): `cargo check`, the specific tests, `pnpm build` if frontend. Re-dispatch with the failure output if not met.
5. When all tasks are met, run `qa` for the full suite and the budgets. Ensure `.claude/state/tests-ran.txt` contains `cargo test exit=0`.
6. Write `docs/reports/phase-$ARGUMENTS.md` using the template below, commit everything to main, and stop. Do not start the next phase.

If something needs Vlad (keys, accounts, prices, a dependency outside the allowlist, product ambiguity), list it under "Decizii necesare" in the report and stop; do not guess.

## Report template (write it in Romanian, technical terms in English)

```
# Raport Faza N — <titlul fazei>

## Ce s-a construit
- <bullet per deliverable, with file paths>

## Ce s-a testat
| Comandă | Exit | Observații |
|---|---|---|

## Măsurători vs. bugete (PLAN.md 2.6)
| Metrică | Buget | Măsurat |

## Probleme cunoscute
- ...

## Decizii necesare de la Vlad
- ...

## Checklist manual pentru Vlad
- [ ] ...

## Propunere pentru faza următoare
- <un paragraf, doar dacă ceva din PLAN.md trebuie schimbat>
```
