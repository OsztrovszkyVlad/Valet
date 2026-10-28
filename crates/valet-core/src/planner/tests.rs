use super::*;
use crate::platform::testing::FakePlatform;
use serde_json::{Value, json};
use std::collections::HashMap;

/// Fixed "now" one hour after real time, so freshly written fixtures are
/// older than the being-written window.
fn ctx() -> PlanContext {
    ctx_with(None, HashMap::new())
}

fn ctx_with(home: Option<PathBuf>, env: HashMap<String, String>) -> PlanContext {
    PlanContext {
        now: SystemTime::now() + Duration::from_secs(3600),
        home_dir: home,
        env: Box::new(move |k| env.get(k).cloned()),
        folders: HashMap::new(),
    }
}

fn write(path: &Path) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, b"x").unwrap();
}

fn set_mtime(path: &Path, t: SystemTime) {
    fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(t)
        .unwrap();
}

fn rule_json(n: u128, name: &str, conditions: Value, actions: Value, options: Value) -> Rule {
    serde_json::from_value(json!({
        "id": Uuid::from_u128(n),
        "name": name,
        "enabled": true,
        "conditions": conditions,
        "actions": actions,
        "options": options,
    }))
    .unwrap()
}

/// Rule matching every file (no conditions) with one action.
fn all(n: u128, name: &str, action_type: &str, path: &str) -> Rule {
    rule_json(
        n,
        name,
        json!([]),
        json!([{ "type": action_type, "params": { "path": path } }]),
        json!({}),
    )
}

fn move_all(dest: &str) -> Rule {
    all(1, "r", "moveTo", dest)
}

fn tmp_root() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("root");
    fs::create_dir_all(&root).unwrap();
    (tmp, root)
}

fn run(roots: &[PathBuf], rules: &[Rule]) -> Plan {
    plan(roots, rules, &FakePlatform::default(), &ctx())
}

fn sources(p: &Plan) -> Vec<PathBuf> {
    p.ops.iter().map(|o| o.source.clone()).collect()
}

fn skips(p: &Plan) -> Vec<(PathBuf, SkipReason, Option<String>)> {
    p.skipped
        .iter()
        .map(|s| (s.path.clone(), s.reason, s.rule_name.clone()))
        .collect()
}

fn reasons_for(p: &Plan, path: &Path) -> Vec<SkipReason> {
    p.skipped
        .iter()
        .filter(|s| s.path == path)
        .map(|s| s.reason)
        .collect()
}

// ---------------------------------------------------------------- exclusions

#[test]
fn each_default_excluded_dir_is_skipped_once_and_not_descended() {
    for name in DEFAULT_EXCLUDED_DIRS {
        let (_tmp, root) = tmp_root();
        let dir = root.join(name);
        write(&dir.join("a.pdf"));
        write(&dir.join("sub").join("b.pdf"));
        write(&root.join("keep.pdf"));
        let p = run(std::slice::from_ref(&root), &[move_all("Out")]);
        assert_eq!(sources(&p), vec![root.join("keep.pdf")], "{name}");
        assert_eq!(
            skips(&p),
            vec![(dir, SkipReason::Excluded, None)],
            "{name}: one Excluded entry for the dir, none for files inside"
        );
    }
}

#[test]
fn excluded_names_match_ascii_case_insensitively() {
    let (_tmp, root) = tmp_root();
    for name in ["NODE_MODULES", "program files", "Target", "APPDATA"] {
        write(&root.join(name).join("a.pdf"));
    }
    let p = run(std::slice::from_ref(&root), &[move_all("Out")]);
    assert!(p.ops.is_empty());
    assert_eq!(p.skipped.len(), 4);
    assert!(p.skipped.iter().all(|s| s.reason == SkipReason::Excluded));
}

#[test]
fn excluded_name_only_applies_to_directories() {
    let (_tmp, root) = tmp_root();
    write(&root.join("target"));
    let p = run(std::slice::from_ref(&root), &[move_all("Out")]);
    assert_eq!(sources(&p), vec![root.join("target")]);
}

#[test]
fn valetignore_excludes_its_dir_and_everything_below() {
    let (_tmp, root) = tmp_root();
    let sub = root.join("sub");
    write(&sub.join(IGNORE_MARKER));
    write(&sub.join("a.pdf"));
    write(&sub.join("deeper").join("b.pdf"));
    write(&root.join("keep.pdf"));
    let p = run(std::slice::from_ref(&root), &[move_all("Out")]);
    assert_eq!(sources(&p), vec![root.join("keep.pdf")]);
    assert_eq!(skips(&p), vec![(sub, SkipReason::Excluded, None)]);
}

#[test]
fn root_containing_valetignore_is_excluded() {
    let (_tmp, root) = tmp_root();
    write(&root.join(IGNORE_MARKER));
    write(&root.join("a.pdf"));
    let p = run(std::slice::from_ref(&root), &[move_all("Out")]);
    assert!(p.ops.is_empty());
    assert_eq!(skips(&p), vec![(root, SkipReason::Excluded, None)]);
}

#[test]
fn excluded_names_in_root_path_are_an_explicit_opt_in() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("node_modules").join("target");
    write(&root.join("a.pdf"));
    write(&root.join("target").join("b.pdf"));
    let p = run(std::slice::from_ref(&root), &[move_all("Out")]);
    assert_eq!(sources(&p), vec![root.join("a.pdf")]);
    assert_eq!(
        skips(&p),
        vec![(root.join("target"), SkipReason::Excluded, None)]
    );
}

// ---------------------------------------------------------------- hidden

#[test]
fn dot_file_is_hidden_and_dot_dir_not_descended() {
    let (_tmp, root) = tmp_root();
    write(&root.join(".secret.pdf"));
    write(&root.join(".cache").join("a.pdf"));
    write(&root.join("keep.pdf"));
    let p = run(std::slice::from_ref(&root), &[move_all("Out")]);
    assert_eq!(sources(&p), vec![root.join("keep.pdf")]);
    assert_eq!(
        skips(&p),
        vec![
            (root.join(".cache"), SkipReason::Hidden, None),
            (
                root.join(".secret.pdf"),
                SkipReason::Hidden,
                Some("r".into())
            ),
        ]
    );
}

#[test]
fn dot_named_root_is_scanned() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join(".stuff");
    write(&root.join("a.pdf"));
    let p = run(std::slice::from_ref(&root), &[move_all("Out")]);
    assert_eq!(sources(&p), vec![root.join("a.pdf")]);
}

#[test]
fn hidden_file_without_matching_rule_is_reported_without_rule() {
    let (_tmp, root) = tmp_root();
    write(&root.join(".x"));
    let p = run(std::slice::from_ref(&root), &[]);
    assert_eq!(skips(&p), vec![(root.join(".x"), SkipReason::Hidden, None)]);
}

#[test]
fn platform_hidden_file_and_dir_are_skipped() {
    let (_tmp, root) = tmp_root();
    let f = root.join("desktop.ini");
    let d = root.join("sys");
    write(&f);
    write(&d.join("a.pdf"));
    let mut platform = FakePlatform::default();
    platform.hidden.insert(f.clone());
    platform.hidden.insert(d.clone());
    let p = plan(
        std::slice::from_ref(&root),
        &[move_all("Out")],
        &platform,
        &ctx(),
    );
    assert!(p.ops.is_empty());
    assert_eq!(
        skips(&p),
        vec![
            (f, SkipReason::Hidden, Some("r".into())),
            (d, SkipReason::Hidden, None),
        ]
    );
}

#[test]
fn include_hidden_rule_plans_hidden_files() {
    let (_tmp, root) = tmp_root();
    let dot = root.join(".env.pdf");
    let attr = root.join("attr.pdf");
    write(&dot);
    write(&attr);
    let mut platform = FakePlatform::default();
    platform.hidden.insert(attr.clone());
    let r = rule_json(
        1,
        "hid",
        json!([]),
        json!([{ "type": "moveTo", "params": { "path": "Out" } }]),
        json!({ "includeHidden": true }),
    );
    let p = plan(std::slice::from_ref(&root), &[r], &platform, &ctx());
    assert_eq!(sources(&p), vec![dot, attr]);
    assert!(p.skipped.is_empty());
}

