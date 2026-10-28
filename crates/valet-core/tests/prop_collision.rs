//! Property tests for `collision::next_free_name` (PLAN.md 4.6, rust-testing suite 2):
//! injective over any set of taken names, stable across runs, and the
//! `stem (n).ext` rule with only the last extension kept apart.

use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use proptest::prelude::*;
use valet_core::collision::next_free_name;

const DIR: &str = "dest";

/// File stem without a trailing dot (so `Path::extension` splits exactly
/// where the generator did). May contain inner dots (`a.tar`).
fn stem() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9_]([a-zA-Z0-9 _.-]{0,6}[a-zA-Z0-9_])?"
}

fn ext() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9]{1,4}"
}

/// A file name with an extension (the common case).
fn name_with_ext() -> impl Strategy<Value = (String, String)> {
    (stem(), ext())
}

/// Names already taken in the folder: arbitrary names plus `stem (n).ext`
/// variants of the requested name, so gaps and runs both occur.
fn taken_set(
    base_stem: String,
    base_ext: String,
) -> impl Strategy<Value = (String, String, BTreeSet<String>)> {
    let variants = proptest::collection::btree_set(2u64..12, 0..8);
    let others = proptest::collection::btree_set(
        (stem(), ext()).prop_map(|(s, e)| format!("{s}.{e}")),
        0..8,
    );
    (variants, others, any::<bool>()).prop_map(move |(ns, others, base_taken)| {
        let mut taken: BTreeSet<String> = others;
        if base_taken {
            taken.insert(format!("{base_stem}.{base_ext}"));
        }
        for n in ns {
            taken.insert(format!("{base_stem} ({n}).{base_ext}"));
        }
        (base_stem.clone(), base_ext.clone(), taken)
    })
}

fn case() -> impl Strategy<Value = (String, String, BTreeSet<String>)> {
    name_with_ext().prop_flat_map(|(s, e)| taken_set(s, e))
}

fn paths(names: &BTreeSet<String>) -> HashSet<PathBuf> {
    names.iter().map(|n| Path::new(DIR).join(n)).collect()
}

/// Reserves `n` names for the same requested destination, one after the
/// other, each time treating earlier results as taken (what the planner does).
fn reserve_n(dest: &Path, taken: &HashSet<PathBuf>, n: usize) -> Vec<PathBuf> {
    let mut reserved: HashSet<PathBuf> = HashSet::new();
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let got = next_free_name(dest, |p| taken.contains(p) || reserved.contains(p));
        reserved.insert(got.clone());
        out.push(got);
    }
    out
}

