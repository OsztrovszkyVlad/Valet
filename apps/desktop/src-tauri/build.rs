use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    // VALET_BUILD_DATE (Unix seconds): the build date a license must cover
    // (docs/LICENSING.md). SOURCE_DATE_EPOCH when set (release builds),
    // else the time this script runs.
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");
    let date = std::env::var("SOURCE_DATE_EPOCH")
        .ok()
        .and_then(|v| v.trim().parse::<i64>().ok())
        .unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0)
        });
    println!("cargo:rustc-env=VALET_BUILD_DATE={date}");
    tauri_build::build()
}
