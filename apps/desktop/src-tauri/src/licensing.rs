//! Licensing in the app (`docs/LICENSING.md`): the license and token files
//! in the config folder, plan resolution with the current clock, the plan
//! transitions (Free limits on, or off again), the online activate /
//! validate / deactivate flows, and the hourly scheduler.
//!
//! Files (next to `valet.sqlite3`): `license.valet-license`,
//! `activation.valet-token`. They are written as a temp file in the same
//! folder, then renamed over the old one (Valet's own files). Removed or
//! replaced files are moved to `license-removed/<unix>-<name>`, never
//! deleted.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use valet_core::entitlements::{
    apply_free_limits, restore_after_upgrade, Limits, FREE_FOLDERS, FREE_OWN_RULES,
};
use valet_core::license::{
    covers, parse_public_key, read_activation, read_license, resolve_plan, LicenseInfo,
    LicensePayload, PlanInputs, PlanStatus, PlanTier, Reason, PRODUCT,
};
use valet_core::storage::Db;

use crate::commands::settings::{folder_limit, read_config, watched_folders};
use crate::license_client::{ClientError, LicenseClient};
use crate::state::{config_dir, AppState};

pub(crate) const LICENSE_FILE: &str = "license.valet-license";
pub(crate) const TOKEN_FILE: &str = "activation.valet-token";
pub(crate) const REMOVED_DIR: &str = "license-removed";

/// `meta` key: the limits the last transition applied, `free` or `full`
/// (Trial and Pro). `plan_choice_pending` is set only when this turns to
/// `free`, so a choice the user made is not asked for again every hour.
pub(crate) const META_PLAN_APPLIED: &str = "plan_applied";

/// Event with the [`PlanView`] after the plan or the plan-disabled rules
/// changed.
pub(crate) const EVENT_PLAN_CHANGED: &str = "plan-changed";

pub(crate) const ERR_MACHINE: &str = "license.machine_unavailable";
pub(crate) const ERR_IO: &str = "license.io";

/// License and token files larger than this are not read.
const MAX_FILE_BYTES: u64 = 64 * 1024;

/// How often the scheduler re-resolves the plan.
const TICK: Duration = Duration::from_secs(60 * 60);
/// Online validation, when due, at most this often.
const VALIDATE_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// `VALET_BUILD_DATE` (Unix seconds), set by `build.rs`.
pub(crate) fn build_date() -> i64 {
    env!("VALET_BUILD_DATE").parse().unwrap_or(0)
}

/// Now, Unix seconds (UTC).
pub(crate) fn unix_now() -> i64 {
    chrono::Utc::now().timestamp()
}

fn non_empty(text: &'static str) -> Option<&'static str> {
    let t = text.trim();
    (!t.is_empty()).then_some(t)
}

/// An `https://` URL without spaces or quotes.
fn is_https_url(url: &str) -> bool {
    url.len() > "https://".len()
        && url.starts_with("https://")
        && !url.chars().any(|c| {
            c.is_whitespace() || c.is_control() || matches!(c, '"' | '<' | '>' | '^' | '`')
        })
}

/// The checkout URL (`license/checkout.txt` next to `pubkey.txt`), when it
/// is set and an `https://` URL.
pub(crate) fn shipped_checkout_url() -> Option<&'static str> {
    non_empty(include_str!(
        "../../../../crates/valet-core/license/checkout.txt"
    ))
    .filter(|u| is_https_url(u))
}

/// What the licensing code needs from the outside world. Injected so tests
/// use the TEST key, a fixed machine id and a local server.
#[derive(Debug, Clone)]
pub(crate) struct LicenseEnv {
    pub(crate) public_key: Option<String>,
    pub(crate) api_base: Option<String>,
    pub(crate) machine_id: Option<String>,
    pub(crate) build_date: i64,
    pub(crate) checkout_url: Option<String>,
    /// Microsoft Store (MSIX) build.
    pub(crate) packaged: bool,
    /// False only in tests (plain HTTP to 127.0.0.1).
    pub(crate) https_only: bool,
}

impl LicenseEnv {
    /// The shipped key, URL and checkout files, this machine's id, the
    /// build date and the package identity.
    pub(crate) fn system() -> Self {
        Self {
            public_key: valet_core::license::shipped_public_key().map(str::to_owned),
            api_base: valet_core::license::license_api_base().map(str::to_owned),
            machine_id: valet_platform::machine_id(),
            build_date: build_date(),
            checkout_url: shipped_checkout_url().map(str::to_owned),
            packaged: valet_platform::is_packaged(),
            https_only: true,
        }
    }

    /// `pubkey.txt` holds a key that parses and `api.txt` is set.
    pub(crate) fn configured(&self) -> bool {
        self.api_base.is_some()
            && self
                .public_key
                .as_deref()
                .and_then(parse_public_key)
                .is_some()
    }

    /// The license service client; `NotConfigured` (and no client built)
    /// unless [`LicenseEnv::configured`].
    fn client(&self) -> Result<LicenseClient, ClientError> {
        if !self.configured() {
            return Err(ClientError::NotConfigured);
        }
        LicenseClient::configured(
            self.api_base.as_deref(),
            self.public_key.as_deref(),
            self.https_only,
        )
    }
}

// ---------------------------------------------------------------- files

