//! Plan transitions with real data (`docs/LICENSING.md`, "Falling back to
//! Free"): a database with own and preset rules, four watched folders,
//! approvals written by a real executed preview and a journal with real
//! file moves. The plan changes through `sync_in` (license and token files
//! on disk, as in the app), the choice through `apply_choice_in`, undo
//! through `check_undo_allowed` + `executor::undo_batch` (the `undo_batch`
//! command's steps).
//!
//! Watched folder entries are Windows-style strings checked with
//! `split_config` and an `exists` that says yes: a temp dir on Windows lies
//! under `AppData`, which `path_problem` refuses. The moved files are real.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use uuid::Uuid;
use valet_core::executor;
use valet_core::journal::{BatchKind, Journal, NewOp, OpType};
use valet_core::license::{PlanTier, Reason, DAY};
use valet_core::planner::{OpKind, Plan, PlannedOp};
use valet_core::rules::Rule;
use valet_core::storage::Db;

use super::tests::{fixture, own_rule, preset_rule, test_env, NOW};
use super::*;
use crate::commands::license::apply_choice_in;
use crate::commands::operations::{
    check_undo_allowed, execute_blocking, execute_previewed, list_batches_in, ERR_UNDO_LATEST_ONLY,
};
use crate::commands::settings::{
    load_approvals, root_key, split_config, write_config, Config, PreviewApproval,
};
use crate::state::AppState;

const FOLDERS: [&str; 4] = [
    r"C:\Users\u\Downloads",
    r"C:\Users\u\Desktop",
    r"C:\Users\u\Documents\Inbox",
    r"D:\Scans",
];

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn planned_move(source: &Path, dest: &Path, rule: u128) -> PlannedOp {
    let meta = std::fs::metadata(source).unwrap();
    PlannedOp {
        source: source.to_path_buf(),
        dest: dest.to_path_buf(),
        kind: OpKind::Move,
        rule_id: id(rule),
        rule_name: format!("r{rule}"),
        size: meta.len(),
        mtime: meta.modified().unwrap(),
    }
}

/// Every batch and operation, as JSON text.
async fn journal_snapshot(j: &Journal) -> String {
    let batches = j.list_batches(u32::MAX).await.unwrap();
    let mut out = serde_json::to_string(&batches).unwrap();
    for b in &batches {
        out.push_str(&serde_json::to_string(&j.list_ops(&b.id).await.unwrap()).unwrap());
    }
    out
}

struct Setup {
    tmp: tempfile::TempDir,
    cfg: PathBuf,
    db: Db,
    state: AppState,
}

impl Setup {
    async fn new() -> Setup {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = tmp.path().join("cfg");
        std::fs::create_dir_all(&cfg).unwrap();
        let db_path = cfg.join("valet.sqlite3");
        let db = Db::connect(&db_path).await.unwrap();
        // The trial ended 16 days before the fixtures' clock.
        db.ensure_trial_started(NOW - 30 * DAY).await.unwrap();
        let state = AppState::with_license(db_path, test_env(None));
        Setup {
            tmp,
            cfg,
            db,
            state,
        }
    }

    async fn sync(&self) -> Synced {
        sync_in(&self.state, &self.db, &self.cfg, NOW)
            .await
            .unwrap()
    }

    async fn rules(&self) -> Vec<Rule> {
        self.db.list_rules().await.unwrap()
    }

    async fn enabled_own(&self) -> BTreeSet<Uuid> {
        self.rules()
            .await
            .iter()
            .filter(|r| r.enabled && !r.is_preset())
            .map(|r| r.id)
            .collect()
    }

    async fn plan_disabled(&self) -> BTreeSet<Uuid> {
        self.rules()
            .await
            .iter()
            .filter(|r| r.option_flag("planDisabled"))
            .map(|r| r.id)
            .collect()
    }

    /// (active, inactive) watched folders on the synced plan.
    fn folders(&self) -> (Vec<String>, Vec<String>) {
        let split = split_config(
            &read_config(&self.cfg).unwrap(),
            |_| true,
            self.state.folder_limit(),
        );
        assert!(split.invalid.is_empty() && split.unavailable.is_empty());
        (
            split
                .available
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect(),
            split.inactive,
        )
    }

    /// Every rule in `ids` is approved at its stored version under every
    /// folder in `folders`.
    async fn approved(&self, ids: &[u128], folders: &[&str]) -> bool {
        let approvals = load_approvals(&self.cfg);
        let rules = self.rules().await;
        ids.iter().all(|n| {
            let r = rules.iter().find(|r| r.id == id(*n)).unwrap();
            folders
                .iter()
                .all(|f| approvals.allows(r, &root_key(Path::new(f))))
        })
    }

    fn install_license(&self) {
        write_own(
            &self.cfg,
            LICENSE_FILE,
            &fixture("license-valid.valet-license"),
        )
        .unwrap();
        write_own(&self.cfg, TOKEN_FILE, &fixture("token-valid.valet-token")).unwrap();
    }
}

fn set(ids: &[u128]) -> BTreeSet<Uuid> {
    ids.iter().map(|n| id(*n)).collect()
}

