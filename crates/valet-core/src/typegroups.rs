//! Fixed extension lists for the `typeGroup` condition (lowercase, no dot).
//!
//! A file belongs to a group when its last extension, lowercased, is in the
//! group's list (`archive.tar.gz` counts as `gz`, an archive). A file without
//! an extension belongs to no group. The lists do not overlap.
//!
//! - `image`: jpg, jpeg, png, gif, bmp, webp, tif, tiff, heic, heif, avif,
//!   svg, ico, psd, raw, cr2, nef, arw, dng
//! - `document`: pdf, doc, docx, xls, xlsx, ppt, pptx, odt, ods, odp, rtf,
//!   txt, md, csv, epub
//! - `archive`: zip, rar, 7z, tar, gz, tgz, bz2, xz, zst, cab, iso
//! - `installer`: exe, msi, msix, msixbundle, appx, appxbundle
//! - `video`: mp4, mkv, avi, mov, wmv, webm, m4v, mpg, mpeg, flv, 3gp
//! - `audio`: mp3, wav, flac, aac, ogg, m4a, wma, opus, aiff, mid, midi
//! - `code`: rs, py, js, ts, jsx, tsx, java, kt, swift, c, h, cpp, hpp, cs,
//!   go, rb, php, sh, ps1, bat, cmd, html, css, json, xml, yaml, yml, toml,
//!   sql, ipynb

/// Group names accepted by `typeGroup`, in UI order.
pub const GROUPS: &[&str] = &[
    "image",
    "document",
    "archive",
    "installer",
    "video",
    "audio",
    "code",
];

const IMAGE: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "bmp", "webp", "tif", "tiff", "heic", "heif", "avif", "svg",
    "ico", "psd", "raw", "cr2", "nef", "arw", "dng",
];
const DOCUMENT: &[&str] = &[
    "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "odt", "ods", "odp", "rtf", "txt", "md",
    "csv", "epub",
];
const ARCHIVE: &[&str] = &[
    "zip", "rar", "7z", "tar", "gz", "tgz", "bz2", "xz", "zst", "cab", "iso",
];
const INSTALLER: &[&str] = &["exe", "msi", "msix", "msixbundle", "appx", "appxbundle"];
const VIDEO: &[&str] = &[
    "mp4", "mkv", "avi", "mov", "wmv", "webm", "m4v", "mpg", "mpeg", "flv", "3gp",
];
const AUDIO: &[&str] = &[
    "mp3", "wav", "flac", "aac", "ogg", "m4a", "wma", "opus", "aiff", "mid", "midi",
];
const CODE: &[&str] = &[
    "rs", "py", "js", "ts", "jsx", "tsx", "java", "kt", "swift", "c", "h", "cpp", "hpp", "cs",
    "go", "rb", "php", "sh", "ps1", "bat", "cmd", "html", "css", "json", "xml", "yaml", "yml",
    "toml", "sql", "ipynb",
];

/// Extensions of `group` (exact, lowercase name); `None` for an unknown group.
pub fn extensions(group: &str) -> Option<&'static [&'static str]> {
    Some(match group {
        "image" => IMAGE,
        "document" => DOCUMENT,
        "archive" => ARCHIVE,
        "installer" => INSTALLER,
        "video" => VIDEO,
        "audio" => AUDIO,
        "code" => CODE,
        _ => return None,
    })
}

/// `ext` (lowercase, no dot) is in `group`. Unknown group: false.
pub fn contains(group: &str, ext: &str) -> bool {
    extensions(group).is_some_and(|list| list.contains(&ext))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn every_group_has_a_list_and_lists_do_not_overlap() {
        let mut seen = HashSet::new();
        for g in GROUPS {
            let list = extensions(g).unwrap_or_else(|| panic!("{g} has no list"));
            assert!(!list.is_empty(), "{g}");
            for e in list {
                assert_eq!(*e, e.to_lowercase(), "{g}: {e} not lowercase");
                assert!(!e.starts_with('.'), "{g}: {e} has a dot");
                assert!(seen.insert(*e), "{e} is in two groups");
            }
        }
    }

    #[test]
    fn required_members() {
        for e in ["exe", "msi", "msix", "msixbundle", "appx", "appxbundle"] {
            assert!(contains("installer", e), "{e}");
        }
        for e in [
            "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "odt", "ods", "odp", "rtf", "txt",
            "md", "csv", "epub",
        ] {
            assert!(contains("document", e), "{e}");
        }
    }

    #[test]
    fn unknown_group_contains_nothing() {
        assert!(extensions("Image").is_none());
        assert!(!contains("other", "pdf"));
        assert!(!contains("", "pdf"));
    }
}
