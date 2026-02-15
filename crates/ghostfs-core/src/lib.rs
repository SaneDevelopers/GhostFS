use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub mod fs;
pub mod recovery;

// Re-export key recovery types
pub use recovery::{
    ActivityLevel, ConfidenceReport, FileSignature, RecoveryConfig, RecoveryEngine, RecoveryError,
    RecoveryProgress, RecoveryResult, RecoveryStage, SignatureAnalysisResult,
};

// Re-export XFS recovery config for advanced users
pub use fs::xfs::XfsRecoveryConfig;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileSystemType {
    Xfs,
    Btrfs,
    ExFat,
}

impl std::fmt::Display for FileSystemType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileSystemType::Xfs => write!(f, "XFS"),
            FileSystemType::Btrfs => write!(f, "Btrfs"),
            FileSystemType::ExFat => write!(f, "exFAT"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoverySession {
    pub id: Uuid,
    pub fs_type: FileSystemType,
    pub device_path: PathBuf,
    pub created_at: DateTime<Utc>,
    pub scan_results: Vec<DeletedFile>,
    pub total_scanned: u64,
    pub confidence_threshold: f32,
    pub metadata: SessionMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    pub device_size: u64,
    pub filesystem_size: u64,
    pub block_size: u32,
    pub scan_duration_ms: u64,
    pub files_found: u32,
    pub recoverable_files: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeletedFile {
    pub id: u64,
    pub inode_or_cluster: u64, // inode (XFS/Btrfs) or cluster (exFAT)
    pub original_path: Option<PathBuf>,
    pub size: u64,
    pub deletion_time: Option<DateTime<Utc>>,
    pub confidence_score: f32, // 0.0-1.0
    pub file_type: FileType,
    pub data_blocks: Vec<BlockRange>,
    pub is_recoverable: bool,
    pub metadata: FileMetadata,

    /// Filesystem-specific metadata for confidence scoring
    /// Serialized to preserve full recovery session fidelity when saving/loading sessions
    pub fs_metadata: Option<FsSpecificMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    pub mime_type: Option<String>,
    pub file_extension: Option<String>,
    pub permissions: Option<u32>,
    pub owner_uid: Option<u32>,
    pub owner_gid: Option<u32>,
    pub created_time: Option<DateTime<Utc>>,
    pub modified_time: Option<DateTime<Utc>>,
    pub accessed_time: Option<DateTime<Utc>>,
    pub extended_attributes: HashMap<String, Vec<u8>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileType {
    RegularFile,
    Directory,
    SymbolicLink,
    BlockDevice,
    CharacterDevice,
    Fifo,
    Socket,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockRange {
    pub start_block: u64,
    pub block_count: u64,
    pub is_allocated: bool,
}

/// Filesystem-specific metadata for confidence scoring
/// This metadata is crucial for accurate confidence calculations and is now fully serializable
/// to support session persistence and recovery result caching.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FsSpecificMetadata {
    Xfs(XfsFileMetadata),
    Btrfs(BtrfsFileMetadata),
    ExFat(ExFatFileMetadata),
}

/// XFS-specific file metadata for confidence scoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XfsFileMetadata {
    /// Which allocation group contains the inode
    pub ag_number: u32,
    /// Inode number within the AG
    pub ag_inode_number: u32,
    /// Number of data extents
    pub extent_count: u32,
    /// Format of extent storage (local/extent list/btree)
    pub extent_format: XfsExtentFormat,
    /// Whether extents are properly aligned
    pub is_aligned: bool,
    /// Link count before deletion
    pub last_link_count: u32,
    /// XFS generation counter
    pub inode_generation: u32,
}

/// XFS extent storage format
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum XfsExtentFormat {
    /// Data stored directly in inode (small files)
    Local,
    /// Direct extent list in inode
    Extents,
    /// B+tree format (large files with many extents)
    Btree,
}

/// Btrfs-specific file metadata for confidence scoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BtrfsFileMetadata {
    /// Btrfs generation number
    pub generation: u64,
    /// Transaction ID
    pub transid: u64,
    /// Whether checksum validation passed
    pub checksum_valid: bool,
    /// File exists in a snapshot
    pub in_snapshot: bool,
    /// Number of COW (copy-on-write) extents
    pub cow_extent_count: u32,
    /// Extent reference counts
    pub extent_refs: Vec<u64>,
    /// Level in B-tree (0 = leaf)
    pub tree_level: u8,
}

/// exFAT-specific file metadata for confidence scoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExFatFileMetadata {
    /// Starting cluster number
    pub first_cluster: u32,
    /// Full FAT cluster chain
    pub cluster_chain: Vec<u32>,
    /// Whether FAT chain is valid and complete
    pub chain_valid: bool,
    /// Whether filename is valid UTF-16
    pub utf16_valid: bool,
    /// Number of directory entries (file + stream + name entries)
    pub entry_count: u8,
    /// Directory entry set checksum
    pub checksum: u16,
    /// File attributes from directory entry
    pub attributes: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEntry {
    pub timestamp: DateTime<Utc>,
    pub event_type: TimelineEventType,
    pub file_id: u64,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TimelineEventType {
    FileCreated,
    FileModified,
    FileDeleted,
    FileRecovered,
}

/// Main scanning function - enhanced version
pub fn scan_image(image_path: &Path, fs: FileSystemType) -> Result<RecoverySession> {
    tracing::info!("Starting scan of {} as {}", image_path.display(), fs);

    // For now, create a basic session with placeholder data
    // This will be replaced with actual file system scanning logic
    let session = RecoverySession {
        id: Uuid::new_v4(),
        fs_type: fs,
        device_path: image_path.to_path_buf(),
        created_at: Utc::now(),
        scan_results: Vec::new(),
        total_scanned: 0,
        confidence_threshold: 0.5,
        metadata: SessionMetadata {
            device_size: std::fs::metadata(image_path)?.len(),
            filesystem_size: 0,
            block_size: 4096,
            scan_duration_ms: 0,
            files_found: 0,
            recoverable_files: 0,
        },
    };

    tracing::info!(
        "Created session {} for {} file system",
        session.id,
        session.fs_type
    );

    Ok(session)
}

/// Scan and analyze using the advanced recovery engine
pub fn scan_and_analyze(image_path: &Path, fs: FileSystemType) -> Result<RecoverySession> {
    scan_and_analyze_with_config(image_path, fs, None)
}

/// Scan and analyze with custom XFS configuration
pub fn scan_and_analyze_with_config(
    image_path: &Path,
    fs: FileSystemType,
    xfs_config: Option<fs::xfs::XfsRecoveryConfig>,
) -> Result<RecoverySession> {
    use memmap2::MmapOptions;
    use recovery::{RecoveryConfig, RecoveryEngine, RecoveryStrategy, ScanDepth};
    use std::fs::File;

    // Software auto-determines recoverability based on confidence scoring
    // Files with >= 40% confidence are marked as recoverable
    const AUTO_CONFIDENCE_THRESHOLD: f32 = 0.4;

    let file = File::open(image_path)?;
    let mmap = unsafe { MmapOptions::new().map(&file)? };

    // Create recovery configuration
    let config = RecoveryConfig {
        min_confidence_threshold: AUTO_CONFIDENCE_THRESHOLD,
        scan_depth: ScanDepth::Standard,
        recovery_strategies: vec![
            RecoveryStrategy::DirectoryTableScan,
            RecoveryStrategy::FileSignatureScan,
            RecoveryStrategy::MetadataReconstruction,
        ],
        xfs_config,
        ..Default::default()
    };

    // Initialize recovery engine
    let session_id = Uuid::new_v4().to_string();
    let mut engine = RecoveryEngine::new(
        fs,
        mmap,
        4096, // Default block size
        session_id.clone(),
        config,
    );

    // Set up progress callback
    engine.set_progress_callback(|progress| {
        tracing::info!(
            "Recovery progress: {:.1}% - {} ({} files found)",
            progress.progress_percent,
            progress.current_operation,
            progress.files_found
        );
    });

    // Execute recovery
    let recovery_result = engine.execute_recovery()?;

    // Convert to legacy session format
    let session = RecoverySession {
        id: Uuid::parse_str(&recovery_result.session_id)?,
        fs_type: fs,
        device_path: image_path.to_path_buf(),
        created_at: Utc::now(),
        scan_results: recovery_result.files,
        total_scanned: recovery_result.total_files_found as u64,
        confidence_threshold: AUTO_CONFIDENCE_THRESHOLD,
        metadata: SessionMetadata {
            device_size: std::fs::metadata(image_path)?.len(),
            filesystem_size: std::fs::metadata(image_path)?.len(),
            block_size: 4096,
            scan_duration_ms: 0, // TODO: Track actual duration
            files_found: recovery_result.total_files_found as u32,
            recoverable_files: recovery_result.recoverable_files as u32,
        },
    };

    tracing::info!(
        "Recovery complete: {} files found, {} recoverable (auto-threshold: {})",
        recovery_result.total_files_found,
        recovery_result.recoverable_files,
        AUTO_CONFIDENCE_THRESHOLD
    );

    Ok(session)
}

/// Recover files from a session to the specified output directory
pub fn recover_files(
    image_path: &Path,
    session: &RecoverySession,
    output_dir: &Path,
    file_ids: Option<Vec<u64>>,
) -> Result<RecoveryReport> {
    use memmap2::MmapOptions;
    use std::fs::create_dir_all;

    // Create output directory if it doesn't exist
    create_dir_all(output_dir)?;

    // Open the source image for reading
    let source_file = std::fs::File::open(image_path)?;
    let mmap = unsafe { MmapOptions::new().map(&source_file)? };

    let mut recovered_count = 0;
    let mut failed_count = 0;
    let mut total_bytes_recovered = 0u64;
    let mut recovery_details = Vec::new();

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
        "Starting recovery of {} files to {}",
        files_to_recover.len(),
        output_dir.display()
    );

    for deleted_file in &files_to_recover {
        match recover_single_file(&mmap, deleted_file, output_dir, session.fs_type) {
            Ok(bytes_recovered) => {
                recovered_count += 1;
                total_bytes_recovered += bytes_recovered;
                recovery_details.push(FileRecoveryResult {
                    file_id: deleted_file.id,
                    original_path: deleted_file.original_path.clone(),
                    recovered_path: generate_recovery_path(output_dir, deleted_file),
                    size: deleted_file.size,
                    bytes_recovered,
                    status: RecoveryStatus::Success,
                    confidence_score: deleted_file.confidence_score,
                });
                tracing::info!(
                    "✅ Recovered file ID {} ({} bytes)",
                    deleted_file.id,
                    bytes_recovered
                );
            }
            Err(e) => {
                failed_count += 1;
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

    let report = RecoveryReport {
        total_files: files_to_recover.len(),
        recovered_files: recovered_count,
        failed_files: failed_count,
        total_bytes_recovered,
        output_directory: output_dir.to_path_buf(),
        recovery_details,
    };

    tracing::info!(
        "Recovery complete: {}/{} files recovered, {} bytes total",
        recovered_count,
        files_to_recover.len(),
        total_bytes_recovered
    );

    Ok(report)
}

/// Recover a single file from the memory-mapped source
fn recover_single_file(
    mmap: &memmap2::Mmap,
    deleted_file: &DeletedFile,
    output_dir: &Path,
    fs_type: FileSystemType,
) -> Result<u64> {
    let output_path = generate_recovery_path(output_dir, deleted_file);
    let mut output_file = File::create(&output_path)?;
    let mut bytes_written = 0u64;

    // Determine block-to-byte conversion multiplier based on filesystem type
    // XFS/Btrfs: block numbers need to be multiplied by block size (4096)
    // exFAT: data_blocks already store byte offsets, so multiplier is 1
    let offset_multiplier = match fs_type {
        FileSystemType::Xfs => 4096,
        FileSystemType::Btrfs => 4096,
        FileSystemType::ExFat => 1, // exFAT data_blocks use byte offsets
    };

    // Recover data from each block range
    for block_range in &deleted_file.data_blocks {
        let start_offset = block_range.start_block * offset_multiplier as u64;
        let total_bytes = block_range.block_count * offset_multiplier as u64;
        let end_offset = start_offset + total_bytes;

        // Make sure we don't read past the end of the image
        if start_offset >= mmap.len() as u64 {
            tracing::warn!("Block range starts beyond image bounds: {}", start_offset);
            continue;
        }

        let actual_end = std::cmp::min(end_offset, mmap.len() as u64);
        let actual_bytes = actual_end - start_offset;

        // Also limit by the file's expected size
        let remaining_file_bytes = deleted_file.size.saturating_sub(bytes_written);
        let bytes_to_copy = std::cmp::min(actual_bytes, remaining_file_bytes);

        if bytes_to_copy > 0 {
            let data_slice = &mmap[start_offset as usize..(start_offset + bytes_to_copy) as usize];
            output_file.write_all(data_slice)?;
            bytes_written += bytes_to_copy;

            tracing::debug!(
                "Copied {} bytes from block range {}-{}",
                bytes_to_copy,
                block_range.start_block,
                block_range.start_block + block_range.block_count
            );
        }

        // Stop if we've recovered the expected file size
        if bytes_written >= deleted_file.size {
            break;
        }
    }

    output_file.flush()?;

    // Set file permissions if available
    if let Some(permissions) = deleted_file.metadata.permissions {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(permissions);
            std::fs::set_permissions(&output_path, perms)?;
        }
    }

    Ok(bytes_written)
}

/// Generate a recovery path for a deleted file
fn generate_recovery_path(output_dir: &Path, deleted_file: &DeletedFile) -> PathBuf {
    let filename = if let Some(ref original_path) = deleted_file.original_path {
        // Use the original filename if available
        original_path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("recovered_file_{}", deleted_file.id))
    } else {
        // Generate filename based on file type and metadata
        let extension = deleted_file
            .metadata
            .file_extension
            .as_ref()
            .map(|ext| format!(".{}", ext))
            .unwrap_or_else(|| match deleted_file.file_type {
                FileType::RegularFile => ".dat".to_string(),
                FileType::Directory => "".to_string(),
                _ => ".unknown".to_string(),
            });

        format!("recovered_file_{}{}", deleted_file.id, extension)
    };

    output_dir.join(filename)
}

/// Recovery report with detailed results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryReport {
    pub total_files: usize,
    pub recovered_files: usize,
    pub failed_files: usize,
    pub total_bytes_recovered: u64,
    pub output_directory: PathBuf,
    pub recovery_details: Vec<FileRecoveryResult>,
}

/// Individual file recovery result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRecoveryResult {
    pub file_id: u64,
    pub original_path: Option<PathBuf>,
    pub recovered_path: PathBuf,
    pub size: u64,
    pub bytes_recovered: u64,
    pub status: RecoveryStatus,
    pub confidence_score: f32,
}

/// Recovery status for individual files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecoveryStatus {
    Success,
    Failed(String),
}