#[test]
fn include_hidden_is_decided_by_the_winning_rule() {
    let (_tmp, root) = tmp_root();
    let dot = root.join(".a.pdf");
    write(&dot);
    let tag_first = rule_json(
        1,
        "tag",
        json!([]),
        json!([{ "type": "tag", "params": { "tags": ["t"] } }]),
        json!({}),
    );
    let mover = rule_json(
        2,
        "mover",
        json!([]),
        json!([{ "type": "moveTo", "params": { "path": "Out" } }]),
        json!({ "includeHidden": false }),
    );
    let p = run(std::slice::from_ref(&root), &[tag_first.clone(), mover]);
    assert_eq!(
        skips(&p),
        vec![(dot.clone(), SkipReason::Hidden, Some("mover".into()))]
    );

    let mover_opt_in = rule_json(
        2,
        "mover",
        json!([]),
        json!([{ "type": "moveTo", "params": { "path": "Out" } }]),
        json!({ "includeHidden": true }),
    );
    let p = run(std::slice::from_ref(&root), &[tag_first, mover_opt_in]);
    assert_eq!(sources(&p), vec![dot]);
}

// ---------------------------------------------------------------- reparse points

#[cfg(unix)]
#[test]
fn symlinked_file_and_dir_are_reparse_points() {
    use std::os::unix::fs::symlink;
    let (tmp, root) = tmp_root();
    let outside = tmp.path().join("outside");
    write(&outside.join("real.pdf"));
    write(&outside.join("dir").join("inner.pdf"));
    symlink(outside.join("real.pdf"), root.join("link.pdf")).unwrap();
    symlink(outside.join("dir"), root.join("linkdir")).unwrap();
    let p = run(std::slice::from_ref(&root), &[move_all("Out")]);
    assert!(p.ops.is_empty());
    assert_eq!(
        skips(&p),
        vec![
            (root.join("link.pdf"), SkipReason::ReparsePoint, None),
            (root.join("linkdir"), SkipReason::ReparsePoint, None),
        ]
    );
}

/// Platform that reports one path as a reparse point (junction on Windows).
struct ReparseAt(PathBuf, FakePlatform);
impl Platform for ReparseAt {
    fn trash(&self, p: &Path) -> std::io::Result<()> {
        self.1.trash(p)
    }
    fn can_trash(&self, p: &Path) -> bool {
        self.1.can_trash(p)
    }
    fn is_cloud_placeholder(&self, p: &Path) -> bool {
        self.1.is_cloud_placeholder(p)
    }
    fn is_locked(&self, p: &Path) -> bool {
        self.1.is_locked(p)
    }
    fn is_hidden_or_system(&self, p: &Path) -> bool {
        self.1.is_hidden_or_system(p)
    }
    fn is_reparse_point(&self, p: &Path) -> bool {
        p == self.0
    }
    fn same_volume(&self, a: &Path, b: &Path) -> std::io::Result<bool> {
        self.1.same_volume(a, b)
    }
    fn rename_no_replace(&self, a: &Path, b: &Path) -> std::io::Result<()> {
        self.1.rename_no_replace(a, b)
    }
    fn os_path(&self, p: &Path) -> PathBuf {
        self.1.os_path(p)
    }
}

#[test]
fn platform_reparse_dir_is_not_descended() {
    let (_tmp, root) = tmp_root();
    let junction = root.join("junction");
    write(&junction.join("a.pdf"));
    let platform = ReparseAt(junction.clone(), FakePlatform::default());
    let p = plan(
        std::slice::from_ref(&root),
        &[move_all("Out")],
        &platform,
        &ctx(),
    );
    assert!(p.ops.is_empty());
    assert_eq!(skips(&p), vec![(junction, SkipReason::ReparsePoint, None)]);
}

// ---------------------------------------------------------------- being written

#[test]
fn each_being_written_case_is_skipped() {
    let (_tmp, root) = tmp_root();
    let names = [
        "a.crdownload",
        "b.part",
        "c.tmp",
        "d.download",
        "e.partial",
        "F.TMP",
        "~$report.docx",
    ];
    for n in names {
        write(&root.join(n));
    }
    let p = run(std::slice::from_ref(&root), &[move_all("Out")]);
    assert!(p.ops.is_empty());
    assert_eq!(p.skipped.len(), names.len());
    assert!(
        p.skipped
            .iter()
            .all(|s| s.reason == SkipReason::BeingWritten)
    );
}

#[test]
fn recently_modified_file_is_being_written() {
    let (_tmp, root) = tmp_root();
    let recent = root.join("recent.pdf");
    let older = root.join("older.pdf");
    write(&recent);
    write(&older);
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    set_mtime(&recent, now - Duration::from_secs(5));
    set_mtime(&older, now - Duration::from_secs(11));
    let mut c = ctx();
    c.now = now;
    let p = plan(
        std::slice::from_ref(&root),
        &[move_all("Out")],
        &FakePlatform::default(),
        &c,
    );
    assert_eq!(sources(&p), vec![older]);
    assert_eq!(skips(&p), vec![(recent, SkipReason::BeingWritten, None)]);
}

// ---------------------------------------------------------------- placeholders

#[test]
fn cloud_placeholder_file_and_dir_are_skipped() {
    let (_tmp, root) = tmp_root();
    let f = root.join("cloud.pdf");
    let d = root.join("clouddir");
    write(&f);
    write(&d.join("inner.pdf"));
    let mut platform = FakePlatform::default();
    platform.placeholders.insert(f.clone());
    platform.placeholders.insert(d.clone());
    let p = plan(
        std::slice::from_ref(&root),
        &[move_all("Out")],
        &platform,
        &ctx(),
    );
    assert!(p.ops.is_empty());
    assert_eq!(
        skips(&p),
        vec![
            (f, SkipReason::CloudPlaceholder, None),
            (d, SkipReason::CloudPlaceholder, None),
        ]
    );
}

#[test]
fn placeholder_wins_over_being_written_and_hidden() {
    let (_tmp, root) = tmp_root();
    let f = root.join(".x.tmp");
    write(&f);
    let mut platform = FakePlatform::default();
    platform.placeholders.insert(f.clone());
    let p = plan(std::slice::from_ref(&root), &[], &platform, &ctx());
    assert_eq!(reasons_for(&p, &f), vec![SkipReason::CloudPlaceholder]);
}

// ---------------------------------------------------------------- rule selection

#[test]
fn first_transfer_rule_wins_and_later_ones_conflict() {
    let (_tmp, root) = tmp_root();
    let f = root.join("a.pdf");
    write(&f);
    let rules = [
        all(1, "first", "moveTo", "A"),
        all(2, "second", "copyTo", "B"),
        all(3, "third", "moveTo", "C"),
    ];
    let p = run(std::slice::from_ref(&root), &rules);
    assert_eq!(p.ops.len(), 1);
    assert_eq!(p.ops[0].rule_name, "first");
    assert_eq!(p.ops[0].rule_id, Uuid::from_u128(1));
    assert_eq!(p.ops[0].dest, root.join("A").join("a.pdf"));
    assert_eq!(
        skips(&p),
        vec![
            (f.clone(), SkipReason::Conflict, Some("second".into())),
            (f, SkipReason::Conflict, Some("third".into())),
        ]
    );
}

#[test]
fn first_transfer_action_of_winning_rule_is_used() {
    let (_tmp, root) = tmp_root();
    write(&root.join("a.pdf"));
    let r = rule_json(
        1,
        "r",
        json!([]),
        json!([
            { "type": "tag", "params": { "tags": ["t"] } },
            { "type": "copyTo", "params": { "path": "First" } },
            { "type": "moveTo", "params": { "path": "Second" } }
        ]),
        json!({}),
    );
    let p = run(std::slice::from_ref(&root), &[r]);
    assert_eq!(p.ops.len(), 1);
    assert_eq!(p.ops[0].kind, OpKind::Copy);
    assert_eq!(p.ops[0].dest, root.join("First").join("a.pdf"));
    assert!(
        p.skipped.is_empty(),
        "tag is not reported when an op is planned"
    );
}

#[test]
fn disabled_rule_does_not_win() {
    let (_tmp, root) = tmp_root();
    write(&root.join("a.pdf"));
    let mut off = all(1, "off", "moveTo", "A");
    off.enabled = false;
    let p = run(
        std::slice::from_ref(&root),
        &[off, all(2, "on", "moveTo", "B")],
    );
    assert_eq!(p.ops[0].rule_name, "on");
    assert!(p.skipped.is_empty());
}

#[test]
fn tag_only_rule_before_mover_is_not_reported() {
    let (_tmp, root) = tmp_root();
    write(&root.join("a.pdf"));
    let tag = rule_json(
        1,
        "tag",
        json!([]),
        json!([{ "type": "tag", "params": { "tags": ["t"] } }]),
        json!({}),
    );
    let p = run(
        std::slice::from_ref(&root),
        &[tag, all(2, "mv", "moveTo", "B")],
    );
    assert_eq!(p.ops.len(), 1);
    assert!(p.skipped.is_empty());
}

