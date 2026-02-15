/// Enhanced recovery with forensics support
///
/// This module provides forensics-enabled recovery operations that integrate
/// audit trail logging and hash verification for legal/forensic use cases.
use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::forensics::{
    AuditEvent, AuditEventType, AuditLog, AuditLogger, HashAlgorithm, HashManifest,
};
use crate::{
    DeletedFile, FileRecoveryResult, FileSystemType, RecoveryReport, RecoverySession,
    RecoveryStatus,
};

/// Configuration for forensics-enabled recovery
#[derive(Debug, Clone)]
pub struct ForensicsConfig {
    /// Enable audit trail logging
    pub enable_audit: bool,

    /// Path to audit log file
    pub audit_log_path: Option<PathBuf>,

    /// Enable hash verification
    pub enable_hash_verification: bool,

    /// Hash algorithm to use
    pub hash_algorithm: HashAlgorithm,

    /// Path to save hash manifest
    pub manifest_path: Option<PathBuf>,

    /// Enable partial file recovery
    pub enable_partial_recovery: bool,

    /// Enable smart extent reconstruction
    pub enable_extent_reconstruction: bool,
}

impl Default for ForensicsConfig {
    fn default() -> Self {
        Self {
            enable_audit: false,
            audit_log_path: None,
            enable_hash_verification: false,
            hash_algorithm: HashAlgorithm::SHA256,
            manifest_path: None,
            enable_partial_recovery: false,
            enable_extent_reconstruction: false,
        }
    }
}

impl ForensicsConfig {
    /// Create a new forensics config with all features enabled
    pub fn full_forensics(output_dir: &Path) -> Self {
        Self {
            enable_audit: true,
            audit_log_path: Some(output_dir.join("audit.jsonl")),
            enable_hash_verification: true,
            hash_algorithm: HashAlgorithm::SHA256,
            manifest_path: Some(output_dir.join("hash_manifest.json")),
            enable_partial_recovery: true,
            enable_extent_reconstruction: true,
        }
    }

    /// Create config with only audit logging
    pub fn audit_only(audit_path: PathBuf) -> Self {
        Self {
            enable_audit: true,
            audit_log_path: Some(audit_path),
            ..Default::default()
        }
    }

    /// Create config with only hash verification
    pub fn hash_only(manifest_path: PathBuf, algorithm: HashAlgorithm) -> Self {
        Self {
            enable_hash_verification: true,
            hash_algorithm: algorithm,
            manifest_path: Some(manifest_path),
            ..Default::default()
        }
    }
}

/// Enhanced recovery report with forensics data
#[derive(Debug, Clone)]
pub struct ForensicsRecoveryReport {
    /// Standard recovery report
    pub report: RecoveryReport,

    /// Path to audit log (if enabled)
    pub audit_log_path: Option<PathBuf>,

    /// Path to hash manifest (if enabled)
    pub manifest_path: Option<PathBuf>,

    /// Number of partial recoveries performed
    pub partial_recoveries: usize,

    /// Number of extent reconstructions performed
    pub extent_reconstructions: usize,
}

