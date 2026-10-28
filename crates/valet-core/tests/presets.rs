//! Each shipped preset on a synthetic home folder (PLAN.md 2.4): install,
//! plan, assert the exact operations, reports and skips, execute with
//! `FakePlatform`, undo, and compare the tree with the original. Then the
//! interactions between presets.

use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use chrono::{DateTime, Datelike, Local};
use valet_core::{
    executor::{self, EMPTY_DIR_MARK},
    planner::{self, OpKind, Plan, PlanContext, SkipReason},
    platform::testing::{FakePlatform, set_mtime},
    presets::{self, Lang},
    rules::Rule,
    storage::Db,
};

const DAY: u64 = 86_400;

struct Home {
    tmp: tempfile::TempDir,
    now: SystemTime,
}

impl Home {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        for d in ["Downloads", "Desktop", "Documents", "Pictures"] {
            fs::create_dir_all(tmp.path().join("home").join(d)).unwrap();
        }
        Self {
            tmp,
            now: SystemTime::now(),
        }
    }

    fn home(&self) -> PathBuf {
        self.tmp.path().join("home")
    }

    fn p(&self, rel: &str) -> PathBuf {
        self.home().join(rel)
    }

    /// mtime `days` days (plus one hour) before `now`.
    fn ago(&self, days: u64) -> SystemTime {
        self.now - Duration::from_secs(days * DAY + 3600)
    }

    /// Writes `rel` with unique content and an mtime `days` days back.
    fn file(&self, rel: &str, days: u64) -> PathBuf {
        let path = self.p(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, format!("content of {rel}")).unwrap();
        set_mtime(&path, self.ago(days));
        path
    }

    /// Creates the empty folder `rel` with an mtime `days` days back.
    fn dir(&self, rel: &str, days: u64) -> PathBuf {
        let path = self.p(rel);
        fs::create_dir_all(&path).unwrap();
        set_mtime(&path, self.ago(days));
        path
    }

    fn ctx(&self) -> PlanContext {
        let folders: HashMap<String, PathBuf> = [
            ("home", ""),
            ("downloads", "Downloads"),
            ("desktop", "Desktop"),
            ("documents", "Documents"),
            ("pictures", "Pictures"),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), self.p(v)))
        .collect();
        PlanContext {
            now: self.now,
            home_dir: Some(self.home()),
            env: Box::new(|_| None),
            folders,
        }
    }

    /// The watched folders chosen in onboarding.
    fn roots(&self) -> Vec<PathBuf> {
        ["Downloads", "Desktop", "Documents", "Pictures"]
            .iter()
            .map(|d| self.p(d))
            .collect()
    }

    fn rel(&self, p: &Path) -> String {
        p.strip_prefix(self.home())
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/")
    }

    fn trash_dir(&self) -> PathBuf {
        self.tmp.path().join("trash")
    }

    fn platform(&self) -> FakePlatform {
        FakePlatform::with_trash_dir(self.trash_dir())
    }

    async fn db(&self) -> Db {
        Db::connect(&self.tmp.path().join("state").join("valet.sqlite3"))
            .await
            .unwrap()
    }
}

fn year(t: SystemTime) -> String {
    format!("{:04}", DateTime::<Local>::from(t).year())
}

fn year_month(t: SystemTime) -> String {
    let d = DateTime::<Local>::from(t);
    format!("{:04}-{:02}", d.year(), d.month())
}

/// Sorted (relative path, content) of every file under `dir`.
fn tree(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    walkdir::WalkDir::new(dir)
        .into_iter()
        .map(|e| e.unwrap())
        .filter(|e| e.file_type().is_file())
        .map(|e| {
            (
                e.path().strip_prefix(dir).unwrap().to_path_buf(),
                fs::read(e.path()).unwrap(),
            )
        })
        .collect()
}

/// Installs `ids` (Romanian) into a fresh database and returns the stored rules.
async fn installed(h: &Home, ids: &[&str]) -> (Db, Vec<Rule>) {
    let db = h.db().await;
    let ids: Vec<String> = ids.iter().map(|s| s.to_string()).collect();
    presets::install(&db, &ids, Lang::Ro).await.unwrap();
    let rules = db.list_rules().await.unwrap();
    (db, rules)
}

fn plan(h: &Home, rules: &[Rule]) -> Plan {
    planner::plan(&h.roots(), rules, &h.platform(), &h.ctx())
}

/// (source, kind, destination) relative to the home folder; "" for trash.
fn ops(h: &Home, p: &Plan) -> Vec<(String, OpKind, String)> {
    let mut v: Vec<_> = p
        .ops
        .iter()
        .map(|o| {
            let dest = if o.kind == OpKind::Trash {
                String::new()
            } else {
                h.rel(&o.dest)
            };
            (h.rel(&o.source), o.kind, dest)
        })
        .collect();
    sort_ops(&mut v);
    v
}

fn sort_ops(v: &mut [(String, OpKind, String)]) {
    v.sort_by(|a, b| (&a.0, &a.2).cmp(&(&b.0, &b.2)));
}

fn op(src: &str, kind: OpKind, dest: &str) -> (String, OpKind, String) {
    (src.to_owned(), kind, dest.to_owned())
}

fn reports(h: &Home, p: &Plan) -> Vec<(String, String)> {
    let mut v: Vec<_> = p
        .reports
        .iter()
        .map(|r| (h.rel(&r.path), r.rule_name.clone()))
        .collect();
    v.sort();
    v
}

/// Skips that name a rule (the reasons a preset left a file alone).
fn rule_skips(h: &Home, p: &Plan) -> Vec<(String, SkipReason)> {
    let mut v: Vec<_> = p
        .skipped
        .iter()
        .filter(|s| s.rule_name.is_some())
        .map(|s| (h.rel(&s.path), s.reason))
        .collect();
    v.sort_by_key(|(p, r)| (p.clone(), format!("{r:?}")));
    v
}

/// Executes the plan, checks every op is done, undoes the batch and checks
/// that the home tree is byte-identical to `before`.
async fn execute_and_undo(h: &Home, db: &Db, p: &Plan, before: &BTreeMap<PathBuf, Vec<u8>>) {
    let platform = h.platform();
    let journal = db.journal();
    let rep = executor::execute(&p.ops, &journal, &platform)
        .await
        .unwrap();
    assert_eq!(rep.failed_count(), 0, "{:?}", rep.results);
    for o in &p.ops {
        assert!(!o.source.exists(), "{} still there", o.source.display());
    }
    let u = executor::undo_batch(&rep.batch_id, &journal, &platform, false)
        .await
        .unwrap();
    assert_eq!(u.undone.len(), p.ops.len(), "{u:?}");
    assert!(u.failed.is_empty() && u.needs_confirmation.is_empty());
    assert!(u.restored_as.is_empty() && u.restored_modified.is_empty());
    assert_eq!(&tree(&h.home()), before, "undo restores the tree");
}

// ---------------------------------------------------------------- one test per preset

#[tokio::test]
async fn preset_downloads_by_type() {
    let h = Home::new();
    for f in [
        "a.jpg",
        "b.pdf",
        "c.zip",
        "d.exe",
        "e.mp4",
        "f.mp3",
        "notes.xyz",
    ] {
        h.file(&format!("Downloads/{f}"), 1);
    }
    h.file("Downloads/sub/inner.jpg", 1);
    h.file("Downloads/Imagini/old.png", 1);
    h.file("Desktop/photo.jpg", 1);
    let before = tree(&h.home());
    let (db, rules) = installed(&h, &["downloads-by-type"]).await;
    let p = plan(&h, &rules);
    assert_eq!(
        ops(&h, &p),
        [
            op("Downloads/a.jpg", OpKind::Move, "Downloads/Imagini/a.jpg"),
            op("Downloads/b.pdf", OpKind::Move, "Downloads/Documente/b.pdf"),
            op("Downloads/c.zip", OpKind::Move, "Downloads/Arhive/c.zip"),
            op(
                "Downloads/d.exe",
                OpKind::Move,
                "Downloads/Instalatoare/d.exe"
            ),
            op("Downloads/e.mp4", OpKind::Move, "Downloads/Video/e.mp4"),
            op("Downloads/f.mp3", OpKind::Move, "Downloads/Audio/f.mp3"),
        ]
    );
    assert!(p.reports.is_empty());
    assert!(rule_skips(&h, &p).is_empty());
    execute_and_undo(&h, &db, &p, &before).await;

    // English folder names.
    let db_en = Db::connect(&h.tmp.path().join("en.sqlite3")).await.unwrap();
    presets::install(&db_en, &["downloads-by-type".to_string()], Lang::En)
        .await
        .unwrap();
    let p = plan(&h, &db_en.list_rules().await.unwrap());
    let dests: Vec<String> = ops(&h, &p).into_iter().map(|o| o.2).collect();
    assert_eq!(
        dests,
        [
            "Downloads/Images/a.jpg",
            "Downloads/Documents/b.pdf",
            "Downloads/Archives/c.zip",
            "Downloads/Installers/d.exe",
            "Downloads/Videos/e.mp4",
            "Downloads/Music/f.mp3",
        ]
    );
}

#[tokio::test]
async fn preset_screenshots() {
    let h = Home::new();
    let a = h.file("Downloads/Screenshot 2024-05-01 101010.png", 3);
    let b = h.file("Desktop/Captură de ecran 2025-01-02.png", 3);
    let c = h.file("Pictures/Snip_1.jpg", 3);
    let d = h.file("Desktop/clipboard.png", 3);
    let e = h.file("Downloads/Captura-ecran.webp", 3);
    for f in [
        "Downloads/Snippet.png",
        "Downloads/Screenshot.pdf",
        "Downloads/screenshots-tutorial.png",
        "Pictures/Screenshots/Screenshot 1.png",
        "Pictures/Vacanta/Screenshot 2.png",
    ] {
        h.file(f, 3);
    }
    let before = tree(&h.home());
    let (db, rules) = installed(&h, &["screenshots"]).await;
    let p = plan(&h, &rules);
    let dest = |f: &Path| {
        format!(
            "Pictures/Capturi de ecran/{}/{}",
            year(h.ago(3)),
            f.file_name().unwrap().to_string_lossy()
        )
    };
    let mut want: Vec<_> = [&a, &b, &c, &d, &e]
        .iter()
        .map(|f| (h.rel(f), OpKind::Move, dest(f)))
        .collect();
    sort_ops(&mut want);
    assert_eq!(ops(&h, &p), want);
    assert!(rule_skips(&h, &p).is_empty());
    execute_and_undo(&h, &db, &p, &before).await;
}

#[tokio::test]
async fn preset_invoices() {
    let h = Home::new();
    let hits = [
        "Downloads/Factura_123.pdf",
        "Downloads/chitanță-mai.pdf",
        "Downloads/Invoice2024.pdf",
        "Downloads/bon fiscal.pdf",
        "Downloads/FACTURĂ 7.PDF",
        "Downloads/Receipts-2025.pdf",
        "Downloads/sub/receipt.pdf",
    ];
    for f in hits {
        h.file(f, 2);
    }
    for f in [
        "Downloads/bonus.pdf",
        "Downloads/factura.docx",
        "Downloads/refacturare.pdf",
        "Desktop/factura.pdf",
    ] {
        h.file(f, 2);
    }
    let before = tree(&h.home());
    let (db, rules) = installed(&h, &["invoices"]).await;
    let p = plan(&h, &rules);
    let y = year(h.ago(2));
    let mut want: Vec<_> = hits
        .iter()
        .map(|f| {
            let name = Path::new(f).file_name().unwrap().to_string_lossy();
            (
                f.to_string(),
                OpKind::Move,
                format!("Documents/Facturi/{y}/{name}"),
            )
        })
        .collect();
    sort_ops(&mut want);
    assert_eq!(ops(&h, &p), want);
    execute_and_undo(&h, &db, &p, &before).await;
}

#[tokio::test]
async fn preset_large_files() {
    let h = Home::new();
    let gib = 1u64 << 30;
    // Sparse files: the size counts, the disk stays free.
    for (rel, len) in [
        ("Downloads/big.iso", gib + 1),
        ("Documents/film.mkv", gib + 5),
        ("Downloads/exactly.bin", gib),
        ("Desktop/huge.zip", gib + 1),
    ] {
        let path = h.p(rel);
        let f = fs::File::create(&path).unwrap();
        f.set_len(len).unwrap();
        f.set_modified(h.ago(2)).unwrap();
    }
    h.file("Downloads/small.txt", 2);
    let (db, rules) = installed(&h, &["large-files"]).await;
    let p = plan(&h, &rules);
    assert!(p.ops.is_empty());
    assert_eq!(
        reports(&h, &p),
        [
            ("Documents/film.mkv".to_string(), "Fișiere mari".to_string()),
            ("Downloads/big.iso".to_string(), "Fișiere mari".to_string()),
        ]
    );
    let rep = executor::execute(&p.ops, &db.journal(), &h.platform())
        .await
        .unwrap();
    assert!(rep.results.is_empty());
    assert_eq!(
        fs::metadata(h.p("Downloads/big.iso")).unwrap().len(),
        gib + 1
    );
}

#[tokio::test]
async fn preset_duplicates() {
    let h = Home::new();
    for rel in ["Downloads/a.txt", "Downloads/sub/copy of a.txt"] {
        let path = h.p(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"same bytes").unwrap();
        set_mtime(&path, h.ago(2));
    }
    h.file("Downloads/c.txt", 2);
    let before = tree(&h.home());
    let (_db, rules) = installed(&h, &["duplicates"]).await;
    let p = plan(&h, &rules);
    assert!(p.ops.is_empty());
    assert_eq!(
        reports(&h, &p),
        [
            ("Downloads/a.txt".to_string(), "Duplicate".to_string()),
            (
                "Downloads/sub/copy of a.txt".to_string(),
                "Duplicate".to_string()
            ),
        ]
    );
    assert_eq!(p.reports[0].group, p.reports[1].group);
    assert!(p.reports[0].group.is_some());
    assert_eq!(tree(&h.home()), before);
}

#[tokio::test]
async fn preset_clean_desktop() {
    let h = Home::new();
    let old = h.file("Desktop/old.docx", 10);
    let noext = h.file("Desktop/README", 8);
    for (f, days) in [
        ("Desktop/new.docx", 2),
        ("Desktop/Chrome.lnk", 30),
        ("Desktop/site.url", 30),
        ("Desktop/desktop.ini", 30),
        ("Desktop/Proiect/inner.txt", 30),
        ("Downloads/old.docx", 30),
    ] {
        h.file(f, days);
    }
    let before = tree(&h.home());
    let (db, rules) = installed(&h, &["clean-desktop"]).await;
    let p = plan(&h, &rules);
    assert_eq!(
        ops(&h, &p),
        [
            op(
                "Desktop/README",
                OpKind::Move,
                &format!("Documents/De pe Desktop/{}/README", year_month(h.ago(8)))
            ),
            op(
                "Desktop/old.docx",
                OpKind::Move,
                &format!("Documents/De pe Desktop/{}/old.docx", year_month(h.ago(10)))
            ),
        ]
    );
    let _ = (old, noext);
    execute_and_undo(&h, &db, &p, &before).await;
}

#[tokio::test]
async fn preset_old_installers() {
    let h = Home::new();
    let trashed = [
        h.file("Downloads/old.exe", 40),
        h.file("Downloads/Instalatoare/older.msi", 40),
        h.file("Downloads/setup.msix", 31),
        h.file("Downloads/app.appx", 60),
    ];
    h.file("Downloads/new.exe", 2);
    h.file("Downloads/old.zip", 40);
    h.file("Desktop/old.exe", 40);
    let before = tree(&h.home());
    let (db, rules) = installed(&h, &["old-installers"]).await;
    let p = plan(&h, &rules);
    let mut want: Vec<_> = trashed
        .iter()
        .map(|f| (h.rel(f), OpKind::Trash, String::new()))
        .collect();
    sort_ops(&mut want);
    assert_eq!(ops(&h, &p), want);
    execute_and_undo(&h, &db, &p, &before).await;
}