#[test]
fn unsupported_actions_reported_once_per_rule_when_nothing_planned() {
    let (_tmp, root) = tmp_root();
    let f = root.join("a.pdf");
    write(&f);
    let r1 = rule_json(
        1,
        "multi",
        json!([]),
        json!([
            { "type": "tag", "params": { "tags": ["t"] } },
            { "type": "rename", "params": { "pattern": "x" } },
            { "type": "quarantine" }
        ]),
        json!({}),
    );
    let r2 = rule_json(
        2,
        "unknown",
        json!([]),
        json!([{ "type": "delete" }]),
        json!({}),
    );
    let p = run(std::slice::from_ref(&root), &[r1, r2]);
    assert!(p.ops.is_empty());
    assert_eq!(
        skips(&p),
        vec![
            (
                f.clone(),
                SkipReason::UnsupportedAction,
                Some("multi".into())
            ),
            (f, SkipReason::UnsupportedAction, Some("unknown".into())),
        ]
    );
}

#[test]
fn unsupported_actions_reported_when_winner_destination_fails() {
    let (_tmp, root) = tmp_root();
    let f = root.join("a.pdf");
    write(&f);
    let r = rule_json(
        1,
        "r",
        json!([]),
        json!([
            { "type": "moveTo", "params": { "path": "~/Docs" } },
            { "type": "tag", "params": { "tags": ["t"] } }
        ]),
        json!({}),
    );
    let p = run(std::slice::from_ref(&root), &[r]);
    assert_eq!(
        skips(&p),
        vec![
            (f.clone(), SkipReason::InvalidDestination, Some("r".into())),
            (f, SkipReason::UnsupportedAction, Some("r".into())),
        ]
    );
}

#[test]
fn unmatched_file_is_neither_planned_nor_reported() {
    let (_tmp, root) = tmp_root();
    write(&root.join("a.txt"));
    let r = rule_json(
        1,
        "pdf",
        json!([{ "type": "ext", "value": "pdf" }]),
        json!([{ "type": "moveTo", "params": { "path": "A" } }]),
        json!({}),
    );
    let p = run(std::slice::from_ref(&root), &[r]);
    assert_eq!(p, Plan::default());
}

// ---------------------------------------------------------------- destinations

#[test]
fn file_already_in_destination_folder_is_already_in_place() {
    // One folder per root: on a case-insensitive file system (NTFS)
    // `Imagini` and `imagini` siblings would be the same folder and the scan
    // would report both files under the first spelling.
    for (on_disk, template) in [
        ("Imagini", "Imagini"),
        ("Imagini", "imagini"),
        ("imagini", "IMAGINI"),
        ("Imagini", "./Imagini/sub/.."),
    ] {
        let (_tmp, root) = tmp_root();
        let f = root.join(on_disk).join("a.jpg");
        write(&f);
        let p = run(std::slice::from_ref(&root), &[move_all(template)]);
        assert!(p.ops.is_empty(), "{on_disk} vs {template}: {p:?}");
        assert_eq!(
            skips(&p),
            vec![(f, SkipReason::AlreadyInPlace, Some("r".into()))],
            "folder comparison ignores case: {on_disk} vs {template}"
        );
    }
}

#[test]
fn ext_token_without_extension_is_no_extension() {
    let (_tmp, root) = tmp_root();
    let f = root.join("README");
    write(&f);
    let p = run(std::slice::from_ref(&root), &[move_all("By/{ext}")]);
    assert_eq!(
        skips(&p),
        vec![(f.clone(), SkipReason::NoExtension, Some("r".into()))]
    );

    let p = run(std::slice::from_ref(&root), &[move_all("Plain")]);
    assert_eq!(p.ops[0].dest, root.join("Plain").join("README"));
}

#[test]
fn tilde_without_home_is_invalid_destination() {
    let (_tmp, root) = tmp_root();
    let f = root.join("a.pdf");
    write(&f);
    let p = run(std::slice::from_ref(&root), &[move_all("~/Documents")]);
    assert_eq!(
        skips(&p),
        vec![(f, SkipReason::InvalidDestination, Some("r".into()))]
    );
}

#[test]
fn tilde_expands_to_home() {
    let (tmp, root) = tmp_root();
    write(&root.join("a.pdf"));
    let home = tmp.path().join("home");
    let c = ctx_with(Some(home.clone()), HashMap::new());
    for (tpl, folder) in [
        (
            "~/Documents/Finance",
            home.join("Documents").join("Finance"),
        ),
        ("~\\Docs", home.join("Docs")),
        ("~", home.clone()),
    ] {
        let p = plan(
            std::slice::from_ref(&root),
            &[move_all(tpl)],
            &FakePlatform::default(),
            &c,
        );
        assert_eq!(p.ops[0].dest, folder.join("a.pdf"), "{tpl}");
    }
    // `~name` is not a home reference: relative folder under the root.
    let p = plan(
        std::slice::from_ref(&root),
        &[move_all("~old")],
        &FakePlatform::default(),
        &c,
    );
    assert_eq!(p.ops[0].dest, root.join("~old").join("a.pdf"));
}

#[test]
fn missing_env_var_is_invalid_destination_and_set_var_expands() {
    let (tmp, root) = tmp_root();
    let f = root.join("a.pdf");
    write(&f);
    let p = run(std::slice::from_ref(&root), &[move_all("%NOPE%/x")]);
    assert_eq!(
        skips(&p),
        vec![(f, SkipReason::InvalidDestination, Some("r".into()))]
    );

    let pics = tmp.path().join("pics");
    let env = HashMap::from([("PICS".to_string(), pics.to_string_lossy().into_owned())]);
    let c = ctx_with(None, env);
    let p = plan(
        std::slice::from_ref(&root),
        &[move_all("%PICS%/50%off")],
        &FakePlatform::default(),
        &c,
    );
    assert_eq!(p.ops[0].dest, pics.join("50%off").join("a.pdf"));
}

#[test]
fn empty_or_missing_path_param_is_invalid_destination() {
    let (_tmp, root) = tmp_root();
    let f = root.join("a.pdf");
    write(&f);
    for params in [json!({}), json!({ "path": 5 }), json!({ "path": "  " })] {
        let r = rule_json(
            1,
            "r",
            json!([]),
            json!([{ "type": "moveTo", "params": params }]),
            json!({}),
        );
        let p = run(std::slice::from_ref(&root), &[r]);
        assert_eq!(
            skips(&p),
            vec![(f.clone(), SkipReason::InvalidDestination, Some("r".into()))]
        );
    }
}

#[test]
fn year_month_ext_expand_from_mtime() {
    let (_tmp, root) = tmp_root();
    let f = root.join("Scan.PDF");
    write(&f);
    // 2023-06-15 12:00 UTC: same year and month in every time zone.
    set_mtime(
        &f,
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_686_830_400),
    );
    let p = run(
        std::slice::from_ref(&root),
        &[move_all("Arhiva/{year}/{month}/{ext}")],
    );
    assert_eq!(
        p.ops[0].dest,
        root.join("Arhiva")
            .join("2023")
            .join("06")
            .join("pdf")
            .join("Scan.PDF")
    );
    assert_eq!(
        p.ops[0].mtime,
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_686_830_400)
    );
    assert_eq!(p.ops[0].size, 1);
    assert_eq!(p.ops[0].kind, OpKind::Move);
}

#[test]
fn relative_folder_resolves_against_its_own_root() {
    let tmp = tempfile::tempdir().unwrap();
    let r1 = tmp.path().join("Downloads");
    let r2 = tmp.path().join("Desktop");
    write(&r1.join("sub").join("a.jpg"));
    write(&r2.join("b.jpg"));
    let p = run(&[r1.clone(), r2.clone()], &[move_all("Imagini")]);
    let dests: Vec<PathBuf> = p.ops.iter().map(|o| o.dest.clone()).collect();
    assert_eq!(
        dests,
        vec![
            r1.join("Imagini").join("a.jpg"),
            r2.join("Imagini").join("b.jpg")
        ]
    );
}

#[test]
fn relative_folder_with_dot_segments_is_normalized() {
    let (_tmp, root) = tmp_root();
    let f = root.join("Out").join("a.pdf");
    write(&f);
    let p = run(std::slice::from_ref(&root), &[move_all("./x/../Out")]);
    assert_eq!(reasons_for(&p, &f), vec![SkipReason::AlreadyInPlace]);
}

#[test]
fn file_root_resolves_relative_folder_against_its_parent() {
    let (_tmp, root) = tmp_root();
    let f = root.join("a.pdf");
    write(&f);
    let p = run(std::slice::from_ref(&f), &[move_all("Out")]);
    assert_eq!(p.ops[0].dest, root.join("Out").join("a.pdf"));
}

