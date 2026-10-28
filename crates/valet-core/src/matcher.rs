use crate::{rules::Rule, typegroups};
use regex::Regex;
use serde_json::Value;
use std::{
    cell::RefCell,
    collections::HashMap,
    path::Path,
    time::{Duration, SystemTime},
};

/// Distinct patterns kept per thread before the cache is emptied.
const REGEX_CACHE_CAP: usize = 256;

thread_local! {
    /// Compiled `nameMatches` patterns; `None` records an invalid pattern.
    static REGEX_CACHE: RefCell<HashMap<String, Option<Regex>>> = RefCell::new(HashMap::new());
}

/// `pat` matches `text`; an invalid pattern never matches. Each distinct
/// pattern is compiled once per thread.
fn regex_is_match(pat: &str, text: &str) -> bool {
    REGEX_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if !cache.contains_key(pat) {
            if cache.len() >= REGEX_CACHE_CAP {
                cache.clear();
            }
            cache.insert(pat.to_owned(), Regex::new(pat).ok());
        }
        cache[pat].as_ref().is_some_and(|re| re.is_match(text))
    })
}

/// What the matcher knows about one directory entry. Filled by the planner
/// from metadata; the matcher never touches the disk or the clock.
#[derive(Debug, Clone, Copy, Default)]
pub struct FileFacts<'a> {
    pub path: &'a str,
    pub size: u64,
    /// Last modification time; `None` when unknown (age conditions are false).
    pub mtime: Option<SystemTime>,
    /// Creation time (`Metadata::created`); `None` when the OS does not give
    /// one (`createdOlderThanDays` is false).
    pub created: Option<SystemTime>,
    /// The entry is a directory. Rules with `emptyFolder` see only
    /// directories; all other rules see only files.
    pub is_dir: bool,
    /// A directory with no entries at all.
    pub is_empty_dir: bool,
    /// The plan's clock (`PlanContext::now`). `None`: age conditions are false.
    pub now: Option<SystemTime>,
}

/// Seconds in one day for the `*OlderThanDays` conditions.
const DAY_SECS: u64 = 86_400;

