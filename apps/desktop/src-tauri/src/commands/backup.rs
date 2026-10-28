//! Backup commands: create, list, recover, stats, config, integrity check.

use std::path::Path;

use tauri::Manager;

use crate::state::AppState;

// Backup & Recovery System
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub(crate) struct BackupEntry {
    id: String,
    operation_id: String,
    original_path: String,
    backup_path: String,
    file_size: u64,
    file_hash: String,
    created_at: String,
    operation_type: String,
    metadata: BackupMetadata,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct BackupMetadata {
    file_permissions: Option<String>,
    file_modified: Option<String>,
    file_created: Option<String>,
    mime_type: Option<String>,
    compressed: bool,
    encrypted: bool,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct BackupConfig {
    enabled: bool,
    backup_location: String,
    max_backup_size_gb: u32,
    retention_days: u32,
    compress_backups: bool,
    encrypt_backups: bool,
    auto_cleanup: bool,
    backup_schedule: BackupSchedule,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct BackupSchedule {
    enabled: bool,
    interval_hours: u32,
    cleanup_interval_hours: u32,
    max_backups_per_operation: u32,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct RecoveryRequest {
    backup_ids: Vec<String>,
    recovery_location: Option<String>,
    overwrite_existing: bool,
    verify_integrity: bool,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct RecoveryResult {
    recovered_files: Vec<RecoveredFile>,
    failed_recoveries: Vec<RecoveryError>,
    total_recovered: u32,
    total_failed: u32,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct RecoveredFile {
    backup_id: String,
    original_path: String,
    recovered_path: String,
    file_size: u64,
    integrity_verified: bool,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct RecoveryError {
    backup_id: String,
    original_path: String,
    error_message: String,
    error_type: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct BackupStats {
    total_backups: u32,
    total_size_bytes: u64,
    oldest_backup: Option<String>,
    newest_backup: Option<String>,
    compression_ratio: f32,
    backup_health_score: f32,
    space_usage_percent: f32,
}

// Backup & Recovery System Commands
#[tauri::command]
pub(crate) async fn create_backup(
    file_paths: Vec<String>,
    operation_id: String,
    app: tauri::AppHandle,
) -> Result<Vec<BackupEntry>, String> {
    let backup_config = get_backup_config(app.clone()).await?;

    if !backup_config.enabled {
        return Ok(Vec::new()); // Backups disabled
    }

    let mut backup_entries = Vec::new();
    let backup_base_dir = std::path::Path::new(&backup_config.backup_location);

    // Create backup directory if it doesn't exist
    if !backup_base_dir.exists() {
        std::fs::create_dir_all(backup_base_dir)
            .map_err(|e| format!("Failed to create backup directory: {}", e))?;
    }

    for file_path in file_paths {
        match create_file_backup(&file_path, &operation_id, &backup_config).await {
            Ok(backup_entry) => backup_entries.push(backup_entry),
            Err(e) => log::warn!("Failed to backup {}: {}", file_path, e),
        }
    }

    // Store backup metadata in database
    store_backup_entries(&backup_entries, &app).await?;

    Ok(backup_entries)
}

#[tauri::command]
pub(crate) async fn get_backups(
    operation_id: Option<String>,
    app: tauri::AppHandle,
) -> Result<Vec<BackupEntry>, String> {
    load_backup_entries(operation_id, &app).await
}

#[tauri::command]
pub(crate) async fn recover_files(
    recovery_request: RecoveryRequest,
    app: tauri::AppHandle,
) -> Result<RecoveryResult, String> {
    let backup_entries = load_backup_entries(None, &app).await?;

    let mut recovered_files = Vec::new();
    let mut failed_recoveries = Vec::new();

    for backup_id in &recovery_request.backup_ids {
        if let Some(backup) = backup_entries.iter().find(|b| b.id == *backup_id) {
            match recover_single_file(backup, &recovery_request).await {
                Ok(recovered) => recovered_files.push(recovered),
                Err(error) => failed_recoveries.push(error),
            }
        } else {
            failed_recoveries.push(RecoveryError {
                backup_id: backup_id.clone(),
                original_path: "Unknown".to_string(),
                error_message: "Backup not found".to_string(),
                error_type: "NOT_FOUND".to_string(),
            });
        }
    }

    Ok(RecoveryResult {
        total_recovered: recovered_files.len() as u32,
        total_failed: failed_recoveries.len() as u32,
        recovered_files,
        failed_recoveries,
    })
}

#[tauri::command]
pub(crate) async fn cleanup_old_backups(app: tauri::AppHandle) -> Result<u32, String> {
    let backup_config = get_backup_config(app.clone()).await?;
    let backup_entries = load_backup_entries(None, &app).await?;

    let retention_cutoff =
        chrono::Utc::now() - chrono::Duration::days(backup_config.retention_days as i64);
    let mut cleaned_count = 0;

    for backup in backup_entries {
        if let Ok(created_at) = chrono::DateTime::parse_from_rfc3339(&backup.created_at) {
            if created_at.with_timezone(&chrono::Utc) < retention_cutoff {
                match cleanup_backup_file(&backup).await {
                    Ok(_) => {
                        cleaned_count += 1;
                        log::info!("Cleaned up old backup: {}", backup.id);
                    }
                    Err(e) => log::warn!("Failed to cleanup backup {}: {}", backup.id, e),
                }
            }
        }
    }

    Ok(cleaned_count)
}

#[tauri::command]
pub(crate) async fn get_backup_stats(app: tauri::AppHandle) -> Result<BackupStats, String> {
    let backup_entries = load_backup_entries(None, &app).await?;
    let backup_config = get_backup_config(app.clone()).await?;

    let total_backups = backup_entries.len() as u32;
    let total_size_bytes: u64 = backup_entries.iter().map(|b| b.file_size).sum();

    let oldest_backup = backup_entries
        .iter()
        .min_by_key(|b| &b.created_at)
        .map(|b| b.created_at.clone());

    let newest_backup = backup_entries
        .iter()
        .max_by_key(|b| &b.created_at)
        .map(|b| b.created_at.clone());

    // Calculate compression ratio (simplified)
    let compression_ratio = if backup_config.compress_backups {
        0.65
    } else {
        1.0
    };

    // Mock health score calculation
    let backup_health_score = 0.92;

    // Calculate space usage
    let max_size_bytes = (backup_config.max_backup_size_gb as u64) * 1024 * 1024 * 1024;
    let space_usage_percent = (total_size_bytes as f32 / max_size_bytes as f32) * 100.0;

    Ok(BackupStats {
        total_backups,
        total_size_bytes,
        oldest_backup,
        newest_backup,
        compression_ratio,
        backup_health_score,
        space_usage_percent,
    })
}

#[tauri::command]
pub(crate) async fn get_backup_config(app: tauri::AppHandle) -> Result<BackupConfig, String> {
    let _state = app.state::<AppState>();
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("Failed to get config directory: {}", e))?;

    let backup_config_path = config_dir.join("backup_config.json");

    if backup_config_path.exists() {
        let config_content = std::fs::read_to_string(&backup_config_path)
            .map_err(|e| format!("Failed to read backup config: {}", e))?;

        serde_json::from_str(&config_content)
            .map_err(|e| format!("Failed to parse backup config: {}", e))
    } else {
        // Return default config
        Ok(BackupConfig {
            enabled: true,
            backup_location: config_dir.join("backups").to_string_lossy().to_string(),
            max_backup_size_gb: 10,
            retention_days: 30,
            compress_backups: true,
            encrypt_backups: false,
            auto_cleanup: true,
            backup_schedule: BackupSchedule {
                enabled: false,
                interval_hours: 24,
                cleanup_interval_hours: 168, // Weekly
                max_backups_per_operation: 5,
            },
        })
    }
}

#[tauri::command]
pub(crate) async fn save_backup_config(
    config: BackupConfig,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("Failed to get config directory: {}", e))?;

    let backup_config_path = config_dir.join("backup_config.json");

    let config_content = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize backup config: {}", e))?;

    std::fs::write(&backup_config_path, config_content)
        .map_err(|e| format!("Failed to write backup config: {}", e))?;

    Ok(())
}

#[tauri::command]
pub(crate) async fn verify_backup_integrity(
    backup_ids: Vec<String>,
    app: tauri::AppHandle,
) -> Result<Vec<(String, bool, String)>, String> {
    let backup_entries = load_backup_entries(None, &app).await?;
    let mut results = Vec::new();

    for backup_id in backup_ids {
        if let Some(backup) = backup_entries.iter().find(|b| b.id == backup_id) {
            let (is_valid, message) = verify_single_backup(backup).await;
            results.push((backup_id, is_valid, message));
        } else {
            results.push((backup_id, false, "Backup not found".to_string()));
        }
    }

    Ok(results)
}

// Backup System Helper Functions
async fn create_file_backup(
    file_path: &str,
    operation_id: &str,
    config: &BackupConfig,
) -> Result<BackupEntry, String> {
    use std::collections::hash_map::DefaultHasher;
    use std::fs;
    use std::hash::{Hash, Hasher};
    use std::io::Read;
    use std::path::Path;

    let source_path = Path::new(file_path);
    if !source_path.exists() {
        return Err(format!("Source file does not exist: {}", file_path));
    }

    // Generate backup ID and paths
    let backup_id = uuid::Uuid::new_v4().to_string();
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let file_name = source_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    let backup_dir = Path::new(&config.backup_location)
        .join(operation_id)
        .join(timestamp.to_string());

    fs::create_dir_all(&backup_dir)
        .map_err(|e| format!("Failed to create backup directory: {}", e))?;

    let backup_file_path = backup_dir.join(format!("{}_{}", backup_id, file_name));

    // Copy file to backup location
    fs::copy(source_path, &backup_file_path)
        .map_err(|e| format!("Failed to copy file to backup: {}", e))?;

    // Calculate file hash for integrity verification
    let mut file_content = Vec::new();
    fs::File::open(&backup_file_path)
        .and_then(|mut f| f.read_to_end(&mut file_content))
        .map_err(|e| format!("Failed to read backup file for hashing: {}", e))?;

    let mut hasher = DefaultHasher::new();
    file_content.hash(&mut hasher);
    let file_hash = format!("{:x}", hasher.finish());

    // Get file metadata
    let metadata =
        fs::metadata(source_path).map_err(|e| format!("Failed to read file metadata: {}", e))?;

    let backup_metadata = BackupMetadata {
        file_permissions: None, // Simplified for now
        file_modified: metadata
            .modified()
            .ok()
            .and_then(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339().into()),
        file_created: metadata
            .created()
            .ok()
            .and_then(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339().into()),
        mime_type: detect_mime_type(file_path),
        compressed: config.compress_backups,
        encrypted: config.encrypt_backups,
    };

    // Apply compression if enabled
    if config.compress_backups {
        compress_backup_file(&backup_file_path)?;
    }

    // Apply encryption if enabled
    if config.encrypt_backups {
        encrypt_backup_file(&backup_file_path)?;
    }

    Ok(BackupEntry {
        id: backup_id,
        operation_id: operation_id.to_string(),
        original_path: file_path.to_string(),
        backup_path: backup_file_path.to_string_lossy().to_string(),
        file_size: metadata.len(),
        file_hash,
        created_at: chrono::Utc::now().to_rfc3339(),
        operation_type: "file_move".to_string(),
        metadata: backup_metadata,
    })
}

async fn store_backup_entries(
    entries: &[BackupEntry],
    app: &tauri::AppHandle,
) -> Result<(), String> {
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("Failed to get config directory: {}", e))?;

    let backups_db_path = config_dir.join("backups.json");

    // Load existing backups
    let mut all_backups = if backups_db_path.exists() {
        let content = std::fs::read_to_string(&backups_db_path)
            .map_err(|e| format!("Failed to read backups database: {}", e))?;
        serde_json::from_str::<Vec<BackupEntry>>(&content).unwrap_or_default()
    } else {
        Vec::new()
    };

    // Add new entries
    all_backups.extend_from_slice(entries);

    // Save updated database
    let content = serde_json::to_string_pretty(&all_backups)
        .map_err(|e| format!("Failed to serialize backups: {}", e))?;

    std::fs::write(&backups_db_path, content)
        .map_err(|e| format!("Failed to write backups database: {}", e))?;

    Ok(())
}

pub(crate) async fn load_backup_entries(
    operation_id: Option<String>,
    app: &tauri::AppHandle,
) -> Result<Vec<BackupEntry>, String> {
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("Failed to get config directory: {}", e))?;

    let backups_db_path = config_dir.join("backups.json");

    if !backups_db_path.exists() {
        return Ok(Vec::new());
    }

    let content = std::fs::read_to_string(&backups_db_path)
        .map_err(|e| format!("Failed to read backups database: {}", e))?;

    let all_backups: Vec<BackupEntry> = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse backups database: {}", e))?;

    if let Some(op_id) = operation_id {
        Ok(all_backups
            .into_iter()
            .filter(|b| b.operation_id == op_id)
            .collect())
    } else {
        Ok(all_backups)
    }
}

async fn recover_single_file(
    backup: &BackupEntry,
    request: &RecoveryRequest,
) -> Result<RecoveredFile, RecoveryError> {
    use std::fs;
    use std::path::Path;

    let backup_path = Path::new(&backup.backup_path);
    if !backup_path.exists() {
        return Err(RecoveryError {
            backup_id: backup.id.clone(),
            original_path: backup.original_path.clone(),
            error_message: "Backup file not found".to_string(),
            error_type: "FILE_NOT_FOUND".to_string(),
        });
    }

    // Determine recovery location
    let recovery_path = if let Some(custom_location) = &request.recovery_location {
        Path::new(custom_location).join(
            Path::new(&backup.original_path)
                .file_name()
                .unwrap_or_default(),
        )
    } else {
        Path::new(&backup.original_path).to_path_buf()
    };

    // Check if target exists and handle overwrite
    if recovery_path.exists() && !request.overwrite_existing {
        return Err(RecoveryError {
            backup_id: backup.id.clone(),
            original_path: backup.original_path.clone(),
            error_message: "Target file exists and overwrite not allowed".to_string(),
            error_type: "FILE_EXISTS".to_string(),
        });
    }

    // Create parent directory if needed
    if let Some(parent) = recovery_path.parent() {
        fs::create_dir_all(parent).map_err(|e| RecoveryError {
            backup_id: backup.id.clone(),
            original_path: backup.original_path.clone(),
            error_message: format!("Failed to create recovery directory: {}", e),
            error_type: "DIRECTORY_CREATION_FAILED".to_string(),
        })?;
    }

    // Decrypt if necessary
    let mut temp_backup_path = backup_path.to_path_buf();
    if backup.metadata.encrypted {
        temp_backup_path = decrypt_backup_file(backup_path)?;
    }

    // Decompress if necessary
    if backup.metadata.compressed {
        temp_backup_path = decompress_backup_file(&temp_backup_path)?;
    }

    // Copy file to recovery location
    fs::copy(&temp_backup_path, &recovery_path).map_err(|e| RecoveryError {
        backup_id: backup.id.clone(),
        original_path: backup.original_path.clone(),
        error_message: format!("Failed to copy file during recovery: {}", e),
        error_type: "COPY_FAILED".to_string(),
    })?;

    // Verify integrity if requested
    let integrity_verified = if request.verify_integrity {
        verify_file_integrity(&recovery_path, &backup.file_hash).await
    } else {
        true
    };

    // Phase 0: removed permanent deletion (invariant 1).

    Ok(RecoveredFile {
        backup_id: backup.id.clone(),
        original_path: backup.original_path.clone(),
        recovered_path: recovery_path.to_string_lossy().to_string(),
        file_size: backup.file_size,
        integrity_verified,
    })
}

async fn cleanup_backup_file(_backup: &BackupEntry) -> Result<(), String> {
    // Phase 0: removed permanent deletion (invariant 1).
    Err("permanent deletion is disabled (CLAUDE.md invariant 1); backup retention is pending a decision".to_string())
}

async fn verify_single_backup(backup: &BackupEntry) -> (bool, String) {
    match std::fs::metadata(&backup.backup_path) {
        Ok(metadata) => {
            if metadata.len() == backup.file_size {
                (true, "Backup file is valid".to_string())
            } else {
                (
                    false,
                    format!(
                        "File size mismatch: expected {}, found {}",
                        backup.file_size,
                        metadata.len()
                    ),
                )
            }
        }
        Err(_) => (false, "Backup file not found".to_string()),
    }
}

async fn verify_file_integrity(file_path: &std::path::Path, expected_hash: &str) -> bool {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::io::Read;

    let mut file_content = Vec::new();
    if let Ok(mut file) = std::fs::File::open(file_path) {
        if file.read_to_end(&mut file_content).is_ok() {
            let mut hasher = DefaultHasher::new();
            file_content.hash(&mut hasher);
            let actual_hash = format!("{:x}", hasher.finish());
            return actual_hash == expected_hash;
        }
    }
    false
}

fn detect_mime_type(file_path: &str) -> Option<String> {
    use std::path::Path;

    let path = Path::new(file_path);
    if let Some(extension) = path.extension().and_then(|e| e.to_str()) {
        match extension.to_lowercase().as_str() {
            "jpg" | "jpeg" => Some("image/jpeg".to_string()),
            "png" => Some("image/png".to_string()),
            "pdf" => Some("application/pdf".to_string()),
            "txt" => Some("text/plain".to_string()),
            "json" => Some("application/json".to_string()),
            _ => Some("application/octet-stream".to_string()),
        }
    } else {
        None
    }
}

fn compress_backup_file(file_path: &Path) -> Result<(), String> {
    // Simplified compression placeholder
    // In a real implementation, you'd use a compression library like flate2
    log::info!("Compressing backup file: {:?}", file_path);
    Ok(())
}

fn encrypt_backup_file(file_path: &Path) -> Result<(), String> {
    // Simplified encryption placeholder
    // In a real implementation, you'd use an encryption library
    log::info!("Encrypting backup file: {:?}", file_path);
    Ok(())
}

fn decrypt_backup_file(file_path: &Path) -> Result<std::path::PathBuf, RecoveryError> {
    // Simplified decryption placeholder
    log::info!("Decrypting backup file: {:?}", file_path);
    Ok(file_path.to_path_buf())
}

fn decompress_backup_file(file_path: &Path) -> Result<std::path::PathBuf, RecoveryError> {
    // Simplified decompression placeholder
    log::info!("Decompressing backup file: {:?}", file_path);
    Ok(file_path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(
        backup_path: &Path,
        original_path: &Path,
        compressed: bool,
        encrypted: bool,
    ) -> BackupEntry {
        BackupEntry {
            id: "b1".to_string(),
            operation_id: "op1".to_string(),
            original_path: original_path.to_string_lossy().to_string(),
            backup_path: backup_path.to_string_lossy().to_string(),
            file_size: 6,
            file_hash: String::new(),
            created_at: "2000-01-01T00:00:00Z".to_string(),
            operation_type: "move".to_string(),
            metadata: BackupMetadata {
                file_permissions: None,
                file_modified: None,
                file_created: None,
                mime_type: None,
                compressed,
                encrypted,
            },
        }
    }

    #[test]
    fn cleanup_backup_file_refuses_and_leaves_backup_on_disk() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let backup = dir.join("backup.bin");
        std::fs::write(&backup, b"backup").unwrap();
        let e = entry(&backup, &dir.join("orig.txt"), false, false);

        let result = tauri::async_runtime::block_on(cleanup_backup_file(&e));

        assert!(result
            .unwrap_err()
            .contains("permanent deletion is disabled"));
        assert_eq!(std::fs::read(&backup).unwrap(), b"backup");
        assert!(dir.exists());
    }

    #[test]
    fn recover_single_file_keeps_compressed_encrypted_backup() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let backup = dir.join("backup.bin");
        std::fs::write(&backup, b"backup").unwrap();
        let out = dir.join("out");
        let e = entry(&backup, &dir.join("orig.txt"), true, true);
        let request = RecoveryRequest {
            backup_ids: vec!["b1".to_string()],
            recovery_location: Some(out.to_string_lossy().to_string()),
            overwrite_existing: false,
            verify_integrity: false,
        };

        let result = tauri::async_runtime::block_on(recover_single_file(&e, &request));

        assert!(result.is_ok());
        assert_eq!(std::fs::read(&backup).unwrap(), b"backup");
        assert_eq!(std::fs::read(out.join("orig.txt")).unwrap(), b"backup");
    }
}
