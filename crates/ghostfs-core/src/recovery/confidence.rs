/// Confidence scoring algorithm for recovery reliability
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;

use crate::{BlockRange, DeletedFile, FileMetadata, FileSystemType};

/// Context for confidence scoring calculations
#[derive(Debug, Clone)]
pub struct ConfidenceContext {
    pub fs_type: FileSystemType,
    pub scan_time: DateTime<Utc>,
    pub filesystem_integrity: f32, // 0.0-1.0
    pub total_files_found: u32,
    pub device_activity_level: ActivityLevel,
}

#[derive(Debug, Clone)]
pub enum ActivityLevel {
    Low,    // Minimal writes since deletion
    Medium, // Some writes, moderate risk
    High,   // Heavy activity, high overwrite risk
}

/// Calculate confidence score for a deleted file
pub fn calculate_confidence_score(file: &DeletedFile, context: &ConfidenceContext) -> f32 {
    let mut factors = Vec::new();

    // Time-based factors (25% weight)
    factors.push(ConfidenceFactor {
        name: "time_recency",
        score: calculate_time_recency_factor(file.deletion_time, context.scan_time),
        weight: 0.25,
    });

    // Structural integrity factors (35% weight)
    factors.push(ConfidenceFactor {
        name: "metadata_completeness",
        score: calculate_metadata_completeness_factor(&file.metadata),
        weight: 0.15,
    });

    factors.push(ConfidenceFactor {
        name: "data_block_integrity",
        score: calculate_data_block_integrity_factor(&file.data_blocks),
        weight: 0.20,
    });

    // Content validation factors (25% weight)
    factors.push(ConfidenceFactor {
        name: "file_signature_match",
        score: calculate_file_signature_factor(file),
        weight: 0.15,
    });

    factors.push(ConfidenceFactor {
        name: "size_consistency",
        score: calculate_size_consistency_factor(file),
        weight: 0.10,
    });

    // File system specific factors (15% weight)
    factors.push(ConfidenceFactor {
        name: "fs_specific",
        score: calculate_fs_specific_factor(file, context),
        weight: 0.15,
    });

    // Calculate weighted average
    let total_weighted_score: f32 = factors.iter().map(|f| f.score * f.weight).sum();

    let total_weight: f32 = factors.iter().map(|f| f.weight).sum();

    let confidence = if total_weight > 0.0 {
        total_weighted_score / total_weight
    } else {
        0.0
    };

    // Apply global modifiers
    let modified_confidence = apply_global_modifiers(confidence, context);

    // Clamp to valid range
    modified_confidence.clamp(0.0, 1.0)
}

#[derive(Debug)]
struct ConfidenceFactor {
    #[allow(dead_code)]
    name: &'static str,
    score: f32,
    weight: f32,
}

/// Calculate time-based confidence factor
fn calculate_time_recency_factor(
    deletion_time: Option<DateTime<Utc>>,
    scan_time: DateTime<Utc>,
) -> f32 {
    match deletion_time {
        Some(deleted_at) => {
            let time_since_deletion = scan_time.signed_duration_since(deleted_at);

            // More recent deletions have higher confidence
            match time_since_deletion {
                d if d < Duration::hours(1) => 1.0,
                d if d < Duration::hours(24) => 0.9,
                d if d < Duration::days(7) => 0.8,
                d if d < Duration::days(30) => 0.6,
                d if d < Duration::days(90) => 0.4,
                d if d < Duration::days(365) => 0.2,
                _ => 0.1,
            }
        }
        None => 0.3, // Unknown deletion time gets moderate score
    }
}

/// Calculate metadata completeness factor
fn calculate_metadata_completeness_factor(metadata: &FileMetadata) -> f32 {
    let mut completeness_score = 0.0;
    let mut total_fields = 0.0;

    // Check each metadata field
    if metadata.mime_type.is_some() {
        completeness_score += 1.0;
    }
    total_fields += 1.0;

    if metadata.file_extension.is_some() {
        completeness_score += 1.0;
    }
    total_fields += 1.0;

    if metadata.permissions.is_some() {
        completeness_score += 1.0;
    }
    total_fields += 1.0;

    if metadata.created_time.is_some() {
        completeness_score += 1.0;
    }
    total_fields += 1.0;

    if metadata.modified_time.is_some() {
        completeness_score += 1.0;
    }
    total_fields += 1.0;

    if !metadata.extended_attributes.is_empty() {
        completeness_score += 1.0;
    }
    total_fields += 1.0;

    if total_fields > 0.0 {
        completeness_score / total_fields
    } else {
        0.0
    }
}

/// Calculate data block integrity factor
fn calculate_data_block_integrity_factor(data_blocks: &[BlockRange]) -> f32 {
    if data_blocks.is_empty() {
        return 0.0;
    }

    let total_blocks: u64 = data_blocks.iter().map(|range| range.block_count).sum();
    let allocated_blocks: u64 = data_blocks
        .iter()
        .filter(|range| range.is_allocated)
        .map(|range| range.block_count)
        .sum();

    if total_blocks == 0 {
        return 0.0;
    }

    let allocation_ratio = allocated_blocks as f32 / total_blocks as f32;

    // Higher allocation ratio = lower confidence (data may be overwritten)
    // Lower allocation ratio = higher confidence (data likely intact)
    1.0 - allocation_ratio
}

/// Calculate file signature matching factor
fn calculate_file_signature_factor(file: &DeletedFile) -> f32 {
    // If we have MIME type from content analysis, check consistency
    match (&file.metadata.mime_type, &file.metadata.file_extension) {
        (Some(mime), Some(ext)) => {
            if mime_extension_match(mime, ext) {
                0.9
            } else {
                0.3 // Mismatch indicates corruption
            }
        }
        (Some(_), None) => 0.6, // Have mime but no extension
        (None, Some(_)) => 0.5, // Have extension but no mime
        (None, None) => 0.2,    // No type information
    }
}

/// Calculate size consistency factor
fn calculate_size_consistency_factor(file: &DeletedFile) -> f32 {
    let declared_size = file.size;
    let block_size: u64 = file
        .data_blocks
        .iter()
        .map(|range| range.block_count * 4096) // Assume 4KB blocks
        .sum();

    if declared_size == 0 && block_size == 0 {
        return 0.5; // Empty file
    }

    if declared_size == 0 || block_size == 0 {
        return 0.2; // Inconsistent
    }

    // Perfect match = 1.0, decreasing as ratio gets worse
    if declared_size > block_size {
        block_size as f32 / declared_size as f32
    } else {
        declared_size as f32 / block_size as f32
    }
}

/// Calculate file system specific factor
fn calculate_fs_specific_factor(file: &DeletedFile, context: &ConfidenceContext) -> f32 {
    match context.fs_type {
        FileSystemType::Xfs => {
            // XFS specific checks
            calculate_xfs_specific_factor(file, context)
        }
        FileSystemType::Btrfs => {
            // Btrfs specific checks
            calculate_btrfs_specific_factor(file, context)
        }
        FileSystemType::ExFat => {
            // exFAT specific checks
            calculate_exfat_specific_factor(file, context)
        }
    }
}

fn calculate_xfs_specific_factor(file: &DeletedFile, _context: &ConfidenceContext) -> f32 {
    // If no XFS metadata available, return neutral score
    let Some(crate::FsSpecificMetadata::Xfs(ref xfs_meta)) = file.fs_metadata else {
        return 0.5; // No metadata = neutral confidence
    };

    // Calculate three sub-factors
    let ag_score = calculate_xfs_ag_validity(xfs_meta);
    let extent_score = calculate_xfs_extent_integrity(file, xfs_meta);
    let inode_score = calculate_xfs_inode_consistency(file, xfs_meta);

    // Average of three factors
    (ag_score + extent_score + inode_score) / 3.0
}

/// Calculate XFS allocation group structure validity
fn calculate_xfs_ag_validity(meta: &crate::XfsFileMetadata) -> f32 {
    let mut score = 0.0;

    // Factor 1: Generation counter is reasonable (not corrupted)
    // XFS generation counters typically don't exceed millions
    if meta.inode_generation > 0 && meta.inode_generation < 10_000_000 {
        score += 0.33;
    }

    // Factor 2: AG inode number is reasonable
    // Most filesystems have millions of inodes per AG, not billions
    if meta.ag_inode_number < 100_000_000 {
        score += 0.33;
    }

    // Factor 3: Link count before deletion was reasonable
    // Files rarely have more than 1000 hard links
    if meta.last_link_count > 0 && meta.last_link_count < 1000 {
        score += 0.34;
    }

    score
}

