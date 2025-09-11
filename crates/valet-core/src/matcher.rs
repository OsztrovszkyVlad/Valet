use crate::rules::Rule;
use regex::Regex;
use std::path::Path;

pub struct FileFacts<'a> {
  pub path: &'a str,
  pub size: u64,
}

pub fn rule_matches(rule: &Rule, f: &FileFacts) -> bool {
  if !rule.enabled { return false; }
  if rule.always_apply { return true; }

  let file_name = Path::new(f.path).file_name().and_then(|s| s.to_str()).unwrap_or("");

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
      Regex::new(pat).map(|re| re.is_match(file_name)).unwrap_or(false)
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