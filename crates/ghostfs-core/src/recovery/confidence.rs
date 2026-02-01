/// Confidence scoring algorithm for recovery reliability
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;

use crate::{DeletedFile, FileSystemType, FileMetadata, BlockRange};

/// Context for confidence scoring calculations
#[derive(Debug, Clone)]
pub struct ConfidenceContext {
    pub fs_type: FileSystemType,
    pub scan_time: DateTime<Utc>,
    pub filesystem_integrity: f32,  // 0.0-1.0
    pub total_files_found: u32,
    pub device_activity_level: ActivityLevel,
}

#[derive(Debug, Clone)]
pub enum ActivityLevel {
    Low,     // Minimal writes since deletion
    Medium,  // Some writes, moderate risk
    High,    // Heavy activity, high overwrite risk
}

/// Calculate confidence score for a deleted file
pub fn calculate_confidence_score(
    file: &DeletedFile,
    context: &ConfidenceContext
) -> f32 {
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
    let total_weighted_score: f32 = factors.iter()
        .map(|f| f.score * f.weight)
        .sum();
    
    let total_weight: f32 = factors.iter()
        .map(|f| f.weight)
        .sum();
    
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
fn calculate_time_recency_factor(deletion_time: Option<DateTime<Utc>>, scan_time: DateTime<Utc>) -> f32 {
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
    let allocated_blocks: u64 = data_blocks.iter()
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
    let block_size: u64 = file.data_blocks.iter()
        .map(|range| range.block_count * 4096) // Assume 4KB blocks
        .sum();
    
    if declared_size == 0 && block_size == 0 {
        return 0.5; // Empty file
    }
    
    if declared_size == 0 || block_size == 0 {
        return 0.2; // Inconsistent
    }
    
    let ratio = if declared_size > block_size {
        block_size as f32 / declared_size as f32
    } else {
        declared_size as f32 / block_size as f32
    };
    
    // Perfect match = 1.0, decreasing as ratio gets worse
    ratio
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

fn calculate_xfs_specific_factor(_file: &DeletedFile, _context: &ConfidenceContext) -> f32 {
    // TODO: XFS-specific confidence factors:
    // - Inode allocation consistency
    // - Allocation group integrity
    // - B+tree structure validation
    0.5 // Placeholder
}

fn calculate_btrfs_specific_factor(_file: &DeletedFile, _context: &ConfidenceContext) -> f32 {
    // TODO: Btrfs-specific confidence factors:
    // - Tree node integrity
    // - Snapshot presence/consistency
    // - COW structure validation
    // - Checksum validation
    0.5 // Placeholder
}

fn calculate_exfat_specific_factor(_file: &DeletedFile, _context: &ConfidenceContext) -> f32 {
    // TODO: exFAT-specific confidence factors:
    // - Directory entry completeness
    // - Cluster chain validity
    // - FAT consistency
    0.5 // Placeholder
}

/// Apply global modifiers based on overall context
fn apply_global_modifiers(base_confidence: f32, context: &ConfidenceContext) -> f32 {
    let mut modified = base_confidence;
    
    // Filesystem integrity modifier
    modified *= context.filesystem_integrity;
    
    // Device activity level modifier
    let activity_modifier = match context.device_activity_level {
        ActivityLevel::Low => 1.0,     // No penalty
        ActivityLevel::Medium => 0.8,  // 20% penalty
        ActivityLevel::High => 0.6,    // 40% penalty
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
    ].iter().cloned().collect();
    
    let ext_lower = extension.to_lowercase();
    mime_to_ext.get(mime_type)
        .map(|exts| exts.contains(&ext_lower.as_str()))
        .unwrap_or(false)
}

/// Generate a detailed confidence report
pub fn generate_confidence_report(file: &DeletedFile, context: &ConfidenceContext) -> ConfidenceReport {
    let factors = vec![
        ("Time Recency", calculate_time_recency_factor(file.deletion_time, context.scan_time)),
        ("Metadata Completeness", calculate_metadata_completeness_factor(&file.metadata)),
        ("Data Block Integrity", calculate_data_block_integrity_factor(&file.data_blocks)),
        ("File Signature Match", calculate_file_signature_factor(file)),
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
    use crate::{DeletedFile, FileType, FileMetadata};
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
            data_blocks: vec![
                BlockRange { start_block: 100, block_count: 250, is_allocated: false }
            ],
            is_recoverable: true,
            metadata,
        };

        let confidence = calculate_confidence_score(&file, &context);
        assert!(confidence > 0.5, "Should have reasonable confidence for recent file");
        assert!(confidence <= 1.0, "Confidence should not exceed 1.0");
    }

    #[test]
    fn test_mime_extension_matching() {
        assert!(mime_extension_match("image/jpeg", "jpg"));
        assert!(mime_extension_match("image/jpeg", "jpeg"));
        assert!(!mime_extension_match("image/jpeg", "png"));
        assert!(!mime_extension_match("application/pdf", "txt"));
    }
}