/// `rule` has an `emptyFolder` condition (any value), so it applies to
/// directories only.
pub fn is_folder_rule(rule: &Rule) -> bool {
    rule.conditions.iter().any(|c| c.r#type == "emptyFolder")
}

/// `rule` has a `duplicate` condition (any value).
pub fn is_duplicate_rule(rule: &Rule) -> bool {
    rule.conditions.iter().any(|c| c.r#type == "duplicate")
}

/// Extension rules use: one leading dot dropped, Unicode lowercase.
fn norm_ext(e: &str) -> String {
    e.strip_prefix('.').unwrap_or(e).to_lowercase()
}

/// Non-empty array of strings, each normalized with `f`; `None` if the value
/// is not that (malformed).
fn string_list(v: &Value, f: impl Fn(&str) -> String) -> Option<Vec<String>> {
    let arr = v.as_array().filter(|a| !a.is_empty())?;
    arr.iter().map(|x| x.as_str().map(&f)).collect()
}

/// `t` is more than `days` whole days before `now`. Missing times, times in
/// the future and day counts that overflow give false.
fn older_than(t: Option<SystemTime>, now: Option<SystemTime>, days: Option<u64>) -> bool {
    let (Some(t), Some(now), Some(days)) = (t, now, days) else {
        return false;
    };
    let Some(secs) = days.checked_mul(DAY_SECS) else {
        return false;
    };
    now.duration_since(t)
        .is_ok_and(|age| age > Duration::from_secs(secs))
}

/// The rule's conditions hold for `f` (all of them; an enabled rule with no
/// conditions holds for every file). A condition with a malformed value or an
/// unknown type never holds.
///
/// `duplicate: true` holds here: whether a byte-identical copy exists is
/// decided by the planner over all files the rule's other conditions match.
pub fn rule_matches(rule: &Rule, f: &FileFacts) -> bool {
    if !rule.enabled {
        return false;
    }
    if is_folder_rule(rule) != f.is_dir {
        return false;
    }
    if rule.always_apply {
        return true;
    }

    let file_name = Path::new(f.path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let ext = || {
        Path::new(f.path)
            .extension()
            .and_then(|e| e.to_str())
            .map(norm_ext)
            .filter(|e| !e.is_empty())
    };

    rule.conditions.iter().all(|c| match c.r#type.as_str() {
        "ext" => {
            let want = c.value.as_str().unwrap_or_default().to_ascii_lowercase();
            Path::new(f.path)
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case(&want))
                .unwrap_or(false)
        }
        "extIn" => match string_list(&c.value, norm_ext) {
            Some(list) => ext().is_some_and(|e| list.contains(&e)),
            None => false,
        },
        "extNotIn" => match string_list(&c.value, norm_ext) {
            Some(list) => ext().is_none_or(|e| !list.contains(&e)),
            None => false,
        },
        "typeGroup" => match c.value.as_str() {
            Some(g) => ext().is_some_and(|e| typegroups::contains(g, &e)),
            None => false,
        },
        "nameContains" => {
            let needles = match &c.value {
                Value::String(s) => Some(vec![s.to_lowercase()]),
                v => string_list(v, str::to_lowercase),
            };
            match needles {
                Some(n) if n.iter().all(|s| !s.is_empty()) => {
                    let name = file_name.to_lowercase();
                    n.iter().any(|s| name.contains(s.as_str()))
                }
                _ => false,
            }
        }
        "nameMatches" => {
            let pat = c.value.as_str().unwrap_or("");
            regex_is_match(pat, file_name)
        }
        "sizeGt" => {
            let n = c.value.as_i64().unwrap_or(0) as u64;
            f.size > n
        }
        "sizeLt" => {
            let n = c.value.as_i64().unwrap_or(0) as u64;
            f.size < n
        }
        "pathContains" => {
            let needle = c.value.as_str().unwrap_or("").to_ascii_lowercase();
            f.path.to_ascii_lowercase().contains(&needle)
        }
        "modifiedOlderThanDays" => older_than(f.mtime, f.now, c.value.as_u64()),
        "createdOlderThanDays" => older_than(f.created, f.now, c.value.as_u64()),
        "emptyFolder" => c.value == Value::Bool(true) && f.is_dir && f.is_empty_dir,
        "duplicate" => c.value == Value::Bool(true),
        // unimplemented: mimeIs (needs a mime sniff crate)
        _ => false,
    })
}

pub fn matching_rules<'a>(rules: &'a [Rule], facts: &FileFacts) -> Vec<&'a Rule> {
    rules.iter().filter(|r| rule_matches(r, facts)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn rule_with(conditions: Value) -> Rule {
        serde_json::from_value(json!({
            "id": "00000000-0000-0000-0000-000000000001",
            "name": "r",
            "enabled": true,
            "conditions": conditions,
            "actions": [{ "type": "report" }],
        }))
        .unwrap()
    }

    fn cond(t: &str, v: Value) -> Rule {
        rule_with(json!([{ "type": t, "value": v }]))
    }

    fn file(path: &str) -> FileFacts<'_> {
        FileFacts {
            path,
            size: 1,
            ..Default::default()
        }
    }

    fn hits(r: &Rule, path: &str) -> bool {
        rule_matches(r, &file(path))
    }

    const NOW: Duration = Duration::from_secs(1_000 * DAY_SECS);

    fn at(days_ago: u64, extra_secs: i64) -> Option<SystemTime> {
        let base = SystemTime::UNIX_EPOCH + NOW - Duration::from_secs(days_ago * DAY_SECS);
        Some(if extra_secs >= 0 {
            base + Duration::from_secs(extra_secs as u64)
        } else {
            base - Duration::from_secs((-extra_secs) as u64)
        })
    }

    fn aged(path: &str, mtime: Option<SystemTime>, created: Option<SystemTime>) -> FileFacts<'_> {
        FileFacts {
            path,
            mtime,
            created,
            now: Some(SystemTime::UNIX_EPOCH + NOW),
            ..Default::default()
        }
    }

    // ------------------------------------------------------------ nameContains

    #[test]
    fn name_contains_string_is_unicode_case_insensitive_on_file_name() {
        let r = cond("nameContains", json!("CAPTURĂ"));
        assert!(hits(&r, "/d/Captură ecran 1.png"));
        assert!(hits(&r, "/d/xcaptură.png"));
        assert!(!hits(&r, "/d/Captura.png"), "ă is not a");
        // Only the file name is searched, not the folders.
        assert!(!hits(&r, "/captură/a.png"));
    }

    #[test]
    fn name_contains_array_is_any_of() {
        let r = cond("nameContains", json!(["invoice", "Factura", "BON"]));
        assert!(hits(&r, "/d/Invoice-12.pdf"));
        assert!(hits(&r, "/d/factura_1.pdf"));
        assert!(hits(&r, "/d/bon fiscal.pdf"));
        assert!(!hits(&r, "/d/contract.pdf"));
    }

    #[test]
    fn name_contains_malformed_never_matches() {
        for v in [
            json!(""),
            json!([]),
            json!(["a", ""]),
            json!(["a", 1]),
            json!(3),
            json!(null),
            json!({ "x": "a" }),
        ] {
            assert!(!hits(&cond("nameContains", v.clone()), "/d/a.pdf"), "{v}");
        }
    }

    // ------------------------------------------------------------ extIn / extNotIn

    #[test]
    fn ext_in_tolerates_dot_and_case() {
        let r = cond("extIn", json!([".PDF", "docx"]));
        assert!(hits(&r, "/d/a.pdf"));
        assert!(hits(&r, "/d/a.Pdf"));
        assert!(hits(&r, "/d/b.DOCX"));
        assert!(!hits(&r, "/d/c.txt"));
        assert!(!hits(&r, "/d/noext"));
        assert!(!hits(&r, "/d/pdf"));
    }

    #[test]
    fn ext_not_in_is_true_without_extension() {
        let r = cond("extNotIn", json!(["exe", ".msi"]));
        assert!(hits(&r, "/d/a.pdf"));
        assert!(hits(&r, "/d/noext"));
        assert!(!hits(&r, "/d/setup.EXE"));
        assert!(!hits(&r, "/d/x.msi"));
    }

    #[test]
    fn ext_lists_malformed_never_match() {
        for t in ["extIn", "extNotIn"] {
            for v in [
                json!([]),
                json!("pdf"),
                json!([1]),
                json!(["pdf", null]),
                json!(null),
            ] {
                assert!(!hits(&cond(t, v.clone()), "/d/a.txt"), "{t} {v}");
                assert!(!hits(&cond(t, v.clone()), "/d/noext"), "{t} {v}");
            }
        }
    }

    // ------------------------------------------------------------ typeGroup

    #[test]
    fn type_group_uses_fixed_lists() {
        let inst = cond("typeGroup", json!("installer"));
        assert!(hits(&inst, "/d/Setup.EXE"));
        assert!(hits(&inst, "/d/app.msixbundle"));
        assert!(!hits(&inst, "/d/a.zip"));
        assert!(!hits(&inst, "/d/exe"));
        let arch = cond("typeGroup", json!("archive"));
        assert!(hits(&arch, "/d/a.tar.gz"));
        assert!(hits(&cond("typeGroup", json!("document")), "/d/x.PDF"));
        assert!(hits(&cond("typeGroup", json!("image")), "/d/x.jpeg"));
    }

    #[test]
    fn type_group_malformed_never_matches() {
        for v in [
            json!("Installer"),
            json!("other"),
            json!(["installer"]),
            json!(null),
        ] {
            assert!(!hits(&cond("typeGroup", v.clone()), "/d/a.exe"), "{v}");
        }
    }

    // ------------------------------------------------------------ ages

    #[test]
    fn modified_older_than_days() {
        let r = cond("modifiedOlderThanDays", json!(30));
        assert!(rule_matches(&r, &aged("/d/a", at(30, -1), None)));
        assert!(rule_matches(&r, &aged("/d/a", at(400, 0), None)));
        assert!(
            !rule_matches(&r, &aged("/d/a", at(30, 0), None)),
            "exactly 30 days"
        );
        assert!(!rule_matches(&r, &aged("/d/a", at(29, 0), None)));
        // Future mtime, missing mtime, missing clock.
        assert!(!rule_matches(&r, &aged("/d/a", at(0, 10), None)));
        assert!(!rule_matches(&r, &aged("/d/a", None, at(400, 0))));
        let mut no_clock = aged("/d/a", at(400, 0), None);
        no_clock.now = None;
        assert!(!rule_matches(&r, &no_clock));
    }

    #[test]
    fn zero_days_means_any_past_time() {
        let r = cond("modifiedOlderThanDays", json!(0));
        assert!(rule_matches(&r, &aged("/d/a", at(0, -1), None)));
        assert!(!rule_matches(&r, &aged("/d/a", at(0, 0), None)));
    }

    #[test]
    fn created_older_than_days_uses_created_and_is_false_without_it() {
        let r = cond("createdOlderThanDays", json!(7));
        assert!(rule_matches(&r, &aged("/d/a", at(0, 0), at(8, 0))));
        assert!(!rule_matches(&r, &aged("/d/a", at(100, 0), at(6, 0))));
        assert!(!rule_matches(&r, &aged("/d/a", at(100, 0), None)));
    }

    #[test]
    fn age_malformed_never_matches() {
        for t in ["modifiedOlderThanDays", "createdOlderThanDays"] {
            for v in [
                json!(-1),
                json!(1.5),
                json!("7"),
                json!(null),
                json!(u64::MAX),
            ] {
                let r = cond(t, v.clone());
                assert!(
                    !rule_matches(&r, &aged("/d/a", at(999, 0), at(999, 0))),
                    "{t} {v}"
                );
            }
        }
    }

    // ------------------------------------------------------------ emptyFolder

    fn dir(path: &str, empty: bool) -> FileFacts<'_> {
        FileFacts {
            path,
            is_dir: true,
            is_empty_dir: empty,
            ..Default::default()
        }
    }

    #[test]
    fn empty_folder_matches_only_empty_directories() {
        let r = cond("emptyFolder", json!(true));
        assert!(rule_matches(&r, &dir("/d/x", true)));
        assert!(!rule_matches(&r, &dir("/d/x", false)));
        assert!(!hits(&r, "/d/x"), "a file is never an empty folder");
    }

    #[test]
    fn empty_folder_malformed_never_matches() {
        for v in [json!(false), json!("true"), json!(1), json!(null)] {
            assert!(
                !rule_matches(&cond("emptyFolder", v.clone()), &dir("/d/x", true)),
                "{v}"
            );
        }
    }

    #[test]
    fn rules_without_empty_folder_see_files_only() {
        let all = rule_with(json!([]));
        assert!(hits(&all, "/d/a"));
        assert!(!rule_matches(&all, &dir("/d/x", true)));
        let mut always = cond("ext", json!("pdf"));
        always.always_apply = true;
        assert!(!rule_matches(&always, &dir("/d/x.pdf", true)));
        assert!(is_folder_rule(&cond("emptyFolder", json!(false))));
        assert!(!is_folder_rule(&all));
    }

    #[test]
    fn empty_folder_combines_with_age() {
        let r = rule_with(json!([
            { "type": "emptyFolder", "value": true },
            { "type": "modifiedOlderThanDays", "value": 7 },
        ]));
        let mut d = dir("/d/x", true);
        d.now = Some(SystemTime::UNIX_EPOCH + NOW);
        d.mtime = at(8, 0);
        assert!(rule_matches(&r, &d));
        d.mtime = at(6, 0);
        assert!(!rule_matches(&r, &d));
    }

    // ------------------------------------------------------------ duplicate

    #[test]
    fn duplicate_true_is_left_to_the_planner() {
        let r = rule_with(json!([
            { "type": "duplicate", "value": true },
            { "type": "ext", "value": "pdf" },
        ]));
        assert!(hits(&r, "/d/a.pdf"));
        assert!(!hits(&r, "/d/a.txt"), "other conditions still apply");
        assert!(is_duplicate_rule(&r));
        assert!(!is_duplicate_rule(&cond("ext", json!("pdf"))));
    }

    #[test]
    fn duplicate_malformed_never_matches() {
        for v in [json!(false), json!("true"), json!(null)] {
            assert!(!hits(&cond("duplicate", v.clone()), "/d/a.pdf"), "{v}");
        }
    }

    #[test]
    fn cached_regex_gives_same_answers_as_fresh_compile() {
        for (pat, text) in [
            ("(?i)factura", "Factura_1.pdf"),
            ("(?i)factura", "other.pdf"),
            ("^a.*z$", "abcz"),
            ("[unclosed", "[unclosed"),
            ("", "anything"),
        ] {
            let fresh = Regex::new(pat).map(|re| re.is_match(text)).unwrap_or(false);
            assert_eq!(regex_is_match(pat, text), fresh, "{pat} on {text}");
            assert_eq!(regex_is_match(pat, text), fresh, "{pat} on {text} (cached)");
        }
    }

    #[test]
    fn cache_is_emptied_at_cap_and_still_answers() {
        for i in 0..REGEX_CACHE_CAP + 5 {
            assert!(regex_is_match(&format!("^p{i}$"), &format!("p{i}")));
        }
        REGEX_CACHE.with(|c| assert!(c.borrow().len() <= REGEX_CACHE_CAP));
        assert!(!regex_is_match("[bad", "[bad"));
    }
}