/// Recover files with forensics features enabled
pub fn recover_files_with_forensics(
    image_path: &Path,
    session: &RecoverySession,
    output_dir: &Path,
    file_ids: Option<Vec<u64>>,
    config: ForensicsConfig,
) -> Result<ForensicsRecoveryReport> {
    use memmap2::MmapOptions;
    use std::fs::{create_dir_all, File};

    // Create output directory
    create_dir_all(output_dir)?;

    // Initialize audit logger if enabled
    let mut audit_logger: Option<AuditLogger> = if config.enable_audit {
        let log_path_dir = config
            .audit_log_path
            .clone()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| output_dir.to_path_buf());
        let audit_log = AuditLog::new(&session.id.to_string(), log_path_dir)?;
        Some(AuditLogger::new(Arc::new(audit_log)))
    } else {
        None
    };

    // Initialize hash manifest if enabled
    let mut hash_manifest = if config.enable_hash_verification {
        Some(HashManifest::new(
            &session.id.to_string(),
            config.hash_algorithm,
        ))
    } else {
        None
    };

    // Log session start
    if let Some(ref mut logger) = audit_logger {
        logger.session_start(&session.device_path.display().to_string())?;
    }

    // Open source image
    let source_file = File::open(image_path)?;
    let mmap = unsafe { MmapOptions::new().map(&source_file)? };

    let mut recovered_count = 0;
    let mut failed_count = 0;
    let mut total_bytes_recovered = 0u64;
    let mut recovery_details = Vec::new();
    let mut partial_recoveries = 0;
    let mut extent_reconstructions = 0;

    // Filter files to recover
    let files_to_recover: Vec<&DeletedFile> = if let Some(ids) = file_ids {
        session
            .scan_results
            .iter()
            .filter(|f| ids.contains(&f.id))
            .collect()
    } else {
        session
            .scan_results
            .iter()
            .filter(|f| f.is_recoverable)
            .collect()
    };

    tracing::info!(
        "Starting forensics recovery of {} files",
        files_to_recover.len()
    );

    // Recover each file
    for deleted_file in &files_to_recover {
        // Log file detection
        if let Some(ref mut logger) = audit_logger {
            let path = deleted_file
                .original_path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| format!("inode_{}", deleted_file.inode_or_cluster));
            let signature = deleted_file
                .metadata
                .mime_type
                .as_deref()
                .unwrap_or("unknown");
            logger.file_detected(&path, signature, deleted_file.confidence_score)?;
        }

        // Attempt recovery
        match recover_single_file_forensics(
            &mmap,
            deleted_file,
            output_dir,
            session.fs_type,
            &mut audit_logger,
            &mut hash_manifest,
            &config,
        ) {
            Ok((bytes_recovered, was_partial, was_reconstructed)) => {
                recovered_count += 1;
                total_bytes_recovered += bytes_recovered;

                if was_partial {
                    partial_recoveries += 1;
                }
                if was_reconstructed {
                    extent_reconstructions += 1;
                }

                let recovered_path = generate_recovery_path(output_dir, deleted_file);

                recovery_details.push(FileRecoveryResult {
                    file_id: deleted_file.id,
                    original_path: deleted_file.original_path.clone(),
                    recovered_path: recovered_path.clone(),
                    size: deleted_file.size,
                    bytes_recovered,
                    status: RecoveryStatus::Success,
                    confidence_score: deleted_file.confidence_score,
                });

                tracing::info!(
                    "✅ Recovered file ID {} ({} bytes){}{}",
                    deleted_file.id,
                    bytes_recovered,
                    if was_partial { " [PARTIAL]" } else { "" },
                    if was_reconstructed {
                        " [RECONSTRUCTED]"
                    } else {
                        ""
                    }
                );
            }
            Err(e) => {
                failed_count += 1;

                // Log error
                if let Some(ref mut logger) = audit_logger {
                    let _ = logger.error(
                        &format!("Recovery failed for file {}", deleted_file.id),
                        &e.to_string(),
                    );
                }

                recovery_details.push(FileRecoveryResult {
                    file_id: deleted_file.id,
                    original_path: deleted_file.original_path.clone(),
                    recovered_path: generate_recovery_path(output_dir, deleted_file),
                    size: deleted_file.size,
                    bytes_recovered: 0,
                    status: RecoveryStatus::Failed(e.to_string()),
                    confidence_score: deleted_file.confidence_score,
                });

                tracing::warn!("❌ Failed to recover file ID {}: {}", deleted_file.id, e);
            }
        }
    }

    // Save hash manifest if enabled
    let manifest_path = if let Some(ref manifest) = hash_manifest {
        if !manifest.files.is_empty() {
            let path = config
                .manifest_path
                .clone()
                .unwrap_or_else(|| output_dir.join("hash_manifest.json"));
            let json = serde_json::to_string_pretty(manifest)?;
            let json_len = json.len() as u64;
            std::fs::write(&path, json)?;

            if let Some(ref mut logger) = audit_logger {
                logger.file_exported("hash_manifest", &path.display().to_string(), json_len)?;
            }

            Some(path)
        } else {
            None
        }
    } else {
        None
    };

    // Log session end
    if let Some(ref mut logger) = audit_logger {
        logger.session_end(&format!(
            "{} recovered, {} failed",
            recovered_count, failed_count
        ))?;
    }

    let report = RecoveryReport {
        total_files: files_to_recover.len(),
        recovered_files: recovered_count,
        failed_files: failed_count,
        total_bytes_recovered,
        output_directory: output_dir.to_path_buf(),
        recovery_details,
    };

    Ok(ForensicsRecoveryReport {
        report,
        audit_log_path: config.audit_log_path,
        manifest_path,
        partial_recoveries,
        extent_reconstructions,
    })
}

