//! The license matrix (`docs/PLAN.md` Phase 4, qa) end to end at the app
//! layer, against the REAL license service: `node services/license/server.mjs`
//! on a random port of 127.0.0.1 with a temp JSON store and a key made by
//! `keygen.mjs` at test time; licenses come from `issue.mjs`, revocation from
//! `revoke.mjs`, support resets from `reset.mjs`. The app side is the code
//! the commands call (`import_in`, `validate_in`, `remove_in`, `sync_in`) with
//! an injected machine id, build date and `now`.
//!
//! The offline-61-days check needs the service clock in the app's future;
//! `server.mjs` always signs with the real time, so that test runs the same
//! handler, store and signing code through `scripts/license-test-server.mjs`,
//! which adds a movable clock offset.
//!
//! When `node` is not on PATH every test here prints a SKIPPED line and
//! passes.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStderr, Command, Stdio};

use serde_json::Value;
use valet_core::license::{PlanTier, Reason, DAY};
use valet_core::storage::Db;

use super::tests::own_rule;
use super::*;
use crate::state::AppState;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn node_available() -> bool {
    Command::new("node")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Runs `node <script> <args>` with `env`; stdout on success, panics
/// otherwise.
fn node(script: &Path, args: &[&str], env: &[(String, String)]) -> String {
    let out = Command::new("node")
        .arg(script)
        .args(args)
        .envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .output()
        .expect("node runs");
    assert!(
        out.status.success(),
        "{} {args:?}: {}",
        script.display(),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

/// A TEST key pair from `keygen.mjs`: the server env and the public key
/// line.
fn keygen() -> (Vec<(String, String)>, String) {
    let out = node(&repo().join("services/license/keygen.mjs"), &[], &[]);
    let env = out
        .lines()
        .filter(|l| l.starts_with("VALET_LICENSE_"))
        .map(|l| {
            let (k, v) = l.split_once('=').unwrap();
            (k.to_string(), v.to_string())
        })
        .collect::<Vec<_>>();
    assert_eq!(env.len(), 2, "{out}");
    let public = out.lines().find(|l| l.starts_with("RW")).unwrap();
    (env, public.to_string())
}

#[derive(Clone, Copy, PartialEq)]
enum Server {
    /// `services/license/server.mjs`.
    Real,
    /// `scripts/license-test-server.mjs` (same lib code, movable clock).
    Clock,
}

struct Service {
    child: Child,
    _stderr: BufReader<ChildStderr>,
    base: String,
    store: PathBuf,
    clock_file: PathBuf,
    key_env: Vec<(String, String)>,
    public_key: String,
    tmp: tempfile::TempDir,
}

impl Drop for Service {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Service {
    /// `None` (and a SKIPPED line) when node is not installed.
    fn start(kind: Server, test: &str) -> Option<Service> {
        if !node_available() {
            eprintln!("SKIPPED {test}: `node` not found on PATH; the real license service test needs Node >= 20");
            return None;
        }
        let tmp = tempfile::tempdir().unwrap();
        let store = tmp.path().join("store.json");
        let clock_file = tmp.path().join("clock-offset");
        let (key_env, public_key) = keygen();
        let mut cmd = Command::new("node");
        match kind {
            Server::Real => cmd.arg(repo().join("services/license/server.mjs")),
            Server::Clock => cmd
                .arg(repo().join("scripts/license-test-server.mjs"))
                .arg("--clock-file")
                .arg(&clock_file),
        };
        let mut child = cmd
            .arg("--port")
            .arg("0")
            .arg("--store")
            .arg(&store)
            // Not env_clear(): node on Windows needs SystemRoot and friends.
            .env_remove("VALET_LICENSE_STORE")
            // Activation API only; the webhook routes need merchant secrets.
            .env("VALET_MERCHANT", "none")
            .envs(key_env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("node server starts");
        let mut stderr = BufReader::new(child.stderr.take().unwrap());
        let mut line = String::new();
        stderr.read_line(&mut line).unwrap();
        let base = line
            .split("listening on ")
            .nth(1)
            .and_then(|r| r.split(',').next())
            .unwrap_or_else(|| panic!("server did not start: {line:?}"))
            .trim()
            .to_string();
        Some(Service {
            child,
            _stderr: stderr,
            base,
            store,
            clock_file,
            key_env,
            public_key,
            tmp,
        })
    }

    fn script(name: &str) -> PathBuf {
        repo().join("services/license").join(name)
    }

    /// `issue.mjs`: a license registered in this service's store.
    fn issue(&self, days: u32) -> (String, String) {
        self.issue_into(&self.store, &self.key_env, days)
    }

    fn issue_into(
        &self,
        store: &Path,
        key_env: &[(String, String)],
        days: u32,
    ) -> (String, String) {
        let text = node(
            &Self::script("issue.mjs"),
            &[
                "--email",
                "buyer@example.com",
                "--days",
                &days.to_string(),
                "--store",
                store.to_str().unwrap(),
            ],
            key_env,
        );
        let id = license_id(&text);
        (text, id)
    }

    fn revoke(&self, id: &str) {
        node(
            &Self::script("revoke.mjs"),
            &["--license", id, "--store", self.store.to_str().unwrap()],
            &[],
        );
    }

    fn reset(&self, id: &str) {
        node(
            &Self::script("reset.mjs"),
            &["--license", id, "--store", self.store.to_str().unwrap()],
            &[],
        );
    }

    /// The store record of `id`.
    fn record(&self, id: &str) -> Value {
        let store: Value =
            serde_json::from_str(&std::fs::read_to_string(&self.store).unwrap()).unwrap();
        store["licenses"][id].clone()
    }

    fn active_machines(&self, id: &str) -> Vec<String> {
        let mut v: Vec<String> = self.record(id)["machines"]
            .as_object()
            .map(|m| {
                m.iter()
                    .filter(|(_, e)| e["active"] == true)
                    .map(|(k, _)| k.clone())
                    .collect()
            })
            .unwrap_or_default();
        v.sort();
        v
    }

    /// Moves the service clock `secs` ahead of the real time (Clock only).
    fn set_clock(&self, secs: i64) {
        std::fs::write(&self.clock_file, secs.to_string()).unwrap();
    }

    fn env(&self, machine: &str, build_date: i64) -> LicenseEnv {
        LicenseEnv {
            public_key: Some(self.public_key.clone()),
            api_base: Some(self.base.clone()),
            machine_id: Some(machine.to_string()),
            build_date,
            checkout_url: None,
            packaged: false,
            https_only: false,
        }
    }
}

fn license_id(text: &str) -> String {
    let env: Value = serde_json::from_str(text).unwrap();
    let payload: Value = serde_json::from_str(env["payload"].as_str().unwrap()).unwrap();
    payload["license_id"].as_str().unwrap().to_string()
}

fn machine_id(n: u32) -> String {
    format!("{n:032x}")
}

/// One PC: its config folder, database and app state.
struct Pc {
    _tmp: tempfile::TempDir,
    dir: PathBuf,
    db: Db,
    env: LicenseEnv,
    state: AppState,
}

impl Pc {
    /// `trial_ended`: the trial started 30 days before `now`, so a refused
    /// Pro gives Free. Otherwise the trial starts at the first sync.
    async fn new(env: LicenseEnv, now: i64, trial_ended: bool) -> Pc {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_path_buf();
        let db = Db::connect(&dir.join("valet.sqlite3")).await.unwrap();
        if trial_ended {
            db.ensure_trial_started(now - 30 * DAY).await.unwrap();
        }
        let state = AppState::with_license(dir.join("valet.sqlite3"), env.clone());
        Pc {
            _tmp: tmp,
            dir,
            db,
            env,
            state,
        }
    }

    async fn sync(&self, now: i64) -> Synced {
        sync_in(&self.state, &self.db, &self.dir, now)
            .await
            .unwrap()
    }

    /// The same PC running another build (or another env).
    fn with_env(&mut self, env: LicenseEnv) {
        self.state = AppState::with_license(self.dir.join("valet.sqlite3"), env.clone());
        self.env = env;
    }

    fn removed(&self) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(self.dir.join(REMOVED_DIR))
            .map(|rd| {
                rd.map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        v.sort();
        v
    }
}

fn is_pro(plan: &PlanTier, id: &str) -> bool {
    matches!(plan, PlanTier::Pro { license_id, .. } if license_id == id)
}

fn checked_at(token: &str, key: &str) -> i64 {
    read_activation(token, key).unwrap().checked_at
}

#[tokio::test]
async fn real_service_three_activations_then_too_many() {
    let Some(svc) = Service::start(Server::Real, "real_service_three_activations_then_too_many")
    else {
        return;
    };
    let now = unix_now();
    let (lic, id) = svc.issue(365);

    // Machines 1-3 activate: Pro on each.
    let mut pcs = Vec::new();
    for n in 1..=3 {
        let pc = Pc::new(svc.env(&machine_id(n), now), now, true).await;
        import_in(&pc.env, &pc.dir, &lic, now).await.unwrap();
        let s = pc.sync(now).await;
        assert!(is_pro(&s.view.plan, &id), "machine {n}: {:?}", s.view);
        assert!(s.view.reasons.is_empty());
        assert!(s.view.licensing_configured);
        let last = s.view.last_check_at.unwrap();
        assert!(
            (last - now).abs() <= 5,
            "token checked_at {last} vs now {now}"
        );
        pcs.push(pc);
    }
    assert_eq!(
        svc.active_machines(&id),
        (1..=3).map(machine_id).collect::<Vec<_>>()
    );

    // The 4th machine: 409, license.too_many, nothing stored, plan stays
    // Free (trial over) or Trial (trial running).
    let pc4 = Pc::new(svc.env(&machine_id(4), now), now, true).await;
    assert_eq!(
        import_in(&pc4.env, &pc4.dir, &lic, now).await.unwrap_err(),
        "license.too_many"
    );
    assert_eq!(read_stored(&pc4.dir), Stored::default());
    let s = pc4.sync(now).await;
    assert_eq!(s.view.plan, PlanTier::Free);
    assert_eq!(s.view.reasons, vec![Reason::LicenseMissing]);
    let pc5 = Pc::new(svc.env(&machine_id(5), now), now, false).await;
    assert_eq!(
        import_in(&pc5.env, &pc5.dir, &lic, now).await.unwrap_err(),
        "license.too_many"
    );
    assert!(matches!(
        pc5.sync(now).await.view.plan,
        PlanTier::Trial { .. }
    ));
    assert_eq!(svc.active_machines(&id).len(), 3);

    // Machine 1 removes the license: the slot is freed, both files are
    // moved (not deleted), and machine 4 can activate.
    remove_in(&pcs[0].env, &pcs[0].dir, now).await.unwrap();
    assert_eq!(read_stored(&pcs[0].dir), Stored::default());
    assert_eq!(
        pcs[0].removed(),
        // Sorted: "activation…" before "license…".
        vec![
            format!("{now}-{TOKEN_FILE}"),
            format!("{now}-{LICENSE_FILE}")
        ]
    );
    assert_eq!(pcs[0].sync(now).await.view.plan, PlanTier::Free);
    import_in(&pc4.env, &pc4.dir, &lic, now).await.unwrap();
    assert!(is_pro(&pc4.sync(now).await.view.plan, &id));

    // Support reset clears all slots; machines 5, 2 and 3 take them; the
    // stored token of machine 4 then gets 409 on validate: token moved to
    // license-removed/, license file kept, plan Free with the license shown.
    svc.reset(&id);
    assert!(svc.active_machines(&id).is_empty());
    import_in(&pc5.env, &pc5.dir, &lic, now).await.unwrap();
    validate_in(&pcs[1].env, &pcs[1].dir, now).await.unwrap();
    validate_in(&pcs[2].env, &pcs[2].dir, now).await.unwrap();
    let token4 = read_stored(&pc4.dir).token.unwrap();
    let t = now + 1;
    assert_eq!(
        validate_in(&pc4.env, &pc4.dir, t).await.unwrap_err(),
        "license.too_many"
    );
    let stored = read_stored(&pc4.dir);
    assert_eq!(stored.license.as_deref(), Some(lic.as_str()));
    assert_eq!(stored.token, None);
    assert_eq!(
        std::fs::read_to_string(pc4.dir.join(REMOVED_DIR).join(format!("{t}-{TOKEN_FILE}")))
            .unwrap(),
        token4
    );
    let s = pc4.sync(t).await;
    assert_eq!(s.view.plan, PlanTier::Free);
    assert_eq!(s.view.reasons, vec![Reason::ActivationMissing]);
    assert_eq!(s.view.license.as_ref().unwrap().license_id, id);
    assert_eq!(svc.active_machines(&id), [2, 3, 5].map(machine_id).to_vec());
}

#[tokio::test]
async fn real_service_license_not_covering_the_build() {
    let Some(svc) = Service::start(Server::Real, "real_service_license_not_covering_the_build")
    else {
        return;
    };
    let now = unix_now();
    let (lic, id) = svc.issue(1);
    let updates_until = read_license(&lic, &svc.public_key).unwrap().updates_until;
    let later_build = updates_until + DAY;

    // A build released after updates_until refuses the import before any
    // request.
    let pc_new = Pc::new(svc.env(&machine_id(1), later_build), now, true).await;
    assert_eq!(
        import_in(&pc_new.env, &pc_new.dir, &lic, now)
            .await
            .unwrap_err(),
        "license.not_covering_build"
    );
    assert_eq!(read_stored(&pc_new.dir), Stored::default());
    assert!(svc.active_machines(&id).is_empty(), "no activation request");

    // Activated on a covered build, then the user installs a later build:
    // Free with license.not_covering_build, the license is still shown, and
    // the online check refuses without touching the token.
    let mut pc = Pc::new(svc.env(&machine_id(2), now), now, true).await;
    import_in(&pc.env, &pc.dir, &lic, now).await.unwrap();
    assert!(is_pro(&pc.sync(now).await.view.plan, &id));
    pc.with_env(svc.env(&machine_id(2), later_build));
    let s = pc.sync(now).await;
    assert_eq!(s.view.plan, PlanTier::Free);
    assert_eq!(s.view.reasons, vec![Reason::LicenseNotCoveringBuild]);
    assert_eq!(
        s.view.license.as_ref().unwrap().updates_until,
        updates_until
    );
    let before = read_stored(&pc.dir);
    assert_eq!(
        validate_in(&pc.env, &pc.dir, now).await.unwrap_err(),
        "license.not_covering_build"
    );
    assert_eq!(read_stored(&pc.dir), before);
    // The older build keeps working.
    pc.with_env(svc.env(&machine_id(2), now));
    assert!(is_pro(&pc.sync(now).await.view.plan, &id));
}

#[tokio::test]
async fn real_service_forged_and_unknown_licenses() {
    let Some(svc) = Service::start(Server::Real, "real_service_forged_and_unknown_licenses") else {
        return;
    };
    let now = unix_now();
    let (lic, id) = svc.issue(365);
    let pc = Pc::new(svc.env(&machine_id(1), now), now, true).await;

    // Tampered payload: one byte of the email changed after signing.
    let mut env: Value = serde_json::from_str(&lic).unwrap();
    let payload = env["payload"].as_str().unwrap().replace("buyer@", "buyes@");
    env["payload"] = Value::String(payload);
    let tampered = env.to_string();
    assert_eq!(
        import_in(&pc.env, &pc.dir, &tampered, now)
            .await
            .unwrap_err(),
        "license.invalid_signature"
    );

    // Signed with another key, registered in the same store.
    let (other_env, _) = keygen();
    let (foreign, foreign_id) = svc.issue_into(&svc.store, &other_env, 365);
    assert_eq!(
        import_in(&pc.env, &pc.dir, &foreign, now)
            .await
            .unwrap_err(),
        "license.invalid_signature"
    );
    assert_eq!(read_stored(&pc.dir), Stored::default());
    assert!(svc.active_machines(&id).is_empty());
    assert!(svc.active_machines(&foreign_id).is_empty());

    // Signed with the right key but never registered: 404.
    let (unknown, _) = svc.issue_into(&svc.tmp.path().join("other-store.json"), &svc.key_env, 365);
    assert_eq!(
        import_in(&pc.env, &pc.dir, &unknown, now)
            .await
            .unwrap_err(),
        "license.unknown"
    );
    assert_eq!(read_stored(&pc.dir), Stored::default());

    // A forged license file put next to a real token: Free.
    import_in(&pc.env, &pc.dir, &lic, now).await.unwrap();
    assert!(is_pro(&pc.sync(now).await.view.plan, &id));
    write_own(&pc.dir, LICENSE_FILE, &tampered).unwrap();
    let s = pc.sync(now).await;
    assert_eq!(s.view.plan, PlanTier::Free);
    assert_eq!(s.view.reasons, vec![Reason::LicenseInvalidSignature]);
    assert_eq!(s.view.license, None);
    write_own(&pc.dir, LICENSE_FILE, &foreign).unwrap();
    let s = pc.sync(now).await;
    assert_eq!(s.view.plan, PlanTier::Free);
    assert_eq!(s.view.reasons, vec![Reason::LicenseInvalidSignature]);
}

#[tokio::test]
async fn real_service_offline_grace_31_and_61_days() {
    let Some(svc) = Service::start(Server::Clock, "real_service_offline_grace_31_and_61_days")
    else {
        return;
    };
    let now = unix_now();
    let (lic, id) = svc.issue(365);
    let pc = Pc::new(svc.env(&machine_id(1), now), now, true).await;
    for n in 1..=5 {
        pc.db.upsert_rule(&own_rule(n, true)).await.unwrap();
    }
    import_in(&pc.env, &pc.dir, &lic, now).await.unwrap();
    let s = pc.sync(now).await;
    assert!(is_pro(&s.view.plan, &id));
    assert!(!s.status.needs_online_check);
    let token = read_stored(&pc.dir).token.unwrap();
    let t0 = checked_at(&token, &svc.public_key);

    // 31 days offline: still Pro, an online check is due.
    let s = pc.sync(t0 + 31 * DAY).await;
    assert!(is_pro(&s.view.plan, &id), "{:?}", s.view);
    assert!(s.status.needs_online_check);
    assert_eq!(s.view.last_check_at, Some(t0));

    // The service is unreachable at day 31: the token is kept.
    let dead = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        format!("http://{}", l.local_addr().unwrap())
    };
    let offline = LicenseEnv {
        api_base: Some(dead),
        ..pc.env.clone()
    };
    assert_eq!(
        validate_in(&offline, &pc.dir, t0 + 31 * DAY)
            .await
            .unwrap_err(),
        "license.network"
    );
    assert_eq!(read_stored(&pc.dir).token.as_deref(), Some(token.as_str()));

    // 61 days offline: Free; two own rules are plan-disabled, nothing is
    // deleted, the files stay.
    let day61 = t0 + 61 * DAY;
    let s = pc.sync(day61).await;
    assert_eq!(s.view.plan, PlanTier::Free);
    assert_eq!(s.view.reasons, vec![Reason::ActivationExpired]);
    assert_eq!(s.transition.rules_changed, 2);
    let rules = pc.db.list_rules().await.unwrap();
    assert_eq!(rules.len(), 5);
    assert_eq!(rules.iter().filter(|r| r.enabled).count(), 3);
    assert_eq!(read_stored(&pc.dir).token.as_deref(), Some(token.as_str()));

    // The next successful validate (service clock at day 61) restores Pro
    // and the five rules.
    svc.set_clock(day61 - unix_now());
    validate_in(&pc.env, &pc.dir, day61).await.unwrap();
    let new_token = read_stored(&pc.dir).token.unwrap();
    let t1 = checked_at(&new_token, &svc.public_key);
    assert!((t1 - day61).abs() <= 5, "new checked_at {t1} vs {day61}");
    let s = pc.sync(day61).await;
    assert!(is_pro(&s.view.plan, &id), "{:?}", s.view);
    assert!(s.view.reasons.is_empty());
    assert!(!s.status.needs_online_check);
    let rules = pc.db.list_rules().await.unwrap();
    assert!(rules
        .iter()
        .all(|r| r.enabled && !r.option_flag("planDisabled")));
    assert!(rules.iter().all(|r| r.version == 4), "versions unchanged");
}

#[tokio::test]
async fn real_service_revoked_license_moves_the_token() {
    let Some(svc) = Service::start(Server::Real, "real_service_revoked_license_moves_the_token")
    else {
        return;
    };
    let now = unix_now();
    let (lic, id) = svc.issue(365);
    let pc = Pc::new(svc.env(&machine_id(1), now), now, false).await;
    import_in(&pc.env, &pc.dir, &lic, now).await.unwrap();
    assert!(is_pro(&pc.sync(now).await.view.plan, &id));
    let token = read_stored(&pc.dir).token.unwrap();

    svc.revoke(&id);
    let t = now + 2;
    assert_eq!(
        validate_in(&pc.env, &pc.dir, t).await.unwrap_err(),
        "license.revoked"
    );
    let stored = read_stored(&pc.dir);
    assert_eq!(stored.token, None);
    assert_eq!(stored.license.as_deref(), Some(lic.as_str()));
    let moved = pc.dir.join(REMOVED_DIR).join(format!("{t}-{TOKEN_FILE}"));
    assert_eq!(
        std::fs::read_to_string(moved).unwrap(),
        token,
        "moved, not deleted"
    );
    let s = pc.sync(t).await;
    assert!(
        matches!(s.view.plan, PlanTier::Trial { .. }),
        "{:?}",
        s.view
    );
    assert_eq!(s.view.reasons, vec![Reason::ActivationMissing]);

    // Another machine can't activate a revoked license.
    let pc2 = Pc::new(svc.env(&machine_id(2), now), now, true).await;
    assert_eq!(
        import_in(&pc2.env, &pc2.dir, &lic, t).await.unwrap_err(),
        "license.revoked"
    );
    assert_eq!(read_stored(&pc2.dir), Stored::default());
    assert_eq!(pc2.sync(t).await.view.plan, PlanTier::Free);
    // The service keeps the record and the machine history.
    let rec = svc.record(&id);
    assert_eq!(rec["revoked"], true);
    assert!(rec["machines"][machine_id(1)].is_object());
}