/// Plans `moveTo <dest>` for two files under a fresh root and asserts both are
/// skipped as `InvalidDestination` with nothing planned.
fn assert_invalid_destination(root: &Path, dest: &str) {
    let a = root.join("a.pdf");
    let b = root.join("b.pdf");
    write(&a);
    write(&b);
    let p = run(&[root.to_path_buf()], &[move_all(dest)]);
    assert!(p.ops.is_empty(), "{dest}: {:?}", p.ops);
    assert_eq!(
        skips(&p),
        vec![
            (a, SkipReason::InvalidDestination, Some("r".into())),
            (b, SkipReason::InvalidDestination, Some("r".into())),
        ],
        "{dest}"
    );
}

fn abs(p: PathBuf) -> String {
    p.to_string_lossy().into_owned()
}

#[test]
fn destination_inside_windows_dir_is_invalid() {
    let (tmp, root) = tmp_root();
    assert_invalid_destination(&root, &abs(tmp.path().join("Windows").join("x")));
    assert_invalid_destination(&root, &abs(tmp.path().join("WINDOWS").join("System32")));
}

#[test]
fn destination_inside_appdata_is_invalid() {
    let (tmp, root) = tmp_root();
    let dest = tmp
        .path()
        .join("Users")
        .join("u")
        .join("AppData")
        .join("Roaming");
    assert_invalid_destination(&root, &abs(dest));
}

#[test]
fn destination_inside_node_modules_or_git_is_invalid() {
    let (_tmp, root) = tmp_root();
    assert_invalid_destination(&root, "proj/node_modules/pkg");
    assert_invalid_destination(&root, "repo/.git");
}

#[test]
fn every_default_excluded_name_is_an_invalid_destination() {
    let (_tmp, root) = tmp_root();
    for name in DEFAULT_EXCLUDED_DIRS {
        assert_invalid_destination(&root, &format!("Sorted/{name}/x"));
    }
}

#[test]
fn destination_with_valetignore_in_folder_or_ancestor_is_invalid() {
    let (tmp, root) = tmp_root();
    let dest = tmp.path().join("dest");
    write(&dest.join(IGNORE_MARKER));
    assert_invalid_destination(&root, &abs(dest.clone()));
    // Marker in an existing ancestor; the folder itself does not exist yet.
    assert_invalid_destination(&root, &abs(dest.join("sub").join("deeper")));
}

#[test]
fn excluded_name_in_the_chosen_root_does_not_block_destinations_below_it() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("AppData").join("root");
    let f = root.join("a.pdf");
    write(&f);
    let p = run(std::slice::from_ref(&root), &[move_all("Sorted")]);
    assert_eq!(p.ops.len(), 1, "{:?}", p.skipped);
    assert_eq!(p.ops[0].dest, root.join("Sorted").join("a.pdf"));

    // A sibling of the root inside the same excluded dir is shared prefix too.
    let sibling = tmp.path().join("AppData").join("other");
    let p = run(
        std::slice::from_ref(&root),
        &[move_all(&abs(sibling.clone()))],
    );
    assert_eq!(p.ops[0].dest, sibling.join("a.pdf"));

    // But a new excluded component below the root is not.
    let p = run(std::slice::from_ref(&root), &[move_all("x/target")]);
    assert_eq!(reasons_for(&p, &f), vec![SkipReason::InvalidDestination]);
}

#[test]
fn normal_destination_outside_root_still_plans() {
    let (tmp, root) = tmp_root();
    let f = root.join("a.pdf");
    write(&f);
    // Names that only contain an excluded word are fine.
    let dest = tmp
        .path()
        .join("Documents")
        .join("Windows notes")
        .join("targets");
    write(&tmp.path().join("Documents").join("unrelated.valetignore"));
    let p = run(std::slice::from_ref(&root), &[move_all(&abs(dest.clone()))]);
    assert!(p.skipped.is_empty(), "{:?}", p.skipped);
    assert_eq!(p.ops[0].dest, dest.join("a.pdf"));
}

// ---------------------------------------------------------------- collisions

#[test]
fn existing_destination_file_gets_next_free_name() {
    let (_tmp, root) = tmp_root();
    write(&root.join("a.pdf"));
    let taken = root.join("Out").join("a.pdf");
    write(&taken);
    let p = run(
        std::slice::from_ref(&root),
        &[all(1, "r", "moveTo", &root.join("Out").to_string_lossy())],
    );
    // The file already in Out is in place; the other one gets "(2)".
    assert_eq!(p.ops.len(), 1);
    assert_eq!(p.ops[0].source, root.join("a.pdf"));
    assert_eq!(p.ops[0].dest, root.join("Out").join("a (2).pdf"));
    assert_eq!(reasons_for(&p, &taken), vec![SkipReason::AlreadyInPlace]);
}

#[test]
fn two_planned_ops_never_share_a_destination() {
    let (_tmp, root) = tmp_root();
    write(&root.join("x").join("a.pdf"));
    write(&root.join("y").join("a.pdf"));
    let p = run(std::slice::from_ref(&root), &[move_all("Out")]);
    let dests: Vec<PathBuf> = p.ops.iter().map(|o| o.dest.clone()).collect();
    assert_eq!(
        dests,
        vec![
            root.join("Out").join("a.pdf"),
            root.join("Out").join("a (2).pdf")
        ]
    );
}

#[test]
fn reservation_is_case_insensitive() {
    let (_tmp, root) = tmp_root();
    write(&root.join("x").join("A.pdf"));
    write(&root.join("y").join("a.pdf"));
    let p = run(std::slice::from_ref(&root), &[move_all("Out")]);
    let dests: Vec<PathBuf> = p.ops.iter().map(|o| o.dest.clone()).collect();
    assert_eq!(
        dests,
        vec![
            root.join("Out").join("A.pdf"),
            root.join("Out").join("a (2).pdf")
        ]
    );
}

#[test]
fn copy_reserves_destination_too() {
    let (_tmp, root) = tmp_root();
    write(&root.join("x").join("a.pdf"));
    write(&root.join("y").join("a.pdf"));
    let p = run(std::slice::from_ref(&root), &[all(1, "c", "copyTo", "Out")]);
    assert!(p.ops.iter().all(|o| o.kind == OpKind::Copy));
    assert_eq!(p.ops[1].dest, root.join("Out").join("a (2).pdf"));
}

// ---------------------------------------------------------------- roots

#[test]
fn overlapping_and_repeated_roots_plan_each_file_once() {
    let (_tmp, root) = tmp_root();
    let inner = root.join("sub");
    let f = inner.join("a.pdf");
    write(&f);
    write(&root.join("node_modules").join("z.pdf"));
    for roots in [
        vec![root.clone(), inner.clone()],
        vec![inner.clone(), root.clone()],
        vec![root.clone(), root.clone()],
        vec![root.clone(), root.join(".").join("sub")],
    ] {
        let p = run(&roots, &[move_all("/abs/out")]);
        assert_eq!(sources(&p), vec![f.clone()], "{roots:?}");
        assert_eq!(p.skipped.len(), 1, "{roots:?}: excluded dir reported once");
    }
}

#[test]
fn excluded_dir_chosen_as_root_is_scanned_in_any_root_order() {
    let (_tmp, root) = tmp_root();
    let nm = root.join("node_modules");
    let f = nm.join("a.pdf");
    write(&f);
    for roots in [
        vec![root.clone(), nm.clone()],
        vec![nm.clone(), root.clone()],
    ] {
        let p = run(&roots, &[move_all("/abs/out")]);
        assert_eq!(sources(&p), vec![f.clone()], "{roots:?}");
        assert!(p.skipped.is_empty(), "{roots:?}");
    }
}

#[test]
fn nested_root_with_valetignore_is_excluded_in_any_root_order() {
    let (_tmp, root) = tmp_root();
    let sub = root.join("sub");
    write(&sub.join(IGNORE_MARKER));
    write(&sub.join("a.pdf"));
    for roots in [
        vec![root.clone(), sub.clone()],
        vec![sub.clone(), root.clone()],
    ] {
        let p = run(&roots, &[move_all("/abs/out")]);
        assert!(p.ops.is_empty(), "{roots:?}");
        assert_eq!(
            skips(&p),
            vec![(sub.clone(), SkipReason::Excluded, None)],
            "{roots:?}"
        );
    }
}

#[test]
fn nonexistent_root_is_unreadable_and_others_still_scanned() {
    let (tmp, root) = tmp_root();
    let f = root.join("a.pdf");
    write(&f);
    let missing = tmp.path().join("missing");
    let p = run(&[missing.clone(), root], &[move_all("Out")]);
    assert_eq!(sources(&p), vec![f]);
    assert_eq!(skips(&p), vec![(missing, SkipReason::Unreadable, None)]);
}

