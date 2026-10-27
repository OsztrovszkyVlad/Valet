---
name: rust-testing
description: How Valet tests Rust: unit, proptest, 10k-file integration fixtures, fault injection by killing a child process mid-batch, Windows-only tests, criterion budgets, and how to record runs for the stop gate. Load for any test or benchmark work.
---

# Rust testing (Valet)

## Layout
- Unit tests next to code (`#[cfg(test)]`).
- Property tests in `crates/valet-core/tests/prop_*.rs` with `proptest`.
- Integration in `crates/valet-core/tests/integration_*.rs` using `tempfile::tempdir()` and the fixture builder `tests/common/fixture.rs` (creates N files with given extensions, sizes, mtimes, names).
- Windows-only tests gated with `#[cfg(windows)]` in `crates/valet-platform/tests/`.
- Benchmarks in `crates/valet-core/benches/` with `criterion`.

## Required suites
1. Matcher: every condition type × positive/negative case; proptest: no file gets two conflicting actions from one rule; evaluation is deterministic.
2. Collision: proptest that `next_free_name` is injective over any set of existing names and stable across runs.
3. Journal: state machine transitions; recovery from each interrupted state.
4. Fault injection: `tests/fault_injection.rs` spawns the CLI apply binary on a fixture, kills it after a random delay (10–500 ms) during apply, restarts with `--recover`, then asserts every original file exists exactly once (source xor dest), hashes match, and history explains it. 100 iterations in CI (`VALET_FAULT_ITERS`).
5. Integration: each preset on a 10k-file fixture: expected moves, no moves outside scope, undo restores a byte-identical tree (compare sorted (path, hash) lists).
6. Cross-volume: simulate with two tempdirs and `VALET_FORCE_CROSS_VOLUME=1` so the copy+verify+trash path runs on CI.

## Budgets (from docs/PLAN.md 2.6)
index 100k files < 60 s; dry run on 10k < 3 s; idle RAM < 150 MB; idle CPU < 1%; startup < 2 s. Report measured numbers, not adjectives.

## Commands
`cargo test --workspace`, `cargo test -p valet-core --test fault_injection`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check`, `cargo bench -p valet-core`.
After the full suite, append `cargo test exit=<code>` to `.claude/state/tests-ran.txt` (the Stop hook reads it).