/// The stored license and token texts, when present and readable.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Stored {
    pub(crate) license: Option<String>,
    pub(crate) token: Option<String>,
}

fn read_small(path: &Path) -> Option<String> {
    let meta = std::fs::metadata(path).ok()?;
    if meta.len() > MAX_FILE_BYTES {
        log::warn!("License file too large, ignored");
        return None;
    }
    match std::fs::read_to_string(path) {
        Ok(t) => Some(t),
        Err(e) => {
            log::warn!("License file unreadable: {e}");
            None
        }
    }
}

pub(crate) fn read_stored(dir: &Path) -> Stored {
    Stored {
        license: read_small(&dir.join(LICENSE_FILE)),
        token: read_small(&dir.join(TOKEN_FILE)),
    }
}

/// Writes `text` to `dir/name`: a temp file in `dir`, flushed, then renamed
/// over `name`. Only for Valet's own license and token files.
pub(crate) fn write_own(dir: &Path, name: &str, text: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!(".{name}.{}.tmp", uuid::Uuid::new_v4()));
    {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)?;
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, dir.join(name))
}

/// Moves the existing `names` in `dir` to `dir/license-removed/<now>-<name>`
/// (`<now>-<n>-<name>` when taken). Never deletes, never replaces.
pub(crate) fn move_to_removed(dir: &Path, names: &[&str], now: i64) -> std::io::Result<()> {
    for name in names {
        let src = dir.join(name);
        if std::fs::symlink_metadata(&src).is_err() {
            continue;
        }
        let removed = dir.join(REMOVED_DIR);
        std::fs::create_dir_all(&removed)?;
        let mut n = 1u32;
        loop {
            let dest = if n == 1 {
                removed.join(format!("{now}-{name}"))
            } else {
                removed.join(format!("{now}-{n}-{name}"))
            };
            match valet_platform::rename_no_replace(&src, &dest) {
                Ok(()) => break,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && n < 1000 => n += 1,
                Err(e) => return Err(e),
            }
        }
    }
    Ok(())
}

fn io_err(e: std::io::Error) -> String {
    log::warn!("{ERR_IO}: {e}");
    ERR_IO.to_string()
}

// ---------------------------------------------------------------- resolution

/// Resolves the plan at `now`. Starts the trial on the first call
/// (`meta.trial_started_at`).
pub(crate) async fn resolve(
    db: &Db,
    dir: &Path,
    env: &LicenseEnv,
    now: i64,
) -> (PlanStatus, Stored) {
    let trial_started_at = match db.ensure_trial_started(now).await {
        Ok(t) => Some(t),
        Err(e) => {
            log::warn!("Trial start could not be read: {e:#}");
            None
        }
    };
    let stored = read_stored(dir);
    let status = resolve_plan(&PlanInputs {
        license: stored.license.as_deref(),
        activation: stored.token.as_deref(),
        machine_id: env.machine_id.as_deref(),
        now,
        build_date: env.build_date,
        trial_started_at,
        public_key: env.public_key.as_deref(),
    });
    (status, stored)
}

/// `get_plan_status` and the commands that change the plan return this
/// (snake_case, `docs/LICENSING.md`, "App commands").
#[derive(Serialize, Debug, Clone, PartialEq)]
pub(crate) struct PlanView {
    pub(crate) plan: PlanTier,
    pub(crate) reasons: Vec<Reason>,
    pub(crate) limits: Limits,
    pub(crate) licensing_configured: bool,
    pub(crate) checkout_available: bool,
    pub(crate) choice_pending: bool,
    pub(crate) license: Option<LicenseInfo>,
    pub(crate) trial_ends_at: Option<i64>,
    pub(crate) last_check_at: Option<i64>,
    pub(crate) packaged: bool,
}

pub(crate) fn plan_view(
    status: &PlanStatus,
    env: &LicenseEnv,
    choice_pending: bool,
    token: Option<&str>,
) -> PlanView {
    let last_check_at = match (token, env.public_key.as_deref()) {
        (Some(t), Some(k)) => read_activation(t, k).ok().map(|a| a.checked_at),
        _ => None,
    };
    PlanView {
        plan: status.plan.clone(),
        reasons: status.reasons.clone(),
        limits: Limits::for_plan(&status.plan),
        licensing_configured: env.configured(),
        checkout_available: env.checkout_url.is_some(),
        choice_pending,
        license: status.license.clone(),
        trial_ends_at: status.trial_ends_at,
        last_check_at,
        packaged: env.packaged,
    }
}

// ---------------------------------------------------------------- transitions

/// What [`apply_transitions`] changed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Transition {
    /// Free limits turned on or off (the active folders changed).
    pub(crate) tier_changed: bool,
    pub(crate) rules_changed: usize,
    pub(crate) pending_changed: bool,
}

impl Transition {
    pub(crate) fn changed(&self) -> bool {
        self.tier_changed || self.rules_changed > 0 || self.pending_changed
    }
}

fn storage(e: impl std::fmt::Display) -> String {
    log::warn!("Plan transition storage failed: {e:#}");
    "rule.storage".to_string()
}