#[test]
fn plan_order_is_sorted_by_name_and_stable() {
    let (_tmp, root) = tmp_root();
    for n in ["c.pdf", "a.pdf", "b/z.pdf", "b/y.pdf", "d.pdf"] {
        write(&root.join(n));
    }
    let rules = [move_all("/abs/out")];
    let first = run(std::slice::from_ref(&root), &rules);
    let second = run(std::slice::from_ref(&root), &rules);
    assert_eq!(first, second);
    assert_eq!(
        sources(&first),
        ["a.pdf", "b/y.pdf", "b/z.pdf", "c.pdf", "d.pdf"]
            .iter()
            .map(|n| root.join(n))
            .collect::<Vec<_>>()
    );
}

#[test]
fn skip_reason_serializes_camel_case() {
    assert_eq!(
        serde_json::to_value(SkipReason::CloudPlaceholder).unwrap(),
        json!("cloudPlaceholder")
    );
    assert_eq!(serde_json::to_value(OpKind::Move).unwrap(), json!("move"));
}

// ---------------------------------------------------------------- helpers

#[test]
fn expand_env_keeps_lone_percent() {
    let c = ctx();
    assert_eq!(expand_env("100%", &c).unwrap(), "100%");
    assert_eq!(expand_env("%%", &c).unwrap(), "%%");
    assert_eq!(expand_env("a%/b%c", &c).unwrap(), "a%/b%c");
}

#[test]
fn normalize_resolves_dot_segments() {
    assert_eq!(normalize(Path::new("a/./b/../c")), PathBuf::from("a/c"));
    assert_eq!(normalize(Path::new("../a")), PathBuf::from("../a"));
}

// ---------------------------------------------------------------- folder tokens

fn ctx_folders(folders: &[(&str, PathBuf)]) -> PlanContext {
    let mut c = ctx();
    c.folders = folders
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect();
    c
}

fn plan_with(roots: &[PathBuf], rules: &[Rule], c: &PlanContext) -> Plan {
    plan(roots, rules, &FakePlatform::default(), c)
}

#[test]
fn folder_tokens_expand_from_context() {
    let (tmp, root) = tmp_root();
    write(&root.join("a.pdf"));
    let mut pairs = Vec::new();
    for t in FOLDER_TOKENS {
        pairs.push((*t, tmp.path().join(format!("known-{t}"))));
    }
    let c = ctx_folders(&pairs);
    for (t, dir) in &pairs {
        for tpl in [
            format!("{{{t}}}"),
            format!("{{{t}}}/Sub"),
            format!("{{{t}}}\\Sub"),
        ] {
            let p = plan_with(std::slice::from_ref(&root), &[move_all(&tpl)], &c);
            let want = if tpl.len() == t.len() + 2 {
                dir.join("a.pdf")
            } else {
                dir.join("Sub").join("a.pdf")
            };
            assert_eq!(p.ops[0].dest, want, "{tpl}");
        }
    }
}

#[test]
fn missing_folder_or_unknown_token_is_invalid_destination() {
    let (tmp, root) = tmp_root();
    let f = root.join("a.pdf");
    write(&f);
    let c = ctx_folders(&[("downloads", tmp.path().join("dl"))]);
    for tpl in [
        "{desktop}/x",     // known token, folder not in the context
        "{nope}",          // unknown token
        "Out/{downloads}", // folder token after the first component
        "{downloads}x",    // not a whole component
        "Out/{YEAR}",      // token names are lowercase
        "Out/{date}",
    ] {
        let p = plan_with(std::slice::from_ref(&root), &[move_all(tpl)], &c);
        assert_eq!(
            skips(&p),
            vec![(f.clone(), SkipReason::InvalidDestination, Some("r".into()))],
            "{tpl}"
        );
    }
}

#[test]
fn home_token_falls_back_to_home_dir() {
    let (tmp, root) = tmp_root();
    write(&root.join("a.pdf"));
    let home = tmp.path().join("home");
    let c = ctx_with(Some(home.clone()), HashMap::new());
    let p = plan_with(std::slice::from_ref(&root), &[move_all("{home}/D")], &c);
    assert_eq!(p.ops[0].dest, home.join("D").join("a.pdf"));
}

#[test]
fn braces_that_are_not_tokens_stay_literal() {
    let (_tmp, root) = tmp_root();
    write(&root.join("a.pdf"));
    let p = run(
        std::slice::from_ref(&root),
        &[move_all("Out/{2024}/{}/{a b}")],
    );
    assert_eq!(
        p.ops[0].dest,
        root.join("Out")
            .join("{2024}")
            .join("{}")
            .join("{a b}")
            .join("a.pdf")
    );
}

#[test]
fn day_token_has_two_digits() {
    let (_tmp, root) = tmp_root();
    let f = root.join("a.pdf");
    write(&f);
    // 2023-06-05 12:00 UTC: the 5th in every time zone.
    set_mtime(
        &f,
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_685_966_400),
    );
    let p = run(
        std::slice::from_ref(&root),
        &[move_all("A/{year}-{month}-{day}")],
    );
    assert_eq!(
        p.ops[0].dest,
        root.join("A").join("2023-06-05").join("a.pdf")
    );
}

#[test]
fn find_token_finds_letter_names_only() {
    assert_eq!(find_token("a{year}b"), Some((1, 7)));
    assert_eq!(find_token("{1}{x_y}"), Some((3, 8)));
    assert_eq!(find_token("{}{ a}{a"), None);
}

// ---------------------------------------------------------------- scope

fn scoped(n: u128, scope: Value, dest: &str) -> Rule {
    let mut r = all(n, &format!("s{n}"), "moveTo", dest);
    r.scope = serde_json::from_value(scope).unwrap();
    r
}

#[test]
fn empty_scope_applies_under_every_root() {
    let tmp = tempfile::tempdir().unwrap();
    let (r1, r2) = (tmp.path().join("A"), tmp.path().join("B"));
    write(&r1.join("a.pdf"));
    write(&r2.join("b.pdf"));
    let p = run(&[r1.clone(), r2.clone()], &[scoped(1, json!([]), "Out")]);
    assert_eq!(sources(&p), vec![r1.join("a.pdf"), r2.join("b.pdf")]);
}

#[test]
fn scope_path_limits_rule_to_that_root_case_insensitively() {
    let tmp = tempfile::tempdir().unwrap();
    let (r1, r2) = (tmp.path().join("Downloads"), tmp.path().join("Desktop"));
    write(&r1.join("sub").join("a.pdf"));
    write(&r2.join("b.pdf"));
    let entry = tmp.path().join("DOWNLOADS").join("x").join("..");
    let rules = [
        scoped(1, json!([entry.to_string_lossy()]), "Out1"),
        scoped(2, json!([]), "Out2"),
    ];
    let p = run(&[r1.clone(), r2.clone()], &rules);
    let got: Vec<(PathBuf, String)> = p
        .ops
        .iter()
        .map(|o| (o.source.clone(), o.rule_name.clone()))
        .collect();
    assert_eq!(
        got,
        vec![
            (r1.join("sub").join("a.pdf"), "s1".to_string()),
            (r2.join("b.pdf"), "s2".to_string()),
        ]
    );
}

#[test]
fn scope_token_resolves_from_context() {
    let tmp = tempfile::tempdir().unwrap();
    let (dl, desk) = (tmp.path().join("dl"), tmp.path().join("desk"));
    write(&dl.join("a.pdf"));
    write(&desk.join("b.pdf"));
    let c = ctx_folders(&[("downloads", dl.clone()), ("desktop", desk.clone())]);
    let p = plan_with(
        &[dl.clone(), desk.clone()],
        &[scoped(1, json!(["{desktop}"]), "Out")],
        &c,
    );
    assert_eq!(sources(&p), vec![desk.join("b.pdf")]);
}

#[test]
fn unresolvable_or_relative_scope_entries_match_no_root() {
    let (_tmp, root) = tmp_root();
    write(&root.join("a.pdf"));
    for scope in [
        json!(["{pictures}"]),
        json!(["{nope}"]),
        json!(["root"]),
        json!(["%NOPE%"]),
        json!([format!("{}/{{year}}", root.display())]),
    ] {
        let p = run(
            std::slice::from_ref(&root),
            &[scoped(1, scope.clone(), "Out")],
        );
        assert!(p.ops.is_empty(), "{scope}");
        assert!(p.skipped.is_empty(), "{scope}: out of scope is not a skip");
    }
}

#[test]
fn scope_is_exact_root_not_ancestor() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("A").join("B");
    write(&root.join("a.pdf"));
    let parent = tmp.path().join("A");
    let p = run(
        std::slice::from_ref(&root),
        &[scoped(1, json!([parent.to_string_lossy()]), "Out")],
    );
    assert!(p.ops.is_empty());
}