/// Calculate XFS extent integrity
fn calculate_xfs_extent_integrity(file: &DeletedFile, meta: &crate::XfsFileMetadata) -> f32 {
    let mut score = 0.0;

    // Factor 1: Extent format matches file size
    let format_appropriate = match meta.extent_format {
        crate::XfsExtentFormat::Local => file.size < 156, // Fits in inode
        crate::XfsExtentFormat::Extents => meta.extent_count <= 20, // Reasonable for direct list
        crate::XfsExtentFormat::Btree => meta.extent_count > 10, // Needs btree
    };
    if format_appropriate {
        score += 0.4;
    }

    // Factor 2: Extent alignment (aligned extents suggest intact structure)
    if meta.is_aligned {
        score += 0.3;
    }

    // Factor 3: Extent count is reasonable for file size
    if meta.extent_count > 0 {
        // Average extent size should be reasonable (4KB to 10MB)
        let avg_extent_size = file.size / meta.extent_count as u64;
        if (4096..=10_485_760).contains(&avg_extent_size) {
            score += 0.3;
        }
    } else if file.size == 0 {
        // Empty files with no extents are valid
        score += 0.3;
    }

    score
}

/// Calculate XFS inode state consistency
fn calculate_xfs_inode_consistency(file: &DeletedFile, meta: &crate::XfsFileMetadata) -> f32 {
    let mut score = 0.0;

    // Factor 1: File size is reasonable
    // Files larger than 10TB are uncommon and might indicate corruption
    if file.size < 10_995_116_277_760 {
        // 10 TB
        score += 0.4;
    }

    // Factor 2: Has data blocks if size > 0
    let has_blocks = !file.data_blocks.is_empty();
    if (file.size > 0 && has_blocks) || (file.size == 0 && !has_blocks) {
        score += 0.3;
    }

    // Factor 3: Extent count matches data blocks
    let expected_ranges = file.data_blocks.len() as u32;
    if meta.extent_count == expected_ranges || meta.extent_count + 1 == expected_ranges {
        score += 0.3;
    }

    score
}

/// Helper: Check if extents overlap (indicates corruption)
#[allow(dead_code)]
fn check_extent_overlaps(blocks: &[crate::BlockRange]) -> bool {
    for i in 0..blocks.len() {
        for j in (i + 1)..blocks.len() {
            let a = &blocks[i];
            let b = &blocks[j];

            let a_end = a.start_block + a.block_count;
            let b_end = b.start_block + b.block_count;

            // Check for overlap
            if (a.start_block < b_end) && (b.start_block < a_end) {
                return true;
            }
        }
    }
    false
}

fn calculate_btrfs_specific_factor(file: &DeletedFile, _context: &ConfidenceContext) -> f32 {
    // If no Btrfs metadata available, return neutral score
    let Some(crate::FsSpecificMetadata::Btrfs(ref btrfs_meta)) = file.fs_metadata else {
        return 0.5; // No metadata = neutral confidence
    };

    // Calculate three sub-factors with Btrfs-specific weights
    let gen_score = calculate_btrfs_generation_validity(btrfs_meta);
    let checksum_score = calculate_btrfs_checksum_score(btrfs_meta);
    let cow_score = calculate_btrfs_cow_integrity(btrfs_meta);

    // Weighted: checksum is most important for Btrfs
    gen_score * 0.4 + checksum_score * 0.4 + cow_score * 0.2
}

/// Calculate Btrfs generation counter validity
fn calculate_btrfs_generation_validity(meta: &crate::BtrfsFileMetadata) -> f32 {
    let mut score = 0.0;

    // Factor 1: Generation is non-zero (not corrupted)
    if meta.generation > 0 {
        score += 0.4;
    }

    // Factor 2: Generation is reasonable (not absurdly high)
    // Btrfs generation counters typically don't exceed billions
    if meta.generation < 1_000_000_000 {
        score += 0.3;
    }

    // Factor 3: Transaction ID is consistent with generation
    // transid should generally be <= generation
    if meta.transid > 0 && meta.transid <= meta.generation {
        score += 0.3;
    }

    score
}

