use crate::rules::Rule;
use regex::Regex;
use std::{cell::RefCell, collections::HashMap, path::Path};

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

pub struct FileFacts<'a> {
    pub path: &'a str,
    pub size: u64,
}

pub fn rule_matches(rule: &Rule, f: &FileFacts) -> bool {
    if !rule.enabled {
        return false;
    }
    if rule.always_apply {
        return true;
    }

    let file_name = Path::new(f.path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    rule.conditions.iter().all(|c| match c.r#type.as_str() {
        "ext" => {
            let want = c.value.as_str().unwrap_or_default().to_ascii_lowercase();
            Path::new(f.path)
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case(&want))
                .unwrap_or(false)
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