/// Plan transitions (`docs/LICENSING.md`): on Free, `apply_free_limits`
/// (first 3 enabled own rules stay, the rest get `planDisabled`), and when
/// Free starts with more than 3 enabled own rules or more than 2 folders,
/// `plan_choice_pending = true`. On Trial or Pro, `restore_after_upgrade`
/// and `plan_choice_pending = false`. Rules are written with
/// `Db::upsert_rule`, so their `version` (and with it their approvals)
/// stays. Idempotent.
pub(crate) async fn apply_transitions(
    db: &Db,
    dir: &Path,
    plan: &PlanTier,
) -> Result<Transition, String> {
    let key = if plan.is_free() { "free" } else { "full" };
    let previous = db.meta_get(META_PLAN_APPLIED).await.map_err(storage)?;
    let tier_changed = previous.as_deref() != Some(key);
    let rules = db.list_rules().await.map_err(storage)?;
    let pending_before = db.plan_choice_pending().await.map_err(storage)?;
    let mut pending = pending_before;
    let changed = if plan.is_free() {
        let own_enabled = rules.iter().filter(|r| r.enabled && !r.is_preset()).count();
        let folders = read_config(dir).map(|c| c.inbox_paths.len()).unwrap_or(0);
        if tier_changed && (own_enabled > FREE_OWN_RULES || folders > FREE_FOLDERS) {
            pending = true;
        }
        apply_free_limits(&rules)
    } else {
        pending = false;
        restore_after_upgrade(&rules)
    };
    for rule in &changed {
        db.upsert_rule(rule).await.map_err(storage)?;
    }
    if pending != pending_before {
        db.set_plan_choice_pending(pending).await.map_err(storage)?;
    }
    if tier_changed {
        db.meta_set(META_PLAN_APPLIED, key).await.map_err(storage)?;
        log::info!("Plan limits now: {key}");
    }
    Ok(Transition {
        tier_changed,
        rules_changed: changed.len(),
        pending_changed: pending != pending_before,
    })
}

/// Result of [`sync_in`].
#[derive(Debug, Clone)]
pub(crate) struct Synced {
    pub(crate) status: PlanStatus,
    pub(crate) view: PlanView,
    pub(crate) transition: Transition,
}

/// Resolves the plan, applies the transitions and stores the plan in
/// `state` (the watcher's folder limit).
pub(crate) async fn sync_in(
    state: &AppState,
    db: &Db,
    dir: &Path,
    now: i64,
) -> Result<Synced, String> {
    let _rules = state.rule_writes.lock().await;
    let (status, stored) = resolve(db, dir, &state.license, now).await;
    let transition = apply_transitions(db, dir, &status.plan).await?;
    state.set_plan(status.plan.clone());
    let pending = db.plan_choice_pending().await.map_err(storage)?;
    let view = plan_view(&status, &state.license, pending, stored.token.as_deref());
    Ok(Synced {
        status,
        view,
        transition,
    })
}

/// [`sync_in`] for the running app; after a change the watcher restarts on
/// the folders active now and `plan-changed` is emitted.
pub(crate) async fn sync_plan<R: Runtime>(app: &AppHandle<R>, db: &Db) -> Result<Synced, String> {
    let state = app.state::<AppState>();
    let dir = config_dir(app)?;
    let synced = sync_in(&state, db, &dir, unix_now()).await?;
    if synced.transition.changed() {
        after_change(app, &state, &dir, &synced.view);
    }
    Ok(synced)
}

/// Restarts the watcher on the folders active now and emits `plan-changed`.
pub(crate) fn after_change<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    dir: &Path,
    view: &PlanView,
) {
    crate::auto::restart_watcher(
        state,
        watched_folders(dir, folder_limit(&view.plan)).unwrap_or_default(),
    );
    if let Err(e) = app.emit(EVENT_PLAN_CHANGED, view) {
        log::warn!("Failed to emit {EVENT_PLAN_CHANGED}: {e}");
    }
}

/// The plan now (synced first). For commands that enforce limits.
pub(crate) async fn current_plan<R: Runtime>(app: &AppHandle<R>) -> Result<PlanTier, String> {
    let db = connect(app).await?;
    Ok(sync_plan(app, &db).await?.status.plan)
}

pub(crate) async fn connect<R: Runtime>(app: &AppHandle<R>) -> Result<Db, String> {
    let state = app.state::<AppState>();
    Db::connect(&state._db_path).await.map_err(storage)
}

// ---------------------------------------------------------------- online

/// The license in `text` if it verifies, names `valet-pro` and covers this
/// build. Errors are the contract codes.
fn check_license_text(text: &str, env: &LicenseEnv) -> Result<LicensePayload, String> {
    let key = env
        .public_key
        .as_deref()
        .ok_or(Reason::LicenseNotConfigured.code())?;
    let lic = read_license(text, key).map_err(|r| r.code().to_string())?;
    if lic.product != PRODUCT {
        return Err(Reason::LicenseWrongProduct.code().into());
    }
    if !covers(lic.updates_until, env.build_date) {
        return Err(Reason::LicenseNotCoveringBuild.code().into());
    }
    Ok(lic)
}

/// A token from the server is stored only when it verifies and belongs to
/// this license and machine; anything else counts as a network error.
fn check_token(
    token: &str,
    env: &LicenseEnv,
    license_id: &str,
    machine: &str,
) -> Result<(), String> {
    let ok = env
        .public_key
        .as_deref()
        .and_then(|k| read_activation(token, k).ok())
        .is_some_and(|t| t.license_id == license_id && t.machine == machine);
    if ok {
        Ok(())
    } else {
        log::warn!("License service returned a token that does not verify");
        Err(ClientError::Network.code().into())
    }
}