/// Calculate Btrfs checksum validation score
fn calculate_btrfs_checksum_score(meta: &crate::BtrfsFileMetadata) -> f32 {
    // Checksum validation is critical for Btrfs
    if meta.checksum_valid {
        1.0 // Perfect score - data integrity verified
    } else {
        0.0 // Major issue - likely corrupted
    }
}

/// Calculate Btrfs COW structure integrity
fn calculate_btrfs_cow_integrity(meta: &crate::BtrfsFileMetadata) -> f32 {
    let mut score = 0.0;

    // Factor 1: Extent reference counts are reasonable
    if !meta.extent_refs.is_empty() {
        let all_refs_valid = meta.extent_refs.iter().all(|&r| r > 0 && r < 10000); // Reasonable refcount range
        if all_refs_valid {
            score += 0.5;
        }
    } else {
        // No refs might be ok for small files
        score += 0.2;
    }

    // Factor 2: File in snapshot increases recovery confidence
    if meta.in_snapshot {
        score += 0.3;
    }

    // Factor 3: COW extent count is reasonable
    if meta.cow_extent_count > 0 && meta.cow_extent_count < 10000 {
        score += 0.2;
    }

    score
}

fn calculate_exfat_specific_factor(file: &DeletedFile, _context: &ConfidenceContext) -> f32 {
    // If no exFAT metadata available, return neutral score
    let Some(crate::FsSpecificMetadata::ExFat(ref exfat_meta)) = file.fs_metadata else {
        return 0.5; // No metadata = neutral confidence
    };

    // Calculate three sub-factors with exFAT-specific weights
    let chain_score = calculate_exfat_chain_validity(exfat_meta);
    let entry_score = calculate_exfat_entry_consistency(exfat_meta);
    let pattern_score = calculate_exfat_cluster_patterns(file, exfat_meta);

    // Weighted: chain validity is most important
    chain_score * 0.5 + entry_score * 0.3 + pattern_score * 0.2
}

/// Calculate exFAT FAT chain validity
fn calculate_exfat_chain_validity(meta: &crate::ExFatFileMetadata) -> f32 {
    let mut score = 0.0;

    // Factor 1: First cluster is valid (>= 2, clusters 0-1 are reserved)
    if meta.first_cluster >= 2 {
        score += 0.3;
    }

    // Factor 2: Chain integrity flag
    if meta.chain_valid {
        score += 0.5;
    }

    // Factor 3: Chain length is reasonable
    let chain_len = meta.cluster_chain.len();
    if chain_len > 0 && chain_len < 1_000_000 {
        score += 0.2;
    }

    score
}

/// Calculate exFAT directory entry consistency
fn calculate_exfat_entry_consistency(meta: &crate::ExFatFileMetadata) -> f32 {
    let mut score = 0.0;

    // Factor 1: Checksum is present (non-zero means validated)
    if meta.checksum != 0 {
        score += 0.5;
    }

    // Factor 2: Entry count is reasonable
    // exFAT: 1 File Entry + 1 Stream Extension + 1-17 File Name entries
    if meta.entry_count >= 2 && meta.entry_count <= 19 {
        score += 0.3;
    }

    // Factor 3: UTF-16 filename is valid
    if meta.utf16_valid {
        score += 0.2;
    }

    score
}

/// Calculate exFAT cluster usage patterns
fn calculate_exfat_cluster_patterns(file: &DeletedFile, meta: &crate::ExFatFileMetadata) -> f32 {
    let mut score = 0.0;

    // Factor 1: No bad cluster markers (0xFFFFFFF7)
    if !meta.cluster_chain.contains(&0xFFFFFFF7) {
        score += 0.5;
    }

    // Factor 2: All clusters in chain are >= 2 (valid data clusters)
    let all_valid = meta.cluster_chain.iter().all(|&c| c >= 2);
    if all_valid {
        score += 0.3;
    }

    // Factor 3: File has data blocks if size > 0
    if (file.size > 0 && !file.data_blocks.is_empty())
        || (file.size == 0 && file.data_blocks.is_empty())
    {
        score += 0.2;
    }

    score
}