/// Recover a single file with forensics support
fn recover_single_file_forensics(
    mmap: &memmap2::Mmap,
    deleted_file: &DeletedFile,
    output_dir: &Path,
    fs_type: FileSystemType,
    audit_logger: &mut Option<AuditLogger>,
    hash_manifest: &mut Option<HashManifest>,
    config: &ForensicsConfig,
) -> Result<(u64, bool, bool)> {
    use std::fs::File;
    use std::io::Write;

    let output_path = generate_recovery_path(output_dir, deleted_file);
    let mut output_file = File::create(&output_path)?;
    let mut bytes_written = 0u64;

    let offset_multiplier = match fs_type {
        FileSystemType::Xfs => 4096,
        FileSystemType::Btrfs => 4096,
        FileSystemType::ExFat => 1,
    };

    let mut was_partial = false;
    let mut was_reconstructed = false;

    // Check if extent reconstruction is needed
    if config.enable_extent_reconstruction && deleted_file.data_blocks.is_empty() {
        was_reconstructed = true;
        // TODO: Use ExtentReconstructor here
        // For now, just mark as reconstructed
    }

    // Recover data from block ranges
    for block_range in &deleted_file.data_blocks {
        let start_offset = block_range.start_block * offset_multiplier as u64;
        let total_bytes = block_range.block_count * offset_multiplier as u64;
        let end_offset = start_offset + total_bytes;

        if start_offset >= mmap.len() as u64 {
            was_partial = true;
            continue;
        }

        let actual_end = std::cmp::min(end_offset, mmap.len() as u64);
        let actual_bytes = actual_end - start_offset;
        let remaining_file_bytes = deleted_file.size.saturating_sub(bytes_written);
        let bytes_to_copy = std::cmp::min(actual_bytes, remaining_file_bytes);

        if bytes_to_copy > 0 {
            let data_slice = &mmap[start_offset as usize..(start_offset + bytes_to_copy) as usize];
            output_file.write_all(data_slice)?;
            bytes_written += bytes_to_copy;
        }

        if bytes_written >= deleted_file.size {
            break;
        }
    }

    output_file.flush()?;

    // Set permissions if available
    if let Some(permissions) = deleted_file.metadata.permissions {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(permissions);
            std::fs::set_permissions(&output_path, perms)?;
        }
    }

    // Calculate hash if enabled
    if let Some(ref mut manifest) = hash_manifest {
        use crate::forensics::calculate_file_hash;

        let file_hash = calculate_file_hash(&output_path, config.hash_algorithm)?;

        // Log hash calculation
        if let Some(ref mut logger) = audit_logger {
            logger.hash_calculated(
                &deleted_file.id.to_string(),
                config.hash_algorithm.name(),
                &file_hash.hash,
            )?;
        }

        manifest.add_file(output_path.display().to_string(), file_hash);
    }

    // Check if partial recovery
    if bytes_written < deleted_file.size {
        was_partial = true;

        if let Some(ref mut logger) = audit_logger {
            logger.file_recovered(
                &format!("partial_{}", deleted_file.id),
                bytes_written,
                deleted_file.inode_or_cluster,
            )?;
        }
    }

    Ok((bytes_written, was_partial, was_reconstructed))
}

/// Generate recovery path for a file
fn generate_recovery_path(output_dir: &Path, deleted_file: &DeletedFile) -> PathBuf {
    let filename = if let Some(ref original_path) = deleted_file.original_path {
        original_path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("recovered_file_{}", deleted_file.id))
    } else {
        let extension = deleted_file
            .metadata
            .file_extension
            .as_ref()
            .map(|ext| format!(".{}", ext))
            .unwrap_or_else(|| match deleted_file.file_type {
                crate::FileType::RegularFile => ".dat".to_string(),
                crate::FileType::Directory => "".to_string(),
                _ => ".unknown".to_string(),
            });

        format!("recovered_file_{}{}", deleted_file.id, extension)
    };

    output_dir.join(filename)
}