/// `import_license_text`: verifies `text`, activates this machine, then
/// moves any stored license and token to `license-removed/` and stores the
/// new pair. On any error nothing on disk changes.
pub(crate) async fn import_in(
    env: &LicenseEnv,
    dir: &Path,
    text: &str,
    now: i64,
) -> Result<(), String> {
    let client = env.client().map_err(|e| e.code().to_string())?;
    let lic = check_license_text(text, env)?;
    let machine = env.machine_id.as_deref().ok_or(ERR_MACHINE)?;
    let token = client
        .activate(&lic.license_id, machine)
        .await
        .map_err(|e| e.code().to_string())?;
    check_token(&token, env, &lic.license_id, machine)?;
    move_to_removed(dir, &[LICENSE_FILE, TOKEN_FILE], now).map_err(io_err)?;
    write_own(dir, LICENSE_FILE, text).map_err(io_err)?;
    write_own(dir, TOKEN_FILE, &token).map_err(io_err)?;
    log::info!("License imported and activated");
    Ok(())
}

/// `check_license_now` and the scheduled check: `/v1/validate` for the
/// stored license. A new token replaces the old one; 404, 409 and 410 move
/// the token to `license-removed/` (the license file stays); a network
/// error keeps the token.
pub(crate) async fn validate_in(env: &LicenseEnv, dir: &Path, now: i64) -> Result<(), String> {
    let client = env.client().map_err(|e| e.code().to_string())?;
    let stored = read_stored(dir);
    let text = stored.license.ok_or(Reason::LicenseMissing.code())?;
    let lic = check_license_text(&text, env)?;
    let machine = env.machine_id.as_deref().ok_or(ERR_MACHINE)?;
    match client.validate(&lic.license_id, machine).await {
        Ok(token) => {
            check_token(&token, env, &lic.license_id, machine)?;
            write_own(dir, TOKEN_FILE, &token).map_err(io_err)?;
            log::info!("License validated");
            Ok(())
        }
        Err(e) if e.drops_token() => {
            log::warn!("License refused by the service: {}", e.code());
            move_to_removed(dir, &[TOKEN_FILE], now).map_err(io_err)?;
            Err(e.code().into())
        }
        Err(e) => Err(e.code().into()),
    }
}

/// `remove_license`: `/v1/deactivate` when possible (errors are logged and
/// ignored), then both files move to `license-removed/`.
pub(crate) async fn remove_in(env: &LicenseEnv, dir: &Path, now: i64) -> Result<(), String> {
    let stored = read_stored(dir);
    let lic = match (stored.license.as_deref(), env.public_key.as_deref()) {
        (Some(t), Some(k)) => read_license(t, k).ok(),
        _ => None,
    };
    if let (Some(lic), Some(machine)) = (lic, env.machine_id.as_deref()) {
        match env.client() {
            Ok(client) => {
                if let Err(e) = client.deactivate(&lic.license_id, machine).await {
                    log::warn!(
                        "License deactivation failed ({}), removing locally",
                        e.code()
                    );
                }
            }
            Err(e) => log::info!("License deactivation skipped: {}", e.code()),
        }
    }
    move_to_removed(dir, &[LICENSE_FILE, TOKEN_FILE], now).map_err(io_err)?;
    log::info!("License removed");
    Ok(())
}

// ---------------------------------------------------------------- startup

/// Startup: resolve the plan and apply the transitions before the watcher
/// starts, so it watches only the folders active on this plan. Call after
/// journal recovery.
pub(crate) fn startup_sync<R: Runtime>(app: &AppHandle<R>) {
    let result = tauri::async_runtime::block_on(async {
        let db = connect(app).await?;
        sync_plan(app, &db).await
    });
    match result {
        Ok(s) => log::info!("Plan at startup: {}", plan_kind(&s.status.plan)),
        Err(e) => log::warn!("Plan could not be resolved at startup: {e}"),
    }
}

fn plan_kind(plan: &PlanTier) -> &'static str {
    match plan {
        PlanTier::Free => "free",
        PlanTier::Trial { .. } => "trial",
        PlanTier::Pro { .. } => "pro",
    }
}

/// Every hour: re-resolve the plan and apply the transitions (a trial ends,
/// a token ages out). When the token is due for an online check, validate,
/// at most once every 24 h. The first round runs at once.
pub(crate) fn start_scheduler<R: Runtime>(app: &AppHandle<R>) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut last_validate: Option<SystemTime> = None;
        loop {
            match connect(&app).await {
                Ok(db) => scheduled_round(&app, &db, &mut last_validate).await,
                Err(e) => log::warn!("Plan check skipped: {e}"),
            }
            tokio::time::sleep(TICK).await;
        }
    });
}

async fn scheduled_round<R: Runtime>(
    app: &AppHandle<R>,
    db: &Db,
    last_validate: &mut Option<SystemTime>,
) {
    let synced = match sync_plan(app, db).await {
        Ok(s) => s,
        Err(e) => {
            log::warn!("Plan check failed: {e}");
            return;
        }
    };
    let now = SystemTime::now();
    if !validation_due(synced.status.needs_online_check, *last_validate, now) {
        return;
    }
    *last_validate = Some(now);
    let state = app.state::<AppState>();
    let Ok(dir) = config_dir(app) else {
        return;
    };
    if let Err(code) = validate_in(&state.license, &dir, unix_now()).await {
        log::info!("Scheduled license check: {code}");
    }
    if let Err(e) = sync_plan(app, db).await {
        log::warn!("Plan check failed: {e}");
    }
}

