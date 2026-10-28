//! Fault injection (PLAN.md 4.6, rust-testing suite 4): runs `valet-fault apply`
//! on a fixture, kills the child after a pseudo-random 10-500 ms, runs
//! `valet-fault recover`, then checks that every original file exists exactly
//! once (at its source xor at one destination) with its original blake3, that
//! no temp copy is left, that the journal has no unfinished operation and that
//! recovery decided every operation the crash left open. Then `valet-fault
//! undo` must restore the original tree byte-identically.
//!
//! Even iterations use the same-volume rename path, odd ones
//! `VALET_FORCE_CROSS_VOLUME=1` (copy + verify + rename + trash).
//! Iterations 2 and 3 of every 4 send the `.txt` files to the (fake) Recycle
//! Bin with a `trash` rule instead of moving them: each such original must
//! then be exactly once at its source or in the bin, and undo restores it
//! from the bin.
//! Iterations: env `VALET_FAULT_ITERS` (default 10). CI runs 100.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime};

use serde_json::{Value, json};
use valet_core::journal::{BatchKind, OpState, OpType};
use valet_core::storage::Db;

const BIN: &str = env!("CARGO_BIN_EXE_valet-fault");
const FILES: usize = 40;
const MAX_SIZE: u64 = 256 * 1024;
const EXTS: &[&str] = &["pdf", "jpg", "txt", "docx", "zip", "png"];
const SUBDIRS: &[&str] = &["", "a", "a/b"];

/// Deterministic 64-bit LCG (Knuth MMIX constants); no extra crates.
struct Lcg(u64);

impl Lcg {
    fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(1))
    }
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 11
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

type Tree = BTreeMap<PathBuf, String>;

fn hash_bytes(b: &[u8]) -> String {
    blake3::hash(b).to_hex().to_string()
}

/// Writes a file with unique content and an mtime one hour back (outside the
/// planner's being-written window).
fn write_old(path: &Path, body: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut f = File::create(path).unwrap();
    f.write_all(body).unwrap();
    f.set_modified(SystemTime::now() - Duration::from_secs(3600))
        .unwrap();
}

fn body(rng: &mut Lcg, tag: &str, len: u64) -> Vec<u8> {
    let mut out = format!("{tag}\n").into_bytes();
    while (out.len() as u64) < len {
        out.extend_from_slice(&rng.next().to_le_bytes());
    }
    out.truncate(len.max(tag.len() as u64 + 1) as usize);
    out
}

/// Sorted (relative path, blake3) of every regular file under `dir`.
fn tree(dir: &Path) -> Tree {
    let mut out = Tree::new();
    if !dir.exists() {
        return out;
    }
    for e in walkdir::WalkDir::new(dir) {
        let e = e.unwrap();
        if e.file_type().is_file() {
            let rel = e.path().strip_prefix(dir).unwrap().to_path_buf();
            out.insert(rel, hash_bytes(&fs::read(e.path()).unwrap()));
        }
    }
    out
}

/// `.valet-tmp-*` files under `dir`, skipping `skip` (the fake Recycle Bin).
fn temp_files(dir: &Path, skip: &Path) -> Vec<PathBuf> {
    walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_entry(|e| e.path() != skip)
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().starts_with(".valet-tmp-"))
        .map(|e| e.path().to_path_buf())
        .collect()
}

struct Fixture {
    tmp: tempfile::TempDir,
    /// `.txt` files go to the Recycle Bin instead of `dest/txt`.
    trash_txt: bool,
    src: PathBuf,
    dest: PathBuf,
    db: PathBuf,
    rules: PathBuf,
    original: Tree,
    pre_existing: Tree,
}

impl Fixture {
    fn trash(&self) -> PathBuf {
        self.db.parent().unwrap().join("trash")
    }
}

/// Content hashes of the files in the fake Recycle Bin (top level).
fn bin_hashes(f: &Fixture) -> Vec<String> {
    match fs::read_dir(f.trash()) {
        Ok(rd) => rd
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
            .map(|e| hash_bytes(&fs::read(e.path()).unwrap()))
            .collect(),
        Err(_) => vec![],
    }
}

fn is_txt(rel: &Path) -> bool {
    rel.extension().is_some_and(|e| e == "txt")
}