#[tokio::test]
async fn preset_empty_folders() {
    let h = Home::new();
    h.file("Downloads/full/a.txt", 30);
    let nested = h.dir("Downloads/a/b", 10);
    let gol = h.dir("Downloads/Gol", 10);
    let fresh = h.dir("Downloads/Nou", 2);
    let desk = h.dir("Desktop/Gol", 10);
    set_mtime(&h.p("Downloads/a"), h.ago(10));
    let before = tree(&h.home());
    let (db, rules) = installed(&h, &["empty-folders"]).await;
    let p = plan(&h, &rules);
    assert_eq!(
        ops(&h, &p),
        [
            op("Downloads/Gol", OpKind::Trash, ""),
            op("Downloads/a/b", OpKind::Trash, ""),
        ]
    );
    let platform = h.platform();
    let journal = db.journal();
    let rep = executor::execute(&p.ops, &journal, &platform)
        .await
        .unwrap();
    assert_eq!(rep.failed_count(), 0, "{:?}", rep.results);
    assert!(!gol.exists() && !nested.exists());
    assert!(fresh.is_dir() && desk.is_dir());
    for r in &rep.results {
        let rec = journal.get_op(r.op_id).await.unwrap().unwrap();
        assert_eq!(rec.hash_before.as_deref(), Some(EMPTY_DIR_MARK));
    }
    let u = executor::undo_batch(&rep.batch_id, &journal, &platform, false)
        .await
        .unwrap();
    assert_eq!(u.undone.len(), 2, "{u:?}");
    assert!(gol.is_dir() && nested.is_dir());
    assert_eq!(tree(&h.home()), before);
}

// ---------------------------------------------------------------- interactions

#[tokio::test]
async fn invoice_in_downloads_goes_to_invoices_not_documents() {
    let h = Home::new();
    h.file("Downloads/factura.pdf", 2);
    h.file("Downloads/bonus.pdf", 2);
    let (_db, rules) = installed(&h, &["downloads-by-type", "invoices"]).await;
    let p = plan(&h, &rules);
    let y = year(h.ago(2));
    assert_eq!(
        ops(&h, &p),
        [
            op(
                "Downloads/bonus.pdf",
                OpKind::Move,
                "Downloads/Documente/bonus.pdf"
            ),
            op(
                "Downloads/factura.pdf",
                OpKind::Move,
                &format!("Documents/Facturi/{y}/factura.pdf")
            ),
        ]
    );
    assert_eq!(
        rule_skips(&h, &p),
        [("Downloads/factura.pdf".to_string(), SkipReason::Conflict)]
    );
}

#[tokio::test]
async fn old_installer_goes_to_the_bin_new_one_to_installers() {
    let h = Home::new();
    h.file("Downloads/old.exe", 40);
    h.file("Downloads/new.exe", 2);
    let (_db, rules) = installed(&h, &["downloads-by-type", "old-installers"]).await;
    let p = plan(&h, &rules);
    assert_eq!(
        ops(&h, &p),
        [
            op(
                "Downloads/new.exe",
                OpKind::Move,
                "Downloads/Instalatoare/new.exe"
            ),
            op("Downloads/old.exe", OpKind::Trash, ""),
        ]
    );
}

#[tokio::test]
async fn empty_folders_keep_downloads_by_type_destinations() {
    let h = Home::new();
    h.dir("Downloads/Audio", 10);
    h.dir("Downloads/Imagini", 10);
    h.dir("Downloads/Gol", 10);
    let (_db, rules) = installed(&h, &["downloads-by-type", "empty-folders"]).await;
    let p = plan(&h, &rules);
    assert_eq!(ops(&h, &p), [op("Downloads/Gol", OpKind::Trash, "")]);
    assert_eq!(
        rule_skips(&h, &p),
        [
            ("Downloads/Audio".to_string(), SkipReason::RuleDestination),
            ("Downloads/Imagini".to_string(), SkipReason::RuleDestination),
        ]
    );

    // Without "Descărcări după tip", the empty Audio folder goes too.
    let h2 = Home::new();
    h2.dir("Downloads/Audio", 10);
    let (_db, rules) = installed(&h2, &["empty-folders"]).await;
    let p = plan(&h2, &rules);
    assert_eq!(ops(&h2, &p), [op("Downloads/Audio", OpKind::Trash, "")]);
}