#[test]
fn rule_json_without_scope_deserializes_to_empty_scope() {
    let r = move_all("Out");
    assert!(r.scope.is_empty());
}

// ---------------------------------------------------------------- topLevelOnly

#[test]
fn top_level_only_sees_direct_children_of_the_root() {
    let (_tmp, root) = tmp_root();
    write(&root.join("a.pdf"));
    write(&root.join("sub").join("b.pdf"));
    let top = rule_json(
        1,
        "top",
        json!([]),
        json!([{ "type": "moveTo", "params": { "path": "/abs/out" } }]),
        json!({ "topLevelOnly": true }),
    );
    let p = run(std::slice::from_ref(&root), std::slice::from_ref(&top));
    assert_eq!(sources(&p), vec![root.join("a.pdf")]);
    assert!(p.skipped.is_empty());

    // A file given as a root is "directly inside" its folder.
    let p = run(&[root.join("sub").join("b.pdf")], &[top]);
    assert_eq!(sources(&p), vec![root.join("sub").join("b.pdf")]);
}

#[test]
fn top_level_only_false_or_malformed_sees_everything() {
    let (_tmp, root) = tmp_root();
    write(&root.join("a.pdf"));
    write(&root.join("sub").join("b.pdf"));
    for opt in [
        json!({ "topLevelOnly": false }),
        json!({ "topLevelOnly": "yes" }),
    ] {
        let r = rule_json(
            1,
            "r",
            json!([]),
            json!([{ "type": "moveTo", "params": { "path": "/abs/out" } }]),
            opt,
        );
        let p = run(std::slice::from_ref(&root), &[r]);
        assert_eq!(p.ops.len(), 2);
    }
}

// ---------------------------------------------------------------- trash and report

fn acts(n: u128, name: &str, conditions: Value, actions: Value) -> Rule {
    rule_json(n, name, conditions, actions, json!({}))
}

fn reports(p: &Plan) -> Vec<(PathBuf, String, Option<String>)> {
    p.reports
        .iter()
        .map(|r| (r.path.clone(), r.rule_name.clone(), r.group.clone()))
        .collect()
}

#[test]
fn trash_rule_plans_trash_op_without_destination() {
    let (_tmp, root) = tmp_root();
    let f = root.join("setup.exe");
    write(&f);
    write(&root.join("keep.pdf"));
    let r = acts(
        1,
        "old",
        json!([{ "type": "extIn", "value": ["exe"] }]),
        json!([{ "type": "trash" }]),
    );
    let p = run(std::slice::from_ref(&root), &[r]);
    assert_eq!(p.ops.len(), 1);
    let op = &p.ops[0];
    assert_eq!(
        (&op.source, &op.dest, op.kind),
        (&f, &PathBuf::new(), OpKind::Trash)
    );
    assert_eq!(op.rule_name, "old");
    assert!(p.skipped.is_empty() && p.reports.is_empty());
}

#[test]
fn trash_rule_still_applies_every_skip() {
    let (_tmp, root) = tmp_root();
    let hidden = root.join(".h.exe");
    let writing = root.join("a.exe.crdownload");
    let cloud = root.join("cloud.exe");
    let excluded = root.join("node_modules");
    write(&hidden);
    write(&writing);
    write(&cloud);
    write(&excluded.join("x.exe"));
    let mut platform = FakePlatform::default();
    platform.placeholders.insert(cloud.clone());
    let r = acts(1, "t", json!([]), json!([{ "type": "trash" }]));
    let p = plan(std::slice::from_ref(&root), &[r], &platform, &ctx());
    assert!(p.ops.is_empty(), "{:?}", p.ops);
    let mut got = skips(&p);
    got.sort_by(|a, b| a.0.cmp(&b.0));
    let mut want = vec![
        (hidden, SkipReason::Hidden, Some("t".into())),
        (writing, SkipReason::BeingWritten, None),
        (cloud, SkipReason::CloudPlaceholder, None),
        (excluded, SkipReason::Excluded, None),
    ];
    want.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(got, want);
}

#[test]
fn trash_and_transfer_rules_conflict_in_rule_order() {
    let (_tmp, root) = tmp_root();
    let f = root.join("a.pdf");
    write(&f);
    let trash = acts(1, "trash", json!([]), json!([{ "type": "trash" }]));
    let mv = all(2, "move", "moveTo", "Out");
    let p = run(std::slice::from_ref(&root), &[trash.clone(), mv.clone()]);
    assert_eq!(p.ops[0].kind, OpKind::Trash);
    assert_eq!(
        skips(&p),
        vec![(f.clone(), SkipReason::Conflict, Some("move".into()))]
    );

    let p = run(std::slice::from_ref(&root), &[mv, trash]);
    assert_eq!(p.ops[0].kind, OpKind::Move);
    assert_eq!(
        skips(&p),
        vec![(f, SkipReason::Conflict, Some("trash".into()))]
    );
}

#[test]
fn report_rule_adds_entry_and_no_operation() {
    let (_tmp, root) = tmp_root();
    let big = root.join("big.iso");
    fs::create_dir_all(&root).unwrap();
    fs::write(&big, vec![0u8; 2048]).unwrap();
    write(&root.join("small.txt"));
    let r = acts(
        7,
        "Mari",
        json!([{ "type": "sizeGt", "value": 1024 }]),
        json!([{ "type": "report" }]),
    );
    let p = run(std::slice::from_ref(&root), &[r]);
    assert!(p.ops.is_empty() && p.skipped.is_empty());
    assert_eq!(
        p.reports,
        vec![ReportEntry {
            path: big,
            rule_id: Uuid::from_u128(7),
            rule_name: "Mari".into(),
            size: 2048,
            group: None,
        }]
    );
}

#[test]
fn reports_do_not_count_toward_one_op_per_file() {
    let (_tmp, root) = tmp_root();
    let f = root.join("a.pdf");
    write(&f);
    let rep1 = acts(1, "rep1", json!([]), json!([{ "type": "report" }]));
    let mv = all(2, "move", "moveTo", "Out");
    let rep2 = acts(3, "rep2", json!([]), json!([{ "type": "report" }]));
    let p = run(std::slice::from_ref(&root), &[rep1, mv, rep2]);
    assert_eq!(sources(&p), vec![f.clone()]);
    assert!(p.skipped.is_empty(), "no Conflict for report rules");
    assert_eq!(
        reports(&p),
        vec![(f.clone(), "rep1".into(), None), (f, "rep2".into(), None)]
    );
}

#[test]
fn report_and_trash_in_one_rule_give_both() {
    let (_tmp, root) = tmp_root();
    let f = root.join("a.exe");
    write(&f);
    let r = acts(
        1,
        "r",
        json!([]),
        json!([{ "type": "report" }, { "type": "trash" }]),
    );
    let p = run(std::slice::from_ref(&root), &[r]);
    assert_eq!(p.ops[0].kind, OpKind::Trash);
    assert_eq!(reports(&p), vec![(f, "r".into(), None)]);
}

#[test]
fn hidden_file_is_reported_only_with_include_hidden() {
    let (_tmp, root) = tmp_root();
    let f = root.join(".h.txt");
    write(&f);
    let plain = acts(1, "plain", json!([]), json!([{ "type": "report" }]));
    let p = run(std::slice::from_ref(&root), std::slice::from_ref(&plain));
    assert!(p.reports.is_empty());
    assert_eq!(
        skips(&p),
        vec![(f.clone(), SkipReason::Hidden, Some("plain".into()))]
    );

    let opted = rule_json(
        2,
        "opted",
        json!([]),
        json!([{ "type": "report" }]),
        json!({ "includeHidden": true }),
    );
    let p = run(std::slice::from_ref(&root), &[opted, plain]);
    assert_eq!(reports(&p), vec![(f, "opted".into(), None)]);
}

#[test]
fn tag_action_is_still_unsupported() {
    let (_tmp, root) = tmp_root();
    let f = root.join("a.pdf");
    write(&f);
    let r = acts(1, "t", json!([]), json!([{ "type": "tag", "params": {} }]));
    let p = run(std::slice::from_ref(&root), &[r]);
    assert_eq!(
        skips(&p),
        vec![(f, SkipReason::UnsupportedAction, Some("t".into()))]
    );
}

#[test]
fn plan_json_has_reports_and_trash_kind() {
    let v = serde_json::to_value(Plan::default()).unwrap();
    assert_eq!(v["reports"], json!([]));
    assert_eq!(serde_json::to_value(OpKind::Trash).unwrap(), json!("trash"));
    // Plans serialized before `reports` existed still load.
    let old: Plan = serde_json::from_value(json!({ "ops": [], "skipped": [] })).unwrap();
    assert!(old.reports.is_empty());
}

