//! Deterministic collision-free destination names. Valet never overwrites:
//! when `dest` is taken, the file gets `stem (2).ext`, `stem (3).ext`, ...

use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

/// Returns `dest` if `taken(dest)` is false, else the first free candidate of
/// `stem (2).ext`, `stem (3).ext`, ... in the same folder.
///
/// Only the last extension is kept apart (`name.tar.gz` -> `name.tar (2).gz`);
/// a name without extension gets the suffix at the end (`name (2)`).
/// The result depends only on `dest` and `taken`.
pub fn next_free_name(dest: &Path, taken: impl Fn(&Path) -> bool) -> PathBuf {
    if !taken(dest) {
        return dest.to_path_buf();
    }
    let parent = dest.parent().unwrap_or_else(|| Path::new(""));
    let stem = dest
        .file_stem()
        .map(OsString::from)
        .unwrap_or_else(|| OsString::from(""));
    let ext = dest.extension();
    let mut n: u64 = 2;
    loop {
        let mut name = stem.clone();
        name.push(format!(" ({n})"));
        if let Some(ext) = ext {
            name.push(".");
            name.push(ext);
        }
        let candidate = parent.join(name);
        if !taken(&candidate) {
            return candidate;
        }
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn set(paths: &[&str]) -> HashSet<PathBuf> {
        paths.iter().map(PathBuf::from).collect()
    }

    #[test]
    fn free_dest_is_returned_unchanged() {
        let taken = set(&[]);
        let d = Path::new("dir/a.pdf");
        assert_eq!(next_free_name(d, |p| taken.contains(p)), d);
    }

    #[test]
    fn taken_dest_gets_2_then_3() {
        let taken = set(&["dir/a.pdf"]);
        assert_eq!(
            next_free_name(Path::new("dir/a.pdf"), |p| taken.contains(p)),
            PathBuf::from("dir/a (2).pdf")
        );
        let taken = set(&["dir/a.pdf", "dir/a (2).pdf"]);
        assert_eq!(
            next_free_name(Path::new("dir/a.pdf"), |p| taken.contains(p)),
            PathBuf::from("dir/a (3).pdf")
        );
    }

    #[test]
    fn gaps_are_filled_first() {
        let taken = set(&["dir/a.pdf", "dir/a (3).pdf"]);
        assert_eq!(
            next_free_name(Path::new("dir/a.pdf"), |p| taken.contains(p)),
            PathBuf::from("dir/a (2).pdf")
        );
    }

    #[test]
    fn double_extension_keeps_only_last_apart() {
        let taken = set(&["dir/name.tar.gz"]);
        assert_eq!(
            next_free_name(Path::new("dir/name.tar.gz"), |p| taken.contains(p)),
            PathBuf::from("dir/name.tar (2).gz")
        );
    }

    #[test]
    fn no_extension_gets_suffix_at_end() {
        let taken = set(&["dir/README"]);
        assert_eq!(
            next_free_name(Path::new("dir/README"), |p| taken.contains(p)),
            PathBuf::from("dir/README (2)")
        );
    }

    #[test]
    fn dot_file_is_treated_as_stem() {
        let taken = set(&["dir/.env"]);
        assert_eq!(
            next_free_name(Path::new("dir/.env"), |p| taken.contains(p)),
            PathBuf::from("dir/.env (2)")
        );
    }

    #[test]
    fn same_inputs_give_same_output() {
        let taken = set(&["d/x.txt", "d/x (2).txt", "d/x (4).txt"]);
        let a = next_free_name(Path::new("d/x.txt"), |p| taken.contains(p));
        let b = next_free_name(Path::new("d/x.txt"), |p| taken.contains(p));
        assert_eq!(a, b);
        assert_eq!(a, PathBuf::from("d/x (3).txt"));
    }
}