#[tokio::test]
async fn old_screenshot_on_desktop_goes_to_screenshots_not_from_desktop() {
    let h = Home::new();
    h.file("Desktop/Screenshot 2024.png", 10);
    h.file("Desktop/poza.png", 10);
    let (_db, rules) = installed(&h, &["clean-desktop", "screenshots"]).await;
    let p = plan(&h, &rules);
    assert_eq!(
        ops(&h, &p),
        [
            op(
                "Desktop/Screenshot 2024.png",
                OpKind::Move,
                &format!(
                    "Pictures/Capturi de ecran/{}/Screenshot 2024.png",
                    year(h.ago(10))
                )
            ),
            op(
                "Desktop/poza.png",
                OpKind::Move,
                &format!("Documents/De pe Desktop/{}/poza.png", year_month(h.ago(10)))
            ),
        ]
    );
}

#[tokio::test]
async fn snippet_and_bonus_are_not_screenshots_or_invoices() {
    let h = Home::new();
    h.file("Downloads/Snippet.png", 2);
    h.file("Downloads/bonus.pdf", 2);
    let (_db, rules) = installed(&h, &["screenshots", "invoices"]).await;
    let p = plan(&h, &rules);
    assert!(p.ops.is_empty(), "{:?}", p.ops);
}

/// All eight presets on one home folder: every file goes to the most
/// specific preset, and undo restores the tree.
#[tokio::test]
async fn all_presets_together() {
    let h = Home::new();
    h.file("Downloads/Factura_1.pdf", 2);
    h.file("Downloads/Screenshot 1.png", 2);
    h.file("Downloads/old.exe", 40);
    h.file("Downloads/new.exe", 2);
    h.file("Downloads/a.jpg", 2);
    h.file("Desktop/old.docx", 10);
    h.file("Desktop/Captură 2.png", 10);
    h.dir("Downloads/Gol", 10);
    h.dir("Downloads/Video", 10);
    let before = tree(&h.home());
    let ids: Vec<&str> = presets::all().iter().map(|p| p.id.as_str()).collect();
    let (db, rules) = installed(&h, &ids).await;
    assert_eq!(rules.len(), 13);
    let p = plan(&h, &rules);
    let y2 = year(h.ago(2));
    let y10 = year(h.ago(10));
    let mut want = vec![
        op(
            "Desktop/Captură 2.png",
            OpKind::Move,
            &format!("Pictures/Capturi de ecran/{y10}/Captură 2.png"),
        ),
        op(
            "Desktop/old.docx",
            OpKind::Move,
            &format!("Documents/De pe Desktop/{}/old.docx", year_month(h.ago(10))),
        ),
        op(
            "Downloads/Factura_1.pdf",
            OpKind::Move,
            &format!("Documents/Facturi/{y2}/Factura_1.pdf"),
        ),
        op("Downloads/Gol", OpKind::Trash, ""),
        op(
            "Downloads/Screenshot 1.png",
            OpKind::Move,
            &format!("Pictures/Capturi de ecran/{y2}/Screenshot 1.png"),
        ),
        op("Downloads/a.jpg", OpKind::Move, "Downloads/Imagini/a.jpg"),
        op(
            "Downloads/new.exe",
            OpKind::Move,
            "Downloads/Instalatoare/new.exe",
        ),
        op("Downloads/old.exe", OpKind::Trash, ""),
    ];
    sort_ops(&mut want);
    assert_eq!(ops(&h, &p), want);
    assert!(
        rule_skips(&h, &p).contains(&("Downloads/Video".to_string(), SkipReason::RuleDestination))
    );
    execute_and_undo(&h, &db, &p, &before).await;
    assert!(h.p("Downloads/Gol").is_dir());
}