fn fixture(iter: u64) -> Fixture {
    let trash_txt = iter % 4 >= 2;
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    let dest = tmp.path().join("dest");
    let state = tmp.path().join("state");
    fs::create_dir_all(&src).unwrap();
    fs::create_dir_all(&state).unwrap();
    let mut rng = Lcg::new(iter ^ 0xF00D);
    for i in 0..FILES {
        let ext = EXTS[i % EXTS.len()];
        let sub = SUBDIRS[i % SUBDIRS.len()];
        // Every eighth file shares its name with one in another folder, so
        // the plan has collisions between sources.
        let name = if i.is_multiple_of(8) {
            format!("same.{ext}")
        } else {
            format!("file_{i:02}.{ext}")
        };
        let rel = if sub.is_empty() {
            name
        } else {
            format!("{sub}/{name}")
        };
        let len = if i == 1 { 0 } else { rng.below(MAX_SIZE) + 1 };
        write_old(
            &src.join(&rel),
            &body(&mut rng, &format!("src {iter} {i}"), len),
        );
    }
    // Pre-existing files at planned destination names: must never be overwritten.
    for (i, ext) in EXTS.iter().enumerate() {
        let rel = format!("{ext}/file_{:02}.{ext}", i + 2 * EXTS.len());
        let len = rng.below(4096) + 1;
        write_old(
            &dest.join(&rel),
            &body(&mut rng, &format!("pre {iter} {i}"), len),
        );
        let same = format!("{ext}/same.{ext}");
        write_old(
            &dest.join(&same),
            &body(&mut rng, &format!("pre-same {iter} {i}"), 64),
        );
    }
    let dest_str = dest.to_string_lossy().into_owned();
    let rules: Vec<Value> = EXTS
        .iter()
        .enumerate()
        .map(|(i, ext)| {
            let action = if trash_txt && *ext == "txt" {
                json!({ "type": "trash" })
            } else {
                json!({ "type": "moveTo", "params": { "path": format!("{dest_str}/{{ext}}") } })
            };
            json!({
                "id": format!("00000000-0000-0000-0000-{:012}", i + 1),
                "name": format!("by-ext-{ext}"),
                "enabled": true,
                "conditions": [{ "type": "ext", "value": ext }],
                "actions": [action],
            })
        })
        .collect();
    let rules_path = state.join("rules.json");
    fs::write(&rules_path, serde_json::to_vec(&rules).unwrap()).unwrap();
    Fixture {
        trash_txt,
        original: tree(&src),
        pre_existing: tree(&dest),
        src,
        dest,
        db: state.join("valet.sqlite3"),
        rules: rules_path,
        tmp,
    }
}

fn fault_cmd(cross: bool) -> Command {
    let mut c = Command::new(BIN);
    c.env("VALET_FORCE_CROSS_VOLUME", if cross { "1" } else { "0" })
        .env_remove("VALET_FAULT_SLOW_MS")
        .stdin(Stdio::null());
    c
}

/// Runs a `valet-fault` subcommand to completion and parses its JSON report.
fn run_json(cross: bool, args: &[&Path]) -> Result<Value, String> {
    let out = fault_cmd(cross)
        .args(args)
        .output()
        .map_err(|e| format!("spawn: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "{:?} exited {:?}: {}",
            args,
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| format!("bad JSON: {e}"))
}

#[derive(Default)]
struct Stats {
    killed_mid_batch: u32,
    killed_before_batch: u32,
    finished_before_kill: u32,
    recovery_decisions: usize,
    /// Iterations where the disk was not "exactly once" before recovery ran.
    dirty_before_recover: u32,
    /// Recovery explanations (text before the first path or `;`) and counts.
    explanations: BTreeMap<String, u32>,
    undone_batches: u32,
}

/// Every original file exactly once, with its content; pre-existing files
/// untouched; nothing else in src/dest; no temp file.
fn check_exactly_once(f: &Fixture) -> Result<(), String> {
    let src = tree(&f.src);
    let dest = tree(&f.dest);
    let mut errors = Vec::new();
    for (rel, h) in &f.pre_existing {
        if dest.get(rel) != Some(h) {
            errors.push(format!(
                "pre-existing dest/{} changed or gone",
                rel.display()
            ));
        }
    }
    let bin = bin_hashes(f);
    let mut in_bin = 0;
    let mut dest_by_hash: BTreeMap<&String, Vec<&PathBuf>> = BTreeMap::new();
    for (rel, h) in &dest {
        if f.pre_existing.get(rel) != Some(h) {
            dest_by_hash.entry(h).or_default().push(rel);
        }
    }
    for (rel, h) in &f.original {
        let at_src = match src.get(rel) {
            Some(x) if x == h => 1,
            Some(_) => {
                errors.push(format!("src/{} has different content", rel.display()));
                0
            }
            None => 0,
        };
        let at_dest = dest_by_hash.get(h).map_or(0, Vec::len);
        // Trashed originals count in the bin. (The bin also holds sources of
        // cross-volume moves, which are counted at their destination.)
        let at_bin = if f.trash_txt && is_txt(rel) {
            bin.iter().filter(|b| *b == h).count()
        } else {
            0
        };
        in_bin += at_bin;
        if at_src + at_dest + at_bin != 1 {
            errors.push(format!(
                "src/{}: {at_src} at source, {at_dest} at destination {:?}, {at_bin} in the bin",
                rel.display(),
                dest_by_hash.get(h)
            ));
        }
    }
    let expected = f.original.len() + f.pre_existing.len() - in_bin;
    if src.len() + dest.len() != expected {
        errors.push(format!(
            "{} files in src+dest, expected {expected}",
            src.len() + dest.len()
        ));
    }
    let temps = temp_files(f.tmp.path(), &f.trash());
    if !temps.is_empty() {
        errors.push(format!("temp files left: {temps:?}"));
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("\n"))
    }
}

