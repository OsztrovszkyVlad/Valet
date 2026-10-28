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