// ---------------------------------------------------------------- emptyFolder

fn empty_folder_rule(extra: Value, actions: Value) -> Rule {
    let mut conds = vec![json!({ "type": "emptyFolder", "value": true })];
    if let Value::Array(more) = extra {
        conds.extend(more);
    }
    acts(1, "goale", Value::Array(conds), actions)
}

fn trash_empty() -> Rule {
    empty_folder_rule(json!([]), json!([{ "type": "trash" }]))
}

#[test]
fn empty_folder_rule_trashes_empty_directories_only() {
    let (_tmp, root) = tmp_root();
    let empty = root.join("empty");
    let nested = root.join("full").join("inner");
    fs::create_dir_all(&empty).unwrap();
    fs::create_dir_all(&nested).unwrap();
    write(&root.join("full").join("f.txt"));
    write(&root.join("file.txt"));
    let p = run(std::slice::from_ref(&root), &[trash_empty()]);
    let got: Vec<(PathBuf, OpKind, PathBuf)> = p
        .ops
        .iter()
        .map(|o| (o.source.clone(), o.kind, o.dest.clone()))
        .collect();
    assert_eq!(
        got,
        vec![
            (empty, OpKind::Trash, PathBuf::new()),
            (nested, OpKind::Trash, PathBuf::new()),
        ]
    );
    assert_eq!(p.ops[0].size, 0);
    assert!(p.skipped.is_empty());
}

#[test]
fn empty_root_and_directory_chosen_as_root_are_never_candidates() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("root");
    let inner = root.join("inner");
    fs::create_dir_all(&inner).unwrap();
    let p = run(&[root.clone(), inner.clone()], &[trash_empty()]);
    assert!(p.ops.is_empty(), "{:?}", p.ops);
    let p = run(std::slice::from_ref(&inner), &[trash_empty()]);
    assert!(p.ops.is_empty());
}

#[test]
fn excluded_and_hidden_empty_directories_are_skipped_not_trashed() {
    let (_tmp, root) = tmp_root();
    fs::create_dir_all(root.join("node_modules")).unwrap();
    fs::create_dir_all(root.join(".cache")).unwrap();
    let p = run(std::slice::from_ref(&root), &[trash_empty()]);
    assert!(p.ops.is_empty());
    assert_eq!(
        skips(&p),
        vec![
            (root.join(".cache"), SkipReason::Hidden, None),
            (root.join("node_modules"), SkipReason::Excluded, None),
        ]
    );
}

#[test]
fn recently_changed_empty_directory_is_being_written() {
    let (_tmp, root) = tmp_root();
    let d = root.join("new");
    fs::create_dir_all(&d).unwrap();
    let mut c = ctx();
    c.now = SystemTime::now();
    let p = plan_with(std::slice::from_ref(&root), &[trash_empty()], &c);
    assert!(p.ops.is_empty());
    assert_eq!(skips(&p), vec![(d, SkipReason::BeingWritten, None)]);
}

#[test]
fn directories_are_not_scanned_without_folder_rules() {
    let (_tmp, root) = tmp_root();
    fs::create_dir_all(root.join("new")).unwrap();
    let mut c = ctx();
    c.now = SystemTime::now();
    let p = plan_with(
        std::slice::from_ref(&root),
        &[acts(1, "t", json!([]), json!([{ "type": "trash" }]))],
        &c,
    );
    assert!(p.ops.is_empty() && p.skipped.is_empty());
}

#[test]
fn empty_folder_with_age_uses_directory_mtime() {
    let (_tmp, root) = tmp_root();
    let old = root.join("old");
    let young = root.join("young");
    fs::create_dir_all(&old).unwrap();
    fs::create_dir_all(&young).unwrap();
    let ten_days = Duration::from_secs(10 * 86_400);
    fs::File::open(&old)
        .unwrap()
        .set_modified(SystemTime::now() - ten_days)
        .unwrap();
    let r = empty_folder_rule(
        json!([{ "type": "modifiedOlderThanDays", "value": 7 }]),
        json!([{ "type": "trash" }]),
    );
    let p = run(std::slice::from_ref(&root), &[r]);
    assert_eq!(sources(&p), vec![old]);
}

#[test]
fn empty_folder_with_move_is_unsupported_and_report_works() {
    let (_tmp, root) = tmp_root();
    let d = root.join("e");
    fs::create_dir_all(&d).unwrap();
    let mv = empty_folder_rule(
        json!([]),
        json!([{ "type": "moveTo", "params": { "path": "Out" } }]),
    );
    let p = run(std::slice::from_ref(&root), &[mv]);
    assert!(p.ops.is_empty());
    assert_eq!(
        skips(&p),
        vec![(
            d.clone(),
            SkipReason::UnsupportedAction,
            Some("goale".into())
        )]
    );

    let rep = empty_folder_rule(json!([]), json!([{ "type": "report" }]));
    let p = run(std::slice::from_ref(&root), &[rep]);
    assert_eq!(reports(&p), vec![(d, "goale".into(), None)]);
}

#[test]
fn file_rules_never_see_directories() {
    let (_tmp, root) = tmp_root();
    fs::create_dir_all(root.join("e")).unwrap();
    let f = root.join("a.txt");
    write(&f);
    let p = run(
        std::slice::from_ref(&root),
        &[trash_empty(), all(2, "move", "moveTo", "/abs/out")],
    );
    assert_eq!(sources(&p), vec![f, root.join("e")]);
    assert_eq!(p.ops[0].kind, OpKind::Move);
    assert_eq!(p.ops[1].kind, OpKind::Trash);
}

// ---------------------------------------------------------------- duplicate

fn put(path: &Path, body: &[u8], mtime_secs: u64) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, body).unwrap();
    set_mtime(
        path,
        SystemTime::UNIX_EPOCH + Duration::from_secs(mtime_secs),
    );
}

fn dup_rule(n: u128, extra: Value, actions: Value) -> Rule {
    let mut conds = vec![json!({ "type": "duplicate", "value": true })];
    if let Value::Array(more) = extra {
        conds.extend(more);
    }
    acts(n, &format!("dup{n}"), Value::Array(conds), actions)
}

fn group_id(body: &[u8]) -> String {
    blake3::hash(body).to_hex()[..12].to_string()
}

fn hashed() -> Vec<PathBuf> {
    let mut v = HASHED.with(|h| h.borrow().clone());
    v.sort();
    v
}

fn reset_hashed() {
    HASHED.with(|h| h.borrow_mut().clear());
}

#[test]
fn duplicate_report_groups_identical_files_and_hashes_same_size_only() {
    let (_tmp, root) = tmp_root();
    let (a, b, c) = (root.join("a.bin"), root.join("s/b.bin"), root.join("c.bin"));
    let d = root.join("d.bin"); // same size, other bytes
    let e = root.join("e.bin"); // unique size
    for f in [&a, &b, &c] {
        put(f, b"same", 1000);
    }
    put(&d, b"diff", 1000);
    put(&e, b"longer content", 1000);
    reset_hashed();
    let p = run(
        std::slice::from_ref(&root),
        &[dup_rule(1, json!([]), json!([{ "type": "report" }]))],
    );
    let g = Some(group_id(b"same"));
    assert_eq!(
        reports(&p),
        vec![
            (a.clone(), "dup1".into(), g.clone()),
            (c.clone(), "dup1".into(), g.clone()),
            (b.clone(), "dup1".into(), g),
        ]
    );
    assert!(p.ops.is_empty() && p.skipped.is_empty());
    let mut want = vec![a, b, c, d];
    want.sort();
    assert_eq!(hashed(), want, "e has a unique size and is never hashed");
}

#[test]
fn duplicate_trash_keeps_oldest_and_trashes_the_rest() {
    let (_tmp, root) = tmp_root();
    let (old, mid, new) = (
        root.join("z_old.bin"),
        root.join("a_mid.bin"),
        root.join("m_new.bin"),
    );
    put(&old, b"same", 1000);
    put(&mid, b"same", 2000);
    put(&new, b"same", 3000);
    let p = run(
        std::slice::from_ref(&root),
        &[dup_rule(1, json!([]), json!([{ "type": "trash" }]))],
    );
    let ops: Vec<(PathBuf, OpKind)> = p.ops.iter().map(|o| (o.source.clone(), o.kind)).collect();
    assert_eq!(ops, vec![(mid, OpKind::Trash), (new, OpKind::Trash)]);
    assert_eq!(
        reports(&p),
        vec![(old, "dup1".into(), Some(group_id(b"same")))]
    );
}