/// Validate when the token needs it and none ran in the last 24 h (or the
/// clock went back).
pub(crate) fn validation_due(needed: bool, last: Option<SystemTime>, now: SystemTime) -> bool {
    needed
        && last.is_none_or(|l| {
            now.duration_since(l)
                .map_or(true, |elapsed| elapsed >= VALIDATE_INTERVAL)
        })
}

/// The folder of the license files: the app config folder.
pub(crate) fn license_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    config_dir(app)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::license_client::tests::{serve_once, token_answer};
    use crate::license_client::CLIENTS_BUILT;
    use serde_json::json;
    use valet_core::rules::Rule;

    pub(crate) const FIXTURES: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../crates/valet-core/tests/fixtures/license"
    );
    /// The fixtures' clock (`make-fixtures.mjs`).
    pub(crate) const NOW: i64 = 1_794_000_000;
    pub(crate) const BUILD: i64 = 1_793_000_000;
    pub(crate) const MACHINE: &str = "0123456789abcdef0123456789abcdef";
    pub(crate) const LICENSE_ID: &str = "6f1c2a3b-4d5e-4f60-8a71-92b3c4d5e6f7";
    const DAY: i64 = 86_400;

    pub(crate) fn fixture(name: &str) -> String {
        std::fs::read_to_string(Path::new(FIXTURES).join(name)).unwrap()
    }

    /// TEST key, the fixtures' machine and build date, `api` as the service.
    pub(crate) fn test_env(api: Option<&str>) -> LicenseEnv {
        LicenseEnv {
            public_key: Some(fixture("TEST-pubkey.txt")),
            api_base: api.map(str::to_owned),
            machine_id: Some(MACHINE.into()),
            build_date: BUILD,
            checkout_url: None,
            packaged: false,
            https_only: false,
        }
    }

    pub(crate) async fn db(dir: &Path) -> Db {
        Db::connect(&dir.join("valet.sqlite3")).await.unwrap()
    }

    pub(crate) fn own_rule(n: u128, enabled: bool) -> Rule {
        serde_json::from_value(json!({
            "id": uuid::Uuid::from_u128(n), "name": format!("r{n}"), "enabled": enabled,
            "conditions": [{ "type": "ext", "value": "pdf" }],
            "actions": [{ "type": "report" }], "version": 4,
        }))
        .unwrap()
    }

    pub(crate) fn preset_rule(n: u128) -> Rule {
        let mut r = own_rule(n, true);
        r.options = json!({ "presetId": format!("p{n}") });
        r
    }

    #[test]
    fn build_date_is_set_at_compile_time() {
        assert!(build_date() > 1_700_000_000, "{}", build_date());
    }

    #[test]
    fn checkout_url_must_be_https() {
        assert!(is_https_url("https://valet.example/buy?x=1&y=2"));
        for bad in [
            "",
            "https://",
            "http://valet.example",
            "https://a b",
            "https://a\"b",
            "file:///c",
        ] {
            assert!(!is_https_url(bad), "{bad:?}");
        }
        // checkout.txt is empty in the repository.
        assert_eq!(shipped_checkout_url(), None);
    }

    #[test]
    fn system_env_is_not_configured_with_the_shipped_files() {
        let env = LicenseEnv::system();
        assert!(!env.configured());
        assert!(env.https_only);
        assert!(!env.packaged);
        assert_eq!(env.build_date, build_date());
    }

    #[test]
    fn plan_view_json_shape() {
        let env = LicenseEnv {
            checkout_url: Some("https://valet.example/buy".into()),
            ..test_env(Some("https://license.example"))
        };
        let status = resolve_plan(&PlanInputs {
            license: Some(&fixture("license-valid.valet-license")),
            activation: Some(&fixture("token-valid.valet-token")),
            machine_id: Some(MACHINE),
            now: NOW,
            build_date: BUILD,
            trial_started_at: Some(NOW - 30 * DAY),
            public_key: env.public_key.as_deref(),
        });
        let token = fixture("token-valid.valet-token");
        let view = plan_view(&status, &env, false, Some(&token));
        assert_eq!(
            serde_json::to_value(&view).unwrap(),
            json!({
                "plan": { "kind": "pro", "license_id": LICENSE_ID, "email": "test@example.com",
                          "updates_until": 1_821_536_000 },
                "reasons": [],
                "limits": { "own_rules": null, "folders": null, "history_days": null,
                            "undo_latest_only": false, "export_import": true },
                "licensing_configured": true,
                "checkout_available": true,
                "choice_pending": false,
                "license": { "license_id": LICENSE_ID, "email": "test@example.com",
                             "updates_until": 1_821_536_000 },
                "trial_ends_at": NOW - 30 * DAY + 14 * DAY,
                "last_check_at": 1_793_913_600,
                "packaged": false,
            })
        );

        // Free, nothing configured.
        let env = LicenseEnv {
            public_key: None,
            ..test_env(None)
        };
        let status = resolve_plan(&PlanInputs {
            license: None,
            activation: None,
            machine_id: Some(MACHINE),
            now: NOW,
            build_date: BUILD,
            trial_started_at: Some(NOW - 30 * DAY),
            public_key: None,
        });
        assert_eq!(
            serde_json::to_value(plan_view(&status, &env, true, None)).unwrap(),
            json!({
                "plan": { "kind": "free" },
                "reasons": ["license.not_configured"],
                "limits": { "own_rules": 3, "folders": 2, "history_days": 7,
                            "undo_latest_only": true, "export_import": false },
                "licensing_configured": false,
                "checkout_available": false,
                "choice_pending": true,
                "license": null,
                "trial_ends_at": NOW - 16 * DAY,
                "last_check_at": null,
                "packaged": false,
            })
        );
    }

    #[test]
    fn files_are_moved_never_replaced() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        write_own(dir, LICENSE_FILE, "a").unwrap();
        write_own(dir, LICENSE_FILE, "b").unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join(LICENSE_FILE)).unwrap(),
            "b"
        );
        write_own(dir, TOKEN_FILE, "t").unwrap();
        move_to_removed(dir, &[LICENSE_FILE, TOKEN_FILE], 100).unwrap();
        assert!(!dir.join(LICENSE_FILE).exists());
        assert!(!dir.join(TOKEN_FILE).exists());
        let removed = dir.join(REMOVED_DIR);
        assert_eq!(
            std::fs::read_to_string(removed.join(format!("100-{LICENSE_FILE}"))).unwrap(),
            "b"
        );
        assert_eq!(
            std::fs::read_to_string(removed.join(format!("100-{TOKEN_FILE}"))).unwrap(),
            "t"
        );
        // Same second again: a new name, the first stays.
        write_own(dir, LICENSE_FILE, "c").unwrap();
        move_to_removed(dir, &[LICENSE_FILE, TOKEN_FILE], 100).unwrap();
        assert_eq!(
            std::fs::read_to_string(removed.join(format!("100-{LICENSE_FILE}"))).unwrap(),
            "b"
        );
        assert_eq!(
            std::fs::read_to_string(removed.join(format!("100-2-{LICENSE_FILE}"))).unwrap(),
            "c"
        );
        // No temp files left behind.
        let left: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".tmp"))
            .collect();
        assert!(left.is_empty(), "{left:?}");
    }

    #[tokio::test]
    async fn unconfigured_flows_make_no_request() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        write_own(dir, LICENSE_FILE, &fixture("license-valid.valet-license")).unwrap();
        let before = CLIENTS_BUILT.with(|c| c.get());
        let env = LicenseEnv::system();
        let lic = fixture("license-valid.valet-license");
        assert_eq!(
            import_in(&env, dir, &lic, NOW).await.unwrap_err(),
            "license.not_configured"
        );
        assert_eq!(
            validate_in(&env, dir, NOW).await.unwrap_err(),
            "license.not_configured"
        );
        // Only the key: still not configured.
        let key_only = test_env(None);
        assert_eq!(
            import_in(&key_only, dir, &lic, NOW).await.unwrap_err(),
            "license.not_configured"
        );
        assert_eq!(
            validate_in(&key_only, dir, NOW).await.unwrap_err(),
            "license.not_configured"
        );
        remove_in(&key_only, dir, NOW).await.unwrap();
        assert_eq!(
            CLIENTS_BUILT.with(|c| c.get()),
            before,
            "no HTTP client built"
        );
        assert!(dir
            .join(REMOVED_DIR)
            .join(format!("{NOW}-{LICENSE_FILE}"))
            .exists());
    }

    #[tokio::test]
    async fn import_verifies_activates_and_stores_both_files() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let token = fixture("token-valid.valet-token");
        // Refused before any request: bad signature, wrong product, old build.
        let env = test_env(Some("http://127.0.0.1:9"));
        for (file, code) in [
            (
                "license-tampered.valet-license",
                "license.invalid_signature",
            ),
            (
                "license-other-key.valet-license",
                "license.invalid_signature",
            ),
            (
                "license-wrong-product.valet-license",
                "license.wrong_product",
            ),
            (
                "license-not-covering-build.valet-license",
                "license.not_covering_build",
            ),
        ] {
            assert_eq!(
                import_in(&env, dir, &fixture(file), NOW).await.unwrap_err(),
                code,
                "{file}"
            );
        }
        // A refusal from the service stores nothing.
        let (base, _rx) = serve_once(409, "{}".into());
        let lic = fixture("license-valid.valet-license");
        assert_eq!(
            import_in(&test_env(Some(&base)), dir, &lic, NOW)
                .await
                .unwrap_err(),
            "license.too_many"
        );
        assert_eq!(read_stored(dir), Stored::default());
        // A token for another machine is not stored.
        let (base, _rx) = serve_once(
            200,
            token_answer(&fixture("token-other-machine.valet-token")),
        );
        assert_eq!(
            import_in(&test_env(Some(&base)), dir, &lic, NOW)
                .await
                .unwrap_err(),
            "license.network"
        );
        assert_eq!(read_stored(dir), Stored::default());

        // Success.
        let (base, rx) = serve_once(200, token_answer(&token));
        import_in(&test_env(Some(&base)), dir, &lic, NOW)
            .await
            .unwrap();
        assert!(rx.recv().unwrap().0.starts_with("POST /v1/activate "));
        let stored = read_stored(dir);
        assert_eq!(stored.license.as_deref(), Some(lic.as_str()));
        let key = fixture("TEST-pubkey.txt");
        assert_eq!(
            read_activation(stored.token.as_deref().unwrap(), &key)
                .unwrap()
                .machine,
            MACHINE
        );

        // A second import moves the old pair away first.
        let (base, _rx) = serve_once(200, token_answer(&token));
        import_in(&test_env(Some(&base)), dir, &lic, NOW + 1)
            .await
            .unwrap();
        let removed = dir.join(REMOVED_DIR);
        assert!(removed.join(format!("{}-{LICENSE_FILE}", NOW + 1)).exists());
        assert!(removed.join(format!("{}-{TOKEN_FILE}", NOW + 1)).exists());
        assert!(dir.join(LICENSE_FILE).exists());
    }

    #[tokio::test]
    async fn validate_maps_answers_and_moves_the_token_on_refusal() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let lic = fixture("license-valid.valet-license");
        let old_token = fixture("token-31-days.valet-token");
        let new_token = fixture("token-valid.valet-token");
        let env_for = |base: &str| test_env(Some(base));

        // No license stored.
        let (base, _rx) = serve_once(200, token_answer(&new_token));
        assert_eq!(
            validate_in(&env_for(&base), dir, NOW).await.unwrap_err(),
            "license.missing"
        );

        write_own(dir, LICENSE_FILE, &lic).unwrap();
        write_own(dir, TOKEN_FILE, &old_token).unwrap();

        // Network errors keep the token.
        for status in [500, 503] {
            let (base, _rx) = serve_once(status, "{}".into());
            assert_eq!(
                validate_in(&env_for(&base), dir, NOW).await.unwrap_err(),
                "license.network"
            );
            assert_eq!(read_stored(dir).token.as_deref(), Some(old_token.as_str()));
        }

        // 200: new token.
        let (base, rx) = serve_once(200, token_answer(&new_token));
        validate_in(&env_for(&base), dir, NOW).await.unwrap();
        let (line, body) = rx.recv().unwrap();
        assert!(line.starts_with("POST /v1/validate "), "{line}");
        let body: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(body["license_id"], LICENSE_ID);
        assert_eq!(body["machine"], MACHINE);
        let stored: serde_json::Value =
            serde_json::from_str(&read_stored(dir).token.unwrap()).unwrap();
        assert_eq!(
            stored,
            serde_json::from_str::<serde_json::Value>(&new_token).unwrap()
        );

        // 404, 409, 410: the token moves away, the license stays.
        for (i, (status, code)) in [
            (404, "license.unknown"),
            (409, "license.too_many"),
            (410, "license.revoked"),
        ]
        .into_iter()
        .enumerate()
        {
            write_own(dir, TOKEN_FILE, &new_token).unwrap();
            let (base, _rx) = serve_once(status, "{}".into());
            let now = NOW + i as i64;
            assert_eq!(
                validate_in(&env_for(&base), dir, now).await.unwrap_err(),
                code
            );
            let stored = read_stored(dir);
            assert_eq!(stored.token, None);
            assert_eq!(stored.license.as_deref(), Some(lic.as_str()));
            assert!(dir
                .join(REMOVED_DIR)
                .join(format!("{now}-{TOKEN_FILE}"))
                .exists());
        }
    }

    #[tokio::test]
    async fn remove_deactivates_then_moves_both_files() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        write_own(dir, LICENSE_FILE, &fixture("license-valid.valet-license")).unwrap();
        write_own(dir, TOKEN_FILE, &fixture("token-valid.valet-token")).unwrap();
        let (base, rx) = serve_once(204, String::new());
        remove_in(&test_env(Some(&base)), dir, NOW).await.unwrap();
        let (line, body) = rx.recv().unwrap();
        assert!(line.starts_with("POST /v1/deactivate "), "{line}");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&body).unwrap(),
            json!({ "license_id": LICENSE_ID, "machine": MACHINE })
        );
        assert_eq!(read_stored(dir), Stored::default());
        // A network error does not block the removal.
        write_own(dir, LICENSE_FILE, &fixture("license-valid.valet-license")).unwrap();
        remove_in(&test_env(Some("http://127.0.0.1:9")), dir, NOW + 1)
            .await
            .unwrap();
        assert_eq!(read_stored(dir), Stored::default());
    }

    #[tokio::test]
    async fn resolve_starts_the_trial_once() {
        let tmp = tempfile::tempdir().unwrap();
        let db = db(tmp.path()).await;
        let env = test_env(None);
        let (s, _) = resolve(&db, tmp.path(), &env, NOW).await;
        assert_eq!(
            s.plan,
            PlanTier::Trial {
                ends_at: NOW + 14 * DAY
            }
        );
        let (s, _) = resolve(&db, tmp.path(), &env, NOW + 15 * DAY).await;
        assert_eq!(s.plan, PlanTier::Free);
        assert_eq!(s.trial_ends_at, Some(NOW + 14 * DAY));
    }

    #[tokio::test]
    async fn falling_to_free_disables_extra_rules_and_upgrading_restores_them() {
        use crate::commands::settings::{
            load_approvals, record_preview_approval, root_key, PreviewApproval,
        };
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let db = db(dir).await;
        for n in 1..=5 {
            db.upsert_rule(&own_rule(n, true)).await.unwrap();
        }
        db.upsert_rule(&own_rule(6, false)).await.unwrap();
        db.upsert_rule(&preset_rule(7)).await.unwrap();
        let inbox = PathBuf::from("C:\\in");
        record_preview_approval(
            dir,
            &PreviewApproval {
                rules: (1..=7).map(|n| (uuid::Uuid::from_u128(n), 4)).collect(),
                roots: vec![inbox.clone()],
            },
        )
        .unwrap();

        // Trial first: nothing to restore.
        let t = apply_transitions(&db, dir, &PlanTier::Trial { ends_at: NOW })
            .await
            .unwrap();
        assert_eq!(
            t,
            Transition {
                tier_changed: true,
                rules_changed: 0,
                pending_changed: false
            }
        );

        let t = apply_transitions(&db, dir, &PlanTier::Free).await.unwrap();
        assert_eq!(
            t,
            Transition {
                tier_changed: true,
                rules_changed: 2,
                pending_changed: true
            }
        );
        assert!(db.plan_choice_pending().await.unwrap());
        let rules = db.list_rules().await.unwrap();
        let own_enabled: Vec<_> = rules
            .iter()
            .filter(|r| r.enabled && !r.is_preset())
            .collect();
        assert_eq!(own_enabled.len(), 3);
        let plan_disabled: Vec<_> = rules
            .iter()
            .filter(|r| r.option_flag("planDisabled"))
            .collect();
        assert_eq!(plan_disabled.len(), 2);
        assert!(plan_disabled.iter().all(|r| !r.enabled));
        assert!(rules.iter().all(|r| r.version == 4), "versions unchanged");
        assert!(
            rules.iter().any(|r| r.is_preset() && r.enabled),
            "presets untouched"
        );
        assert!(
            !rules
                .iter()
                .find(|r| r.id == uuid::Uuid::from_u128(6))
                .unwrap()
                .enabled
        );

        // Hourly again on Free: nothing changes, pending is not asked again
        // after the user cleared it.
        db.set_plan_choice_pending(false).await.unwrap();
        let t = apply_transitions(&db, dir, &PlanTier::Free).await.unwrap();
        assert!(!t.changed(), "{t:?}");
        assert!(!db.plan_choice_pending().await.unwrap());

        // Pro again: all five enabled, the user-disabled one stays off,
        // approvals still allow them.
        db.set_plan_choice_pending(true).await.unwrap();
        let t = apply_transitions(&db, dir, &PlanTier::Trial { ends_at: NOW })
            .await
            .unwrap();
        assert_eq!(
            t,
            Transition {
                tier_changed: true,
                rules_changed: 2,
                pending_changed: true
            }
        );
        assert!(!db.plan_choice_pending().await.unwrap());
        let rules = db.list_rules().await.unwrap();
        let approvals = load_approvals(dir);
        let key = root_key(&inbox);
        for n in 1..=5 {
            let r = rules
                .iter()
                .find(|r| r.id == uuid::Uuid::from_u128(n))
                .unwrap();
            assert!(r.enabled, "r{n}");
            assert!(r.options.get("planDisabled").is_none(), "r{n}");
            assert_eq!(r.version, 4);
            assert!(approvals.allows(r, &key), "r{n} still approved");
        }
        assert!(
            !rules
                .iter()
                .find(|r| r.id == uuid::Uuid::from_u128(6))
                .unwrap()
                .enabled
        );
    }

    #[tokio::test]
    async fn free_with_too_many_folders_asks_for_a_choice() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let db = db(dir).await;
        crate::commands::settings::write_config(
            dir,
            &crate::commands::settings::Config {
                inbox_paths: vec!["C:\\a".into(), "C:\\b".into(), "C:\\c".into()],
            },
        )
        .unwrap();
        let t = apply_transitions(&db, dir, &PlanTier::Free).await.unwrap();
        assert!(t.tier_changed && t.pending_changed && t.rules_changed == 0);
        assert!(db.plan_choice_pending().await.unwrap());
    }

    #[tokio::test]
    async fn sync_stores_the_plan_in_the_state() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let db = db(dir).await;
        let state = AppState::with_license(dir.join("valet.sqlite3"), test_env(None));
        let s = sync_in(&state, &db, dir, NOW).await.unwrap();
        assert_eq!(
            s.view.plan,
            PlanTier::Trial {
                ends_at: NOW + 14 * DAY
            }
        );
        assert_eq!(state.plan(), s.view.plan);
        assert_eq!(state.folder_limit(), None);
        let s = sync_in(&state, &db, dir, NOW + 15 * DAY).await.unwrap();
        assert_eq!(s.view.plan, PlanTier::Free);
        assert!(s.transition.tier_changed);
        assert_eq!(state.folder_limit(), Some(2));
    }

    #[test]
    fn validation_runs_when_needed_at_most_daily() {
        let t0 = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        assert!(!validation_due(false, None, t0));
        assert!(validation_due(true, None, t0));
        assert!(!validation_due(true, Some(t0), t0 + TICK));
        assert!(validation_due(true, Some(t0), t0 + VALIDATE_INTERVAL));
        assert!(validation_due(true, Some(t0), t0 - Duration::from_secs(1)));
    }
}

/// End-to-end license matrix against the real license service (Node).
#[cfg(test)]
mod service_tests;

/// Plan transitions and Free undo with real files.
#[cfg(test)]
mod transition_tests;