/// Apply global modifiers based on overall context
fn apply_global_modifiers(base_confidence: f32, context: &ConfidenceContext) -> f32 {
    let mut modified = base_confidence;

    // Filesystem integrity modifier
    modified *= context.filesystem_integrity;

    // Device activity level modifier
    let activity_modifier = match context.device_activity_level {
        ActivityLevel::Low => 1.0,    // No penalty
        ActivityLevel::Medium => 0.8, // 20% penalty
        ActivityLevel::High => 0.6,   // 40% penalty
    };
    modified *= activity_modifier;

    // Volume of files found (too many might indicate scan issues)
    let volume_modifier = if context.total_files_found > 10000 {
        0.9 // Slight penalty for very large recoveries
    } else {
        1.0
    };
    modified *= volume_modifier;

    modified
}

/// Check if MIME type matches file extension
fn mime_extension_match(mime_type: &str, extension: &str) -> bool {
    let mime_to_ext: HashMap<&str, Vec<&str>> = [
        ("image/jpeg", vec!["jpg", "jpeg"]),
        ("image/png", vec!["png"]),
        ("image/gif", vec!["gif"]),
        ("application/pdf", vec!["pdf"]),
        ("text/plain", vec!["txt"]),
        ("application/zip", vec!["zip"]),
        ("video/mp4", vec!["mp4"]),
        ("audio/mp3", vec!["mp3"]),
        ("application/x-executable", vec!["exe", "bin"]),
    ]
    .iter()
    .cloned()
    .collect();

    let ext_lower = extension.to_lowercase();
    mime_to_ext
        .get(mime_type)
        .map(|exts| exts.contains(&ext_lower.as_str()))
        .unwrap_or(false)
}

/// Generate a detailed confidence report
pub fn generate_confidence_report(
    file: &DeletedFile,
    context: &ConfidenceContext,
) -> ConfidenceReport {
    let factors = vec![
        (
            "Time Recency",
            calculate_time_recency_factor(file.deletion_time, context.scan_time),
        ),
        (
            "Metadata Completeness",
            calculate_metadata_completeness_factor(&file.metadata),
        ),
        (
            "Data Block Integrity",
            calculate_data_block_integrity_factor(&file.data_blocks),
        ),
        (
            "File Signature Match",
            calculate_file_signature_factor(file),
        ),
        ("Size Consistency", calculate_size_consistency_factor(file)),
        ("FS Specific", calculate_fs_specific_factor(file, context)),
    ];

    let overall_confidence = calculate_confidence_score(file, context);

    ConfidenceReport {
        overall_confidence,
        factors,
        recommendation: get_recovery_recommendation(overall_confidence),
    }
}