fn strings(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn downgrade_upgrade_and_choice_with_real_data() {
    let s = Setup::new().await;
    for n in 1..=5 {
        s.db.upsert_rule(&own_rule(n, true)).await.unwrap();
    }
    s.db.upsert_rule(&preset_rule(6)).await.unwrap();
    write_config(
        &s.cfg,
        &Config {
            inbox_paths: strings(&FOLDERS),
        },
    )
    .unwrap();

    // A real preview executed: two files moved, approvals for all six
    // rules under the four folders.
    let files = s.tmp.path().join("files");
    let out = s.tmp.path().join("out");
    std::fs::create_dir_all(&files).unwrap();
    std::fs::create_dir_all(&out).unwrap();
    let mut ops = Vec::new();
    for (i, name) in ["a.pdf", "b.pdf"].into_iter().enumerate() {
        let p = files.join(name);
        std::fs::write(&p, name.repeat(100)).unwrap();
        ops.push(planned_move(&p, &out.join(name), i as u128 + 1));
    }
    s.state.store_preview(
        "p-1".into(),
        Plan {
            ops,
            skipped: vec![],
            reports: vec![],
        },
        PreviewApproval {
            rules: (1..=6).map(|n| (id(n), 4)).collect(),
            roots: FOLDERS.iter().map(PathBuf::from).collect(),
        },
    );
    let report = execute_previewed(&s.state, &s.db, &s.cfg, "p-1", |_| {})
        .await
        .unwrap();
    assert_eq!(report.done_count(), 2);
    let journal = journal_snapshot(&s.db.journal()).await;

    // Trial over, no license: Free. Own rules 1-3 stay, 4-5 plan-disabled,
    // the preset stays on, folders 1-2 active, the user is asked to choose.
    let synced = s.sync().await;
    assert_eq!(synced.view.plan, PlanTier::Free);
    assert_eq!(synced.view.reasons, vec![Reason::LicenseMissing]);
    assert!(synced.view.choice_pending);
    assert_eq!(synced.transition.rules_changed, 2);
    assert_eq!(s.enabled_own().await, set(&[1, 2, 3]));
    assert_eq!(s.plan_disabled().await, set(&[4, 5]));
    assert!(s.rules().await.iter().any(|r| r.is_preset() && r.enabled));
    assert_eq!(
        s.folders(),
        (strings(&FOLDERS[..2]), strings(&FOLDERS[2..]))
    );
    assert!(s.approved(&[1, 2, 3, 6], &FOLDERS[..2]).await);
    assert!(s.rules().await.iter().all(|r| r.version == 4));

    // Upgrade (license + token on disk): everything back, approvals intact.
    s.install_license();
    let synced = s.sync().await;
    assert!(
        matches!(synced.view.plan, PlanTier::Pro { .. }),
        "{:?}",
        synced.view
    );
    assert!(!synced.view.choice_pending);
    assert_eq!(s.enabled_own().await, set(&[1, 2, 3, 4, 5]));
    assert!(s.plan_disabled().await.is_empty());
    assert_eq!(s.folders(), (strings(&FOLDERS), vec![]));
    assert!(s.approved(&[1, 2, 3, 4, 5, 6], &FOLDERS).await);

    // Downgrade again (token gone): the same three stay, still approved.
    move_to_removed(&s.cfg, &[TOKEN_FILE], NOW).unwrap();
    let synced = s.sync().await;
    assert_eq!(synced.view.plan, PlanTier::Free);
    assert_eq!(synced.view.reasons, vec![Reason::ActivationMissing]);
    assert!(synced.view.choice_pending);
    assert_eq!(s.enabled_own().await, set(&[1, 2, 3]));
    assert_eq!(s.plan_disabled().await, set(&[4, 5]));
    assert!(s.approved(&[1, 2, 3], &FOLDERS[..2]).await);
    assert!(s.rules().await.iter().all(|r| r.version == 4));

    // The user picks rules 2, 4, 5 and folders 3, 4.
    apply_choice_in(
        &s.db,
        &s.cfg,
        &PlanTier::Free,
        &[id(4), id(5), id(2)],
        &strings(&[FOLDERS[2], FOLDERS[3]]),
    )
    .await
    .unwrap();
    assert_eq!(s.enabled_own().await, set(&[2, 4, 5]));
    assert_eq!(s.plan_disabled().await, set(&[1, 3]));
    assert_eq!(
        read_config(&s.cfg).unwrap().inbox_paths,
        strings(&[FOLDERS[2], FOLDERS[3], FOLDERS[0], FOLDERS[1]])
    );
    assert_eq!(
        s.folders(),
        (
            strings(&[FOLDERS[2], FOLDERS[3]]),
            strings(&[FOLDERS[0], FOLDERS[1]])
        )
    );
    assert!(s.approved(&[2, 4, 5, 6], &[FOLDERS[2], FOLDERS[3]]).await);
    assert!(s.rules().await.iter().all(|r| r.version == 4));

    // The hourly sync on Free keeps the choice and does not ask again.
    let synced = s.sync().await;
    assert!(!synced.transition.changed(), "{:?}", synced.transition);
    assert!(!synced.view.choice_pending);
    assert_eq!(s.enabled_own().await, set(&[2, 4, 5]));

    // Pro again: all five, all four folders.
    s.install_license();
    let synced = s.sync().await;
    assert!(matches!(synced.view.plan, PlanTier::Pro { .. }));
    assert_eq!(s.enabled_own().await, set(&[1, 2, 3, 4, 5]));
    assert_eq!(s.folders().1, Vec::<String>::new());
    assert!(s.approved(&[1, 2, 3, 4, 5, 6], &FOLDERS).await);

    // No plan change touched the journal or the moved files.
    assert_eq!(journal_snapshot(&s.db.journal()).await, journal);
    assert_eq!(
        std::fs::read_to_string(out.join("a.pdf")).unwrap(),
        "a.pdf".repeat(100)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn free_undo_latest_apply_only_with_real_files() {
    let s = Setup::new().await;
    let inbox = s.tmp.path().join("in");
    let out = s.tmp.path().join("out");
    std::fs::create_dir_all(&inbox).unwrap();
    std::fs::create_dir_all(&out).unwrap();
    let write = |name: &str| {
        let p = inbox.join(name);
        std::fs::write(&p, format!("content of {name}")).unwrap();
        p
    };
    let j = s.db.journal();
    let platform = s.state.platform.clone();

    // Apply B (oldest), then apply A.
    let b1 = write("b1.pdf");
    let rb = execute_blocking(
        vec![planned_move(&b1, &out.join("b1.pdf"), 1)],
        j.clone(),
        platform.clone(),
    )
    .await
    .unwrap();
    let a1 = write("a1.pdf");
    let ra = execute_blocking(
        vec![planned_move(&a1, &out.join("a1.pdf"), 1)],
        j.clone(),
        platform.clone(),
    )
    .await
    .unwrap();
    assert_eq!((rb.done_count(), ra.done_count()), (1, 1));
    let (a, b) = (ra.batch_id.clone(), rb.batch_id.clone());

    // A crash left an operation of A journaled as planned; startup recovery
    // writes the newest batch, of kind recovery.
    let a2 = write("a2.pdf");
    j.insert_planned(
        &a,
        99,
        &NewOp {
            op_type: OpType::Move,
            source: a2.clone(),
            dest: Some(out.join("a2.pdf")),
            size: Some(std::fs::metadata(&a2).unwrap().len()),
            undo_of: None,
        },
    )
    .await
    .unwrap();
    let rec = executor::recover(&j, platform.as_ref()).await.unwrap();
    let r = rec.batch_id.unwrap();
    assert!(a2.exists(), "never started: not moved");

    let free = PlanTier::Free;
    let list = list_batches_in(&j, 50, &free, unix_now()).await.unwrap();
    let kinds: Vec<_> = list
        .batches
        .iter()
        .map(|b| (b.id.clone(), b.kind))
        .collect();
    assert_eq!(
        kinds,
        vec![
            (r.clone(), BatchKind::Recovery),
            (a.clone(), BatchKind::Apply),
            (b.clone(), BatchKind::Apply)
        ]
    );
    assert_eq!(list.latest_apply_batch_id.as_deref(), Some(a.as_str()));
    check_undo_allowed(&j, &a, &free).await.unwrap();
    assert_eq!(
        check_undo_allowed(&j, &b, &free).await.unwrap_err(),
        ERR_UNDO_LATEST_ONLY
    );
    assert_eq!(
        check_undo_allowed(&j, &r, &free).await.unwrap_err(),
        ERR_UNDO_LATEST_ONLY
    );

    // Undo A: a1 is back, the failed op is skipped.
    let undo = executor::undo_batch(&a, &j, platform.as_ref(), false)
        .await
        .unwrap();
    assert_eq!(undo.undone.len(), 1);
    assert_eq!(undo.skipped.len(), 1);
    assert_eq!(std::fs::read_to_string(&a1).unwrap(), "content of a1.pdf");
    assert!(!out.join("a1.pdf").exists());

    // A stays the latest apply batch: B is still refused on Free.
    let list = list_batches_in(&j, 50, &free, unix_now()).await.unwrap();
    assert_eq!(list.batches[0].kind, BatchKind::Undo);
    assert_eq!(list.latest_apply_batch_id.as_deref(), Some(a.as_str()));
    assert_eq!(
        check_undo_allowed(&j, &b, &free).await.unwrap_err(),
        ERR_UNDO_LATEST_ONLY
    );
    assert_eq!(
        std::fs::read_to_string(out.join("b1.pdf")).unwrap(),
        "content of b1.pdf"
    );

    // On Trial (or Pro) B can be undone.
    let trial = PlanTier::Trial { ends_at: NOW };
    check_undo_allowed(&j, &b, &trial).await.unwrap();
    let undo = executor::undo_batch(&b, &j, platform.as_ref(), false)
        .await
        .unwrap();
    assert_eq!(undo.undone.len(), 1);
    assert_eq!(std::fs::read_to_string(&b1).unwrap(), "content of b1.pdf");
}