/// `n` in `stem (n).ext` / `stem (n)` if `name` has that shape for `stem`/`ext`.
fn suffix_number(name: &str, stem: &str, ext: Option<&str>) -> Option<u64> {
    let rest = name.strip_prefix(stem)?.strip_prefix(" (")?;
    let rest = match ext {
        Some(e) => rest.strip_suffix(&format!(".{e}"))?,
        None => rest,
    };
    let digits = rest.strip_suffix(')')?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, failure_persistence: None, ..ProptestConfig::default() })]

    /// N identical requests get N distinct names, none of them taken.
    #[test]
    fn repeated_reservations_are_distinct_and_free(
        (s, e, taken) in case(),
        n in 1usize..24,
    ) {
        let taken_paths = paths(&taken);
        let dest = Path::new(DIR).join(format!("{s}.{e}"));
        let got = reserve_n(&dest, &taken_paths, n);
        let distinct: HashSet<&PathBuf> = got.iter().collect();
        prop_assert_eq!(distinct.len(), n);
        for p in &got {
            prop_assert!(!taken_paths.contains(p), "{} was taken", p.display());
            prop_assert_eq!(p.parent(), Some(Path::new(DIR)));
        }
    }

    /// Case-insensitive taken predicate (as on NTFS): results stay distinct
    /// after case folding and never fold onto a taken name.
    #[test]
    fn reservations_distinct_case_insensitively(
        (s, e, taken) in case(),
        n in 1usize..16,
    ) {
        let fold = |p: &Path| p.to_string_lossy().to_lowercase();
        let taken_folded: HashSet<String> = paths(&taken).iter().map(|p| fold(p)).collect();
        let dest = Path::new(DIR).join(format!("{}.{}", s.to_uppercase(), e));
        let mut reserved: HashSet<String> = HashSet::new();
        for _ in 0..n {
            let got = next_free_name(&dest, |p| {
                taken_folded.contains(&fold(p)) || reserved.contains(&fold(p))
            });
            prop_assert!(!taken_folded.contains(&fold(&got)));
            prop_assert!(reserved.insert(fold(&got)), "duplicate {}", got.display());
        }
    }

    /// Same inputs give the same sequence, independent of how the taken set
    /// is stored or iterated.
    #[test]
    fn stable_across_runs((s, e, taken) in case(), n in 1usize..16) {
        let dest = Path::new(DIR).join(format!("{s}.{e}"));
        let a = reserve_n(&dest, &paths(&taken), n);
        let b = reserve_n(&dest, &paths(&taken), n);
        let ordered: BTreeSet<PathBuf> = taken.iter().rev().map(|t| Path::new(DIR).join(t)).collect();
        let mut reserved: HashSet<PathBuf> = HashSet::new();
        let c: Vec<PathBuf> = (0..n)
            .map(|_| {
                let got = next_free_name(&dest, |p| ordered.contains(p) || reserved.contains(p));
                reserved.insert(got.clone());
                got
            })
            .collect();
        prop_assert_eq!(&a, &b);
        prop_assert_eq!(&a, &c);
    }

    /// A taken name becomes `stem (n).ext` with only the last extension kept
    /// apart; `n` is the smallest free number >= 2; a free name is returned as is.
    #[test]
    fn suffix_rule_and_smallest_free_number((s, e, taken) in case()) {
        let taken_paths = paths(&taken);
        let requested = format!("{s}.{e}");
        let dest = Path::new(DIR).join(&requested);
        let got = next_free_name(&dest, |p| taken_paths.contains(p));
        if !taken.contains(&requested) {
            prop_assert_eq!(got, dest);
        } else {
            let name = got.file_name().unwrap().to_str().unwrap().to_owned();
            let n = suffix_number(&name, &s, Some(&e));
            prop_assert!(n.is_some(), "{name} is not `{s} (n).{e}`");
            let n = n.unwrap();
            prop_assert!(n >= 2);
            for k in 2..n {
                prop_assert!(taken.contains(&format!("{s} ({k}).{e}")), "gap at {k} not used");
            }
        }
    }

    /// Names without an extension get the suffix at the end.
    #[test]
    fn no_extension_suffix_at_end(
        s in "[a-zA-Z0-9_][a-zA-Z0-9 _-]{0,7}",
        ns in proptest::collection::btree_set(2u64..8, 0..5),
    ) {
        let mut taken: BTreeSet<String> = ns.iter().map(|n| format!("{s} ({n})")).collect();
        taken.insert(s.clone());
        let taken_paths = paths(&taken);
        let got = next_free_name(&Path::new(DIR).join(&s), |p| taken_paths.contains(p));
        let name = got.file_name().unwrap().to_str().unwrap().to_owned();
        let n = suffix_number(&name, &s, None);
        prop_assert!(n.is_some(), "{name} is not `{s} (n)`");
        prop_assert!(!taken.contains(&name));
    }
}

#[test]
fn double_extension_example() {
    let taken: HashSet<PathBuf> = [Path::new(DIR).join("a.tar.gz")].into_iter().collect();
    let got = reserve_n(&Path::new(DIR).join("a.tar.gz"), &taken, 3);
    assert_eq!(
        got,
        vec![
            Path::new(DIR).join("a.tar (2).gz"),
            Path::new(DIR).join("a.tar (3).gz"),
            Path::new(DIR).join("a.tar (4).gz"),
        ]
    );
}