#[derive(Debug)]
pub struct ConfidenceReport {
    pub overall_confidence: f32,
    pub factors: Vec<(&'static str, f32)>,
    pub recommendation: RecoveryRecommendation,
}

#[derive(Debug)]
pub enum RecoveryRecommendation {
    HighConfidence(String),
    MediumConfidence(String),
    LowConfidence(String),
    NotRecommended(String),
}

fn get_recovery_recommendation(confidence: f32) -> RecoveryRecommendation {
    match confidence {
        c if c >= 0.8 => RecoveryRecommendation::HighConfidence(
            "Excellent recovery prospects. File is likely fully recoverable.".to_string()
        ),
        c if c >= 0.6 => RecoveryRecommendation::MediumConfidence(
            "Good recovery prospects. File should be mostly recoverable with possible minor corruption.".to_string()
        ),
        c if c >= 0.4 => RecoveryRecommendation::LowConfidence(
            "Fair recovery prospects. File may be partially recoverable or have significant corruption.".to_string()
        ),
        _ => RecoveryRecommendation::NotRecommended(
            "Poor recovery prospects. File is likely heavily corrupted or unrecoverable.".to_string()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DeletedFile, FileMetadata, FileType};
    use std::collections::HashMap;

    #[test]
    fn test_confidence_calculation() {
        let context = ConfidenceContext {
            fs_type: FileSystemType::Xfs,
            scan_time: Utc::now(),
            filesystem_integrity: 0.9,
            total_files_found: 100,
            device_activity_level: ActivityLevel::Low,
        };

        let metadata = FileMetadata {
            mime_type: Some("image/jpeg".to_string()),
            file_extension: Some("jpg".to_string()),
            permissions: Some(0o644),
            owner_uid: Some(1000),
            owner_gid: Some(1000),
            created_time: Some(Utc::now() - Duration::days(1)),
            modified_time: Some(Utc::now() - Duration::days(1)),
            accessed_time: Some(Utc::now()),
            extended_attributes: HashMap::new(),
        };

        let file = DeletedFile {
            id: 1,
            inode_or_cluster: 12345,
            original_path: Some("/home/user/photo.jpg".into()),
            size: 1024000,
            deletion_time: Some(Utc::now() - Duration::hours(2)),
            confidence_score: 0.0, // Will be calculated
            file_type: FileType::RegularFile,
            data_blocks: vec![BlockRange {
                start_block: 100,
                block_count: 250,
                is_allocated: false,
            }],
            is_recoverable: true,
            metadata,
            fs_metadata: None,
        };

        let confidence = calculate_confidence_score(&file, &context);
        assert!(
            confidence > 0.5,
            "Should have reasonable confidence for recent file"
        );
        assert!(confidence <= 1.0, "Confidence should not exceed 1.0");
    }

    #[test]
    fn test_mime_extension_matching() {
        assert!(mime_extension_match("image/jpeg", "jpg"));
        assert!(mime_extension_match("image/jpeg", "jpeg"));
        assert!(!mime_extension_match("image/jpeg", "png"));
        assert!(!mime_extension_match("application/pdf", "txt"));
    }

    #[test]
    fn test_btrfs_confidence_with_valid_metadata() {
        let context = ConfidenceContext {
            fs_type: FileSystemType::Btrfs,
            scan_time: Utc::now(),
            filesystem_integrity: 0.9,
            total_files_found: 100,
            device_activity_level: ActivityLevel::Low,
        };

        let btrfs_meta = crate::BtrfsFileMetadata {
            generation: 1000,
            transid: 950,
            checksum_valid: true,
            extent_refs: vec![1, 2],
            in_snapshot: true,
            cow_extent_count: 5,
            tree_level: 0,
        };

        let file = DeletedFile {
            id: 1,
            inode_or_cluster: 12345,
            original_path: Some("/data/document.txt".into()),
            size: 50000,
            deletion_time: Some(Utc::now() - Duration::hours(1)),
            confidence_score: 0.0,
            file_type: FileType::RegularFile,
            data_blocks: vec![BlockRange {
                start_block: 100,
                block_count: 50,
                is_allocated: false,
            }],
            is_recoverable: true,
            metadata: FileMetadata {
                mime_type: Some("text/plain".to_string()),
                file_extension: Some("txt".to_string()),
                permissions: None,
                owner_uid: None,
                owner_gid: None,
                created_time: Some(Utc::now() - Duration::days(10)),
                modified_time: Some(Utc::now() - Duration::days(2)),
                accessed_time: Some(Utc::now() - Duration::hours(1)),
                extended_attributes: HashMap::new(),
            },
            fs_metadata: Some(crate::FsSpecificMetadata::Btrfs(btrfs_meta)),
        };

        let confidence = calculate_confidence_score(&file, &context);

        // Should have high confidence: valid checksum, in snapshot, recent deletion
        assert!(
            confidence > 0.7,
            "Btrfs file with valid metadata should have high confidence, got {}",
            confidence
        );
        assert!(confidence <= 1.0, "Confidence should not exceed 1.0");
    }

    #[test]
    fn test_btrfs_confidence_with_invalid_checksum() {
        let context = ConfidenceContext {
            fs_type: FileSystemType::Btrfs,
            scan_time: Utc::now(),
            filesystem_integrity: 0.7,
            total_files_found: 100,
            device_activity_level: ActivityLevel::Medium,
        };

        let btrfs_meta = crate::BtrfsFileMetadata {
            generation: 1000,
            transid: 950,
            checksum_valid: false, // Invalid checksum
            extent_refs: vec![1],
            in_snapshot: false,
            cow_extent_count: 3,
            tree_level: 0,
        };

        let file = DeletedFile {
            id: 1,
            inode_or_cluster: 12345,
            original_path: Some("/data/corrupted.dat".into()),
            size: 10000,
            deletion_time: Some(Utc::now() - Duration::hours(1)),
            confidence_score: 0.0,
            file_type: FileType::RegularFile,
            data_blocks: vec![BlockRange {
                start_block: 100,
                block_count: 10,
                is_allocated: false,
            }],
            is_recoverable: true,
            metadata: FileMetadata {
                mime_type: None,
                file_extension: Some("dat".to_string()),
                permissions: None,
                owner_uid: None,
                owner_gid: None,
                created_time: None,
                modified_time: None,
                accessed_time: None,
                extended_attributes: HashMap::new(),
            },
            fs_metadata: Some(crate::FsSpecificMetadata::Btrfs(btrfs_meta)),
        };

        let confidence = calculate_confidence_score(&file, &context);

        // Should have lower confidence due to invalid checksum
        assert!(
            confidence < 0.6,
            "Btrfs file with invalid checksum should have lower confidence, got {}",
            confidence
        );
    }

    #[test]
    fn test_exfat_confidence_with_valid_chain() {
        let context = ConfidenceContext {
            fs_type: FileSystemType::ExFat,
            scan_time: Utc::now(),
            filesystem_integrity: 0.85,
            total_files_found: 50,
            device_activity_level: ActivityLevel::Low,
        };

        let exfat_meta = crate::ExFatFileMetadata {
            first_cluster: 100,
            cluster_chain: vec![100, 101, 102, 103],
            chain_valid: true,
            checksum: 0x1234,
            entry_count: 3, // File + Stream + Name
            utf16_valid: true,
            attributes: 0x20, // Archive bit
        };

        let file = DeletedFile {
            id: 1,
            inode_or_cluster: 100,
            original_path: Some("/photos/image.jpg".into()),
            size: 204800,
            deletion_time: Some(Utc::now() - Duration::hours(3)),
            confidence_score: 0.0,
            file_type: FileType::RegularFile,
            data_blocks: vec![BlockRange {
                start_block: 100,
                block_count: 4,
                is_allocated: false,
            }],
            is_recoverable: true,
            metadata: FileMetadata {
                mime_type: Some("image/jpeg".to_string()),
                file_extension: Some("jpg".to_string()),
                permissions: None,
                owner_uid: None,
                owner_gid: None,
                created_time: Some(Utc::now() - Duration::days(5)),
                modified_time: Some(Utc::now() - Duration::days(1)),
                accessed_time: Some(Utc::now() - Duration::hours(3)),
                extended_attributes: HashMap::new(),
            },
            fs_metadata: Some(crate::FsSpecificMetadata::ExFat(exfat_meta)),
        };

        let confidence = calculate_confidence_score(&file, &context);

        // Should have good confidence: valid chain, checksum, UTF-16 filename, recent deletion
        // Slightly lower threshold due to other factors (time, metadata completeness, etc.)
        assert!(
            confidence > 0.65,
            "exFAT file with valid metadata should have good confidence, got {}",
            confidence
        );
        assert!(confidence <= 1.0, "Confidence should not exceed 1.0");
    }

    #[test]
    fn test_exfat_confidence_orphaned_cluster() {
        let context = ConfidenceContext {
            fs_type: FileSystemType::ExFat,
            scan_time: Utc::now(),
            filesystem_integrity: 0.6,
            total_files_found: 50,
            device_activity_level: ActivityLevel::High,
        };

        let exfat_meta = crate::ExFatFileMetadata {
            first_cluster: 200,
            cluster_chain: vec![200],
            chain_valid: true,
            checksum: 0,        // No checksum (orphaned)
            entry_count: 0,     // No directory entry
            utf16_valid: false, // No filename
            attributes: 0,
        };

        let file = DeletedFile {
            id: 1,
            inode_or_cluster: 200,
            original_path: Some("orphan_200.dat".into()),
            size: 4096,
            deletion_time: None,
            confidence_score: 0.0,
            file_type: FileType::RegularFile,
            data_blocks: vec![BlockRange {
                start_block: 200,
                block_count: 1,
                is_allocated: false,
            }],
            is_recoverable: true,
            metadata: FileMetadata {
                mime_type: None,
                file_extension: Some("dat".to_string()),
                permissions: None,
                owner_uid: None,
                owner_gid: None,
                created_time: None,
                modified_time: None,
                accessed_time: None,
                extended_attributes: HashMap::new(),
            },
            fs_metadata: Some(crate::FsSpecificMetadata::ExFat(exfat_meta)),
        };

        let confidence = calculate_confidence_score(&file, &context);

        // Should have lower confidence: no directory entry metadata, no deletion time, high activity
        assert!(
            confidence < 0.7,
            "exFAT orphaned cluster should have medium/low confidence, got {}",
            confidence
        );
        assert!(
            confidence > 0.2,
            "Should still have some confidence from valid cluster number"
        );
    }
}
