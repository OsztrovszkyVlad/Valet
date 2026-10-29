//! Text shown by the Rust side (tray menu, notifications) in the language
//! chosen in `app_settings.json` (`language`: "ro" default, "en").
//! The UI itself uses the frontend i18n files.

use tauri::{Manager, Runtime};

use crate::state::config_dir;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Lang {
    Ro,
    En,
}

impl Lang {
    /// "en" and "en-*" are English; anything else is Romanian.
    pub(crate) fn from_code(code: &str) -> Self {
        let code = code.trim().to_ascii_lowercase();
        if code == "en" || code.starts_with("en-") || code.starts_with("en_") {
            Lang::En
        } else {
            Lang::Ro
        }
    }
}

pub(crate) struct Text {
    pub(crate) tray_tooltip: &'static str,
    pub(crate) tray_show: &'static str,
    pub(crate) tray_pause: &'static str,
    pub(crate) tray_resume: &'static str,
    pub(crate) tray_quit: &'static str,
    pub(crate) batch_title: &'static str,
}

const RO: Text = Text {
    tray_tooltip: "Valet",
    tray_show: "Arată Valet",
    tray_pause: "Pauză organizare automată",
    tray_resume: "Reia organizarea automată",
    tray_quit: "Ieșire",
    batch_title: "Valet",
};

const EN: Text = Text {
    tray_tooltip: "Valet",
    tray_show: "Show Valet",
    tray_pause: "Pause automatic organizing",
    tray_resume: "Resume automatic organizing",
    tray_quit: "Quit",
    batch_title: "Valet",
};

pub(crate) fn text(lang: Lang) -> &'static Text {
    match lang {
        Lang::Ro => &RO,
        Lang::En => &EN,
    }
}

/// Romanian count phrase: "1 fișier", "5 fișiere", "20 de fișiere".
/// "de" goes before the noun when the last two digits are 00 or 20..99
/// (and the number is at least 20).
fn ro_count(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else if n >= 20 && (n.is_multiple_of(100) || n % 100 >= 20) {
        format!("{n} de {many}")
    } else {
        format!("{n} {many}")
    }
}

/// Body of the notification after a batch: how many files were organized
/// and, if any, how many failed.
pub(crate) fn batch_body(lang: Lang, done: usize, failed: usize) -> String {
    match lang {
        Lang::Ro => {
            let mut s = ro_count(done, "fișier organizat", "fișiere organizate");
            if failed > 0 {
                let verb = if failed == 1 {
                    "nu a putut fi mutat"
                } else {
                    "nu au putut fi mutate"
                };
                s.push_str(&format!(
                    ", {} {verb}",
                    ro_count(failed, "fișier", "fișiere")
                ));
            }
            s
        }
        Lang::En => {
            let mut s = if done == 1 {
                "1 file organized".to_string()
            } else {
                format!("{done} files organized")
            };
            if failed > 0 {
                s.push_str(&format!(", {failed} failed"));
            }
            s
        }
    }
}

/// `app_settings.json` as loose JSON, or `None` if missing or unreadable.
/// Read field by field so this does not depend on the settings struct.
pub(crate) fn app_settings_json<R: Runtime, M: Manager<R>>(app: &M) -> Option<serde_json::Value> {
    let path = config_dir(app).ok()?.join("app_settings.json");
    let txt = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&txt).ok()
}

/// The UI language from `app_settings.json`; Romanian when unset.
pub(crate) fn current_lang<R: Runtime, M: Manager<R>>(app: &M) -> Lang {
    app_settings_json(app)
        .as_ref()
        .and_then(|v| v.get("language"))
        .and_then(|v| v.as_str())
        .map(Lang::from_code)
        .unwrap_or(Lang::Ro)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_codes() {
        assert_eq!(Lang::from_code("ro"), Lang::Ro);
        assert_eq!(Lang::from_code("en"), Lang::En);
        assert_eq!(Lang::from_code("EN-us"), Lang::En);
        assert_eq!(Lang::from_code(""), Lang::Ro);
        assert_eq!(Lang::from_code("de"), Lang::Ro);
    }

    #[test]
    fn tray_labels() {
        assert_eq!(text(Lang::Ro).tray_show, "Arată Valet");
        assert_eq!(text(Lang::Ro).tray_pause, "Pauză organizare automată");
        assert_eq!(text(Lang::Ro).tray_resume, "Reia organizarea automată");
        assert_eq!(text(Lang::Ro).tray_quit, "Ieșire");
        assert_eq!(text(Lang::En).tray_quit, "Quit");
    }

    #[test]
    fn romanian_counts() {
        assert_eq!(batch_body(Lang::Ro, 1, 0), "1 fișier organizat");
        assert_eq!(batch_body(Lang::Ro, 3, 0), "3 fișiere organizate");
        assert_eq!(batch_body(Lang::Ro, 19, 0), "19 fișiere organizate");
        assert_eq!(batch_body(Lang::Ro, 20, 0), "20 de fișiere organizate");
        assert_eq!(batch_body(Lang::Ro, 101, 0), "101 fișiere organizate");
        assert_eq!(batch_body(Lang::Ro, 120, 0), "120 de fișiere organizate");
        assert_eq!(batch_body(Lang::Ro, 200, 0), "200 de fișiere organizate");
        assert_eq!(
            batch_body(Lang::Ro, 5, 1),
            "5 fișiere organizate, 1 fișier nu a putut fi mutat"
        );
        assert_eq!(
            batch_body(Lang::Ro, 5, 2),
            "5 fișiere organizate, 2 fișiere nu au putut fi mutate"
        );
    }

    #[test]
    fn english_counts() {
        assert_eq!(batch_body(Lang::En, 1, 0), "1 file organized");
        assert_eq!(batch_body(Lang::En, 4, 2), "4 files organized, 2 failed");
    }
}