#[test]
fn duplicate_keeper_ties_break_on_path_length_then_lexicographic() {
    let (_tmp, root) = tmp_root();
    let long = root.join("aaaa.bin");
    let b = root.join("b.bin");
    let a = root.join("a.bin");
    let c = root.join("c.bin");
    for f in [&long, &b, &a, &c] {
        put(f, b"same", 1000);
    }
    let p = run(
        std::slice::from_ref(&root),
        &[dup_rule(1, json!([]), json!([{ "type": "trash" }]))],
    );
    // Same mtime; a.bin, b.bin and c.bin are shortest; a.bin sorts first.
    assert_eq!(reports(&p)[0].0, a);
    assert_eq!(
        sources(&p),
        vec![long, b, c],
        "every copy except the keeper"
    );
}

#[test]
fn keeper_key_orders_mtime_then_length_then_path() {
    let f = |p: &str, t: u64| FoundFile {
        path: PathBuf::from(p),
        root_idx: 0,
        depth: 1,
        base: PathBuf::new(),
        size: 1,
        mtime: SystemTime::UNIX_EPOCH + Duration::from_secs(t),
        created: None,
        hidden: false,
        is_dir: false,
    };
    assert!(keeper_key(&f("/long/path", 1)) < keeper_key(&f("/a", 2)));
    assert!(keeper_key(&f("/b", 1)) < keeper_key(&f("/aa", 1)));
    assert!(keeper_key(&f("/a", 1)) < keeper_key(&f("/b", 1)));
}

#[test]
fn placeholders_hidden_excluded_and_writing_files_are_never_hashed() {
    let (_tmp, root) = tmp_root();
    let real = root.join("a.bin");
    let cloud = root.join("cloud.bin");
    let attr_hidden = root.join("h.bin");
    let dot = root.join(".dot.bin");
    let writing = root.join("w.bin.part");
    let excluded = root.join("node_modules").join("x.bin");
    for f in [&real, &cloud, &attr_hidden, &dot, &writing, &excluded] {
        put(f, b"same", 1000);
    }
    let mut platform = FakePlatform::default();
    platform.placeholders.insert(cloud.clone());
    platform.hidden.insert(attr_hidden.clone());
    let opted = rule_json(
        1,
        "dup",
        json!([{ "type": "duplicate", "value": true }]),
        json!([{ "type": "trash" }]),
        json!({ "includeHidden": true }),
    );
    reset_hashed();
    let p = plan(std::slice::from_ref(&root), &[opted], &platform, &ctx());
    assert!(hashed().is_empty(), "hashed {:?}", hashed());
    assert!(p.ops.is_empty() && p.reports.is_empty());
    assert!(
        p.skipped
            .iter()
            .any(|s| s.path == cloud && s.reason == SkipReason::CloudPlaceholder)
    );
}

#[test]
fn duplicate_candidates_follow_the_other_conditions_and_scope() {
    let tmp = tempfile::tempdir().unwrap();
    let (r1, r2) = (tmp.path().join("A"), tmp.path().join("B"));
    put(&r1.join("a.pdf"), b"same", 1000);
    put(&r1.join("b.txt"), b"same", 1000);
    put(&r2.join("c.pdf"), b"same", 1000);
    let mut r = dup_rule(
        1,
        json!([{ "type": "extIn", "value": ["pdf"] }]),
        json!([{ "type": "report" }]),
    );
    let p = run(&[r1.clone(), r2.clone()], std::slice::from_ref(&r));
    let got: Vec<PathBuf> = p.reports.iter().map(|e| e.path.clone()).collect();
    assert_eq!(got, vec![r1.join("a.pdf"), r2.join("c.pdf")]);

    r.scope = vec![r1.to_string_lossy().into_owned()];
    reset_hashed();
    let p = run(&[r1.clone(), r2], &[r]);
    assert!(p.reports.is_empty(), "the copy in B is out of scope");
    assert!(
        hashed().is_empty(),
        "a single candidate per size is not hashed"
    );
}

#[test]
fn empty_files_are_never_duplicates() {
    let (_tmp, root) = tmp_root();
    put(&root.join("a"), b"", 1000);
    put(&root.join("b"), b"", 1000);
    reset_hashed();
    let p = run(
        std::slice::from_ref(&root),
        &[dup_rule(1, json!([]), json!([{ "type": "report" }]))],
    );
    assert!(p.reports.is_empty());
    assert!(hashed().is_empty());
}

#[test]
fn files_are_hashed_once_per_plan_across_duplicate_rules() {
    let (_tmp, root) = tmp_root();
    put(&root.join("a"), b"same", 1000);
    put(&root.join("b"), b"same", 1000);
    reset_hashed();
    let p = run(
        std::slice::from_ref(&root),
        &[
            dup_rule(1, json!([]), json!([{ "type": "report" }])),
            dup_rule(2, json!([]), json!([{ "type": "report" }])),
        ],
    );
    assert_eq!(p.reports.len(), 4);
    assert_eq!(hashed(), vec![root.join("a"), root.join("b")]);
}

#[test]
fn unreadable_candidate_is_skipped_and_left_out_of_groups() {
    let (_tmp, root) = tmp_root();
    let (a, b, c) = (root.join("a"), root.join("b"), root.join("c"));
    for f in [&a, &b, &c] {
        put(f, b"same", 1000);
    }
    HASH_FAILS.with(|f| f.borrow_mut().insert(b.clone()));
    let p = run(
        std::slice::from_ref(&root),
        &[dup_rule(1, json!([]), json!([{ "type": "trash" }]))],
    );
    HASH_FAILS.with(|f| f.borrow_mut().clear());
    assert_eq!(sources(&p), vec![c]);
    assert_eq!(reports(&p)[0].0, a);
    assert_eq!(
        skips(&p),
        vec![(b, SkipReason::Unreadable, Some("dup1".into()))]
    );
}

#[test]
fn copy_claimed_by_an_earlier_rule_conflicts_and_keeper_stays() {
    let (_tmp, root) = tmp_root();
    let (a, b, c) = (root.join("a.bin"), root.join("b.bin"), root.join("c.bin"));
    put(&a, b"same", 1000);
    put(&b, b"same", 2000);
    put(&c, b"same", 3000);
    let mv = acts(
        9,
        "move-b",
        json!([{ "type": "nameContains", "value": "b." }]),
        json!([{ "type": "moveTo", "params": { "path": "Out" } }]),
    );
    let p = run(
        std::slice::from_ref(&root),
        &[mv, dup_rule(1, json!([]), json!([{ "type": "trash" }]))],
    );
    let ops: Vec<(PathBuf, OpKind)> = p.ops.iter().map(|o| (o.source.clone(), o.kind)).collect();
    assert_eq!(ops, vec![(b.clone(), OpKind::Move), (c, OpKind::Trash)]);
    assert_eq!(
        skips(&p),
        vec![(b, SkipReason::Conflict, Some("dup1".into()))]
    );
    assert_eq!(reports(&p)[0].0, a);
}

#[test]
fn keeper_alone_does_not_block_later_rules() {
    let (_tmp, root) = tmp_root();
    let (a, b) = (root.join("a.bin"), root.join("b.bin"));
    put(&a, b"same", 1000);
    put(&b, b"same", 2000);
    let p = run(
        std::slice::from_ref(&root),
        &[
            dup_rule(1, json!([]), json!([{ "type": "trash" }])),
            all(2, "move", "moveTo", "/abs/out"),
        ],
    );
    let ops: Vec<(PathBuf, OpKind)> = p.ops.iter().map(|o| (o.source.clone(), o.kind)).collect();
    assert_eq!(
        ops,
        vec![(a.clone(), OpKind::Move), (b.clone(), OpKind::Trash)]
    );
    assert_eq!(
        skips(&p),
        vec![(b, SkipReason::Conflict, Some("move".into()))]
    );
}

#[test]
fn template_tokens_valid_accepts_known_tokens_only() {
    for ok in [
        "Out",
        "{downloads}",
        "{downloads}/Imagini/{year}",
        "{home}\\Docs\\{year}-{month}-{day}",
        "Arhiva/{ext}",
        "Out/{2024}/{}",
        "~/Documents/{year}",
    ] {
        assert!(template_tokens_valid(ok), "{ok}");
    }
    for bad in [
        "{nope}",
        "Out/{downloads}",
        "{downloads}x",
        "Out/{YEAR}",
        "{desktop}/{documents}",
    ] {
        assert!(!template_tokens_valid(bad), "{bad}");
    }
}

#[test]
fn excluded_dir_name_ignores_ascii_case() {
    for n in [
        "AppData",
        "appdata",
        "PROGRAM FILES",
        "node_modules",
        "$Recycle.Bin",
    ] {
        assert!(is_excluded_dir_name(n), "{n}");
    }
    for n in ["Downloads", "AppData2", "", "targets"] {
        assert!(!is_excluded_dir_name(n), "{n}");
    }
}