fn iteration(iter: u64, rt: &tokio::runtime::Runtime, stats: &mut Stats) -> Result<(), String> {
    let f = fixture(iter);
    let cross = iter % 2 == 1;
    let mut rng = Lcg::new(iter);
    // A quarter of the runs without the sleep hook and a quarter with 1-4 ms,
    // so kills also land inside file I/O and journal commits, not only in
    // the sleeps after each state change.
    let slow_ms = match rng.below(4) {
        0 => 0,
        1 => 1 + rng.below(4),
        _ => 5 + rng.below(16),
    };
    let delay = Duration::from_millis(10 + rng.below(491));

    let mut child = fault_cmd(cross)
        .env("VALET_FAULT_SLOW_MS", slow_ms.to_string())
        .arg("apply")
        .arg(&f.db)
        .arg(&f.rules)
        .arg(&f.src)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("spawn apply: {e}"))?;
    std::thread::sleep(delay);
    let finished = child.try_wait().map_err(|e| e.to_string())?.is_some();
    if !finished {
        child.kill().map_err(|e| format!("kill: {e}"))?;
    }
    child.wait().map_err(|e| e.to_string())?;

    // What the crash left open, read before recovery.
    let (open_ops, apply_batches_before) = rt.block_on(async {
        let db = Db::connect(&f.db).await.map_err(|e| e.to_string())?;
        let j = db.journal();
        let open = j.unfinished_ops().await.map_err(|e| e.to_string())?;
        let batches = j.list_batches(100).await.map_err(|e| e.to_string())?;
        db.pool.close().await;
        Ok::<_, String>((
            open,
            batches
                .iter()
                .filter(|b| b.kind == BatchKind::Apply)
                .count(),
        ))
    })?;
    if finished {
        stats.finished_before_kill += 1;
    } else if apply_batches_before == 0 {
        stats.killed_before_batch += 1;
    } else {
        stats.killed_mid_batch += 1;
    }

    if check_exactly_once(&f).is_err() {
        stats.dirty_before_recover += 1;
    }
    let rec = run_json(cross, &[Path::new("recover"), &f.db])?;
    for d in rec["decisions"].as_array().into_iter().flatten() {
        let text = d["explanation"].as_str().unwrap_or("");
        let key = text
            .split([';', '/', '\\', '('])
            .next()
            .unwrap_or("")
            .trim();
        *stats
            .explanations
            .entry(format!(
                "{} -> {}: {key}",
                d["previous_state"], d["new_state"]
            ))
            .or_default() += 1;
    }
    let decisions = rec["decisions"].as_array().map_or(0, Vec::len);
    stats.recovery_decisions += decisions;
    if decisions != open_ops.len() {
        return Err(format!(
            "recovery decided {decisions} operations, {} were open: {rec}",
            open_ops.len()
        ));
    }

    check_exactly_once(&f).map_err(|e| format!("after recover: {e}"))?;

    let apply = rt.block_on(async {
        let db = Db::connect(&f.db).await.map_err(|e| e.to_string())?;
        let j = db.journal();
        let left = j.unfinished_ops().await.map_err(|e| e.to_string())?;
        if !left.is_empty() {
            return Err(format!("unfinished ops after recovery: {left:?}"));
        }
        let open = j.unfinished_batches().await.map_err(|e| e.to_string())?;
        if !open.is_empty() {
            return Err(format!("open batches after recovery: {open:?}"));
        }
        if let Some(rid) = rec["batch_id"].as_str() {
            let b = j.get_batch(rid).await.map_err(|e| e.to_string())?;
            if b.and_then(|b| b.summary).is_none_or(|s| s.is_empty()) {
                return Err("recovery batch has no summary".into());
            }
        }
        let batches = j.list_batches(100).await.map_err(|e| e.to_string())?;
        let apply = batches.into_iter().find(|b| b.kind == BatchKind::Apply);
        // Every `done` op: content at dest_path equals hash_after, source gone.
        if let Some(b) = &apply {
            for op in j.list_ops(&b.id).await.map_err(|e| e.to_string())? {
                match op.state {
                    OpState::Done if op.op_type == OpType::Trash => {
                        if op.source_path.exists() {
                            return Err(format!("done trash op {}: file still there", op.id));
                        }
                    }
                    OpState::Done => {
                        let dest = op.dest_path.clone().ok_or("done op without dest")?;
                        let h = fs::read(&dest).map(|b| hash_bytes(&b)).ok();
                        if h.is_none() || h != op.hash_after {
                            return Err(format!("done op {} not at {}", op.id, dest.display()));
                        }
                        if op.source_path.exists() {
                            return Err(format!("done op {}: source still there", op.id));
                        }
                    }
                    OpState::Failed => {
                        if op.error.as_deref().is_none_or(str::is_empty) {
                            return Err(format!("failed op {} has no error text", op.id));
                        }
                    }
                    s => return Err(format!("op {} is {s} after recovery", op.id)),
                }
            }
        }
        db.pool.close().await;
        Ok(apply)
    })?;

    match apply {
        Some(b) if b.counts.done > 0 => {
            let rep = run_json(cross, &[Path::new("undo"), &f.db, Path::new(&b.id)])?;
            let empty = |k: &str| rep[k].as_array().is_none_or(Vec::is_empty);
            if !empty("failed")
                || !empty("needs_confirmation")
                || !empty("restored_as")
                || !empty("restored_modified")
            {
                return Err(format!("undo incomplete: {rep}"));
            }
            if rep["undone"].as_array().map_or(0, Vec::len) as u64 != b.counts.done {
                return Err(format!(
                    "undo undid fewer than {} ops: {rep}",
                    b.counts.done
                ));
            }
            stats.undone_batches += 1;
        }
        _ => {}
    }
    let src = tree(&f.src);
    if src != f.original {
        let a: BTreeSet<_> = f.original.iter().collect();
        let b: BTreeSet<_> = src.iter().collect();
        return Err(format!(
            "after undo, source tree differs: missing {:?}, extra {:?}",
            a.difference(&b).collect::<Vec<_>>(),
            b.difference(&a).collect::<Vec<_>>()
        ));
    }
    if tree(&f.dest) != f.pre_existing {
        return Err("after undo, destination holds more than the pre-existing files".into());
    }
    if f.trash_txt {
        let bin = bin_hashes(&f);
        for (rel, h) in f.original.iter().filter(|(rel, _)| is_txt(rel)) {
            if bin.contains(h) {
                return Err(format!(
                    "after undo, src/{} is still in the bin",
                    rel.display()
                ));
            }
        }
    }
    let temps = temp_files(f.tmp.path(), &f.trash());
    if !temps.is_empty() {
        return Err(format!("after undo, temp files left: {temps:?}"));
    }
    Ok(())
}

#[test]
fn kill_during_apply_then_recover_and_undo() {
    let iters: u64 = std::env::var("VALET_FAULT_ITERS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let start = Instant::now();
    let mut stats = Stats::default();
    let mut failures = Vec::new();
    for i in 0..iters {
        if let Err(e) = iteration(i, &rt, &mut stats) {
            failures.push(format!(
                "iteration {i} ({}): {e}",
                if i % 2 == 1 {
                    "cross-volume"
                } else {
                    "same volume"
                }
            ));
        }
    }
    println!(
        "fault injection: {iters} iterations, {} failures, {:.1} s; killed mid-batch {}, \
         killed before the batch {}, finished before the kill {}; {} recovery decisions; \
         {} iterations not exactly-once before recovery; {} batches undone",
        failures.len(),
        start.elapsed().as_secs_f64(),
        stats.killed_mid_batch,
        stats.killed_before_batch,
        stats.finished_before_kill,
        stats.recovery_decisions,
        stats.dirty_before_recover,
        stats.undone_batches
    );
    for (k, n) in &stats.explanations {
        println!("  {n:4}  {k}");
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}
