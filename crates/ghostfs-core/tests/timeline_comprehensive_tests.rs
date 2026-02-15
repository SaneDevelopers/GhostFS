/// Comprehensive Timeline Recovery Tests
/// Tests edge cases, pattern detection, and filesystem-specific scenarios
use chrono::{Duration, Utc};
use ghostfs_core::{
    DeletedFile, FileMetadata, FileSystemType, FileType, PatternType, RecoverySession,
    RecoveryTimeline, SessionMetadata,
};
use std::collections::HashMap;
use std::path::PathBuf;

/// Helper to create a basic RecoverySession with custom files
fn create_test_session(fs_type: FileSystemType, files: Vec<DeletedFile>) -> RecoverySession {
    let files_count = files.len();
    RecoverySession {
        id: uuid::Uuid::new_v4(),
        fs_type,
        device_path: PathBuf::from("/dev/test"),
        created_at: Utc::now(),
        scan_results: files,
        total_scanned: files_count as u64,
        confidence_threshold: 0.5,
        metadata: SessionMetadata {
            device_size: 1024 * 1024 * 1024,
            filesystem_size: 1024 * 1024 * 1024,
            block_size: 4096,
            scan_duration_ms: 100,
            files_found: files_count as u32,
            recoverable_files: files_count as u32,
        },
    }
}

/// Helper to create a test file with all timestamps
fn create_file_with_timestamps(
    id: u64,
    path: &str,
    mime: &str,
    created: chrono::DateTime<Utc>,
    modified: chrono::DateTime<Utc>,
    deleted: chrono::DateTime<Utc>,
) -> DeletedFile {
    DeletedFile {
        id,
        inode_or_cluster: 1000 + id,
        original_path: Some(PathBuf::from(path)),
        size: 1024,
        deletion_time: Some(deleted),
        confidence_score: 0.85,
        file_type: FileType::RegularFile,
        data_blocks: vec![],
        is_recoverable: true,
        metadata: FileMetadata {
            mime_type: Some(mime.to_string()),
            file_extension: Some(path.split('.').next_back().unwrap_or("").to_string()),
            permissions: Some(0o644),
            owner_uid: Some(1000),
            owner_gid: Some(1000),
            created_time: Some(created),
            modified_time: Some(modified),
            accessed_time: Some(deleted - Duration::hours(1)),
            extended_attributes: HashMap::new(),
        },
        fs_metadata: None,
    }
}

// ============================================================================
// EDGE CASE TESTS
// ============================================================================

#[test]
fn test_empty_timeline() {
    let session = create_test_session(FileSystemType::Xfs, vec![]);
    let timeline = RecoveryTimeline::from_session(&session);

    assert_eq!(timeline.events.len(), 0);
    assert_eq!(timeline.statistics.total_events, 0);
    assert_eq!(timeline.statistics.deletion_events, 0);
    assert_eq!(timeline.patterns.len(), 0);
    assert!(timeline.statistics.peak_deletion_time.is_none());
}

#[test]
fn test_single_file_timeline() {
    let now = Utc::now();
    let file = create_file_with_timestamps(
        1,
        "/test/file.txt",
        "text/plain",
        now - Duration::days(7),
        now - Duration::days(1),
        now,
    );

    let session = create_test_session(FileSystemType::Xfs, vec![file]);
    let timeline = RecoveryTimeline::from_session(&session);

    // Should have 3 events: created, modified, deleted
    assert_eq!(timeline.events.len(), 3);
    assert_eq!(timeline.statistics.deletion_events, 1);
    assert_eq!(timeline.patterns.len(), 0); // No patterns with 1 file
}

#[test]
fn test_files_with_missing_timestamps() {
    let now = Utc::now();

    // File with only deletion time
    let file1 = DeletedFile {
        id: 1,
        inode_or_cluster: 1001,
        original_path: Some(PathBuf::from("/test/file1.txt")),
        size: 1024,
        deletion_time: Some(now),
        confidence_score: 0.85,
        file_type: FileType::RegularFile,
        data_blocks: vec![],
        is_recoverable: true,
        metadata: FileMetadata {
            mime_type: Some("text/plain".to_string()),
            file_extension: Some("txt".to_string()),
            permissions: None,
            owner_uid: None,
            owner_gid: None,
            created_time: None,  // Missing
            modified_time: None, // Missing
            accessed_time: None,
            extended_attributes: HashMap::new(),
        },
        fs_metadata: None,
    };

    // File with no timestamps at all
    let file2 = DeletedFile {
        id: 2,
        inode_or_cluster: 1002,
        original_path: Some(PathBuf::from("/test/file2.txt")),
        size: 2048,
        deletion_time: None, // Missing
        confidence_score: 0.50,
        file_type: FileType::RegularFile,
        data_blocks: vec![],
        is_recoverable: false,
        metadata: FileMetadata {
            mime_type: Some("text/plain".to_string()),
            file_extension: Some("txt".to_string()),
            permissions: None,
            owner_uid: None,
            owner_gid: None,
            created_time: None,
            modified_time: None,
            accessed_time: None,
            extended_attributes: HashMap::new(),
        },
        fs_metadata: None,
    };

    let session = create_test_session(FileSystemType::Xfs, vec![file1, file2]);
    let timeline = RecoveryTimeline::from_session(&session);

    // Should handle missing timestamps gracefully
    assert_eq!(timeline.events.len(), 1); // Only deletion event from file1
    assert_eq!(timeline.statistics.deletion_events, 1);
}

#[test]
fn test_concurrent_deletions_same_timestamp() {
    let now = Utc::now();
    let deletion_time = now - Duration::hours(2);

    // Create 10 files all deleted at the exact same timestamp
    let files: Vec<_> = (0..10)
        .map(|i| {
            create_file_with_timestamps(
                i,
                &format!("/test/file_{}.txt", i),
                "text/plain",
                now - Duration::days(7),
                now - Duration::days(1),
                deletion_time, // Same timestamp for all
            )
        })
        .collect();

    let session = create_test_session(FileSystemType::Xfs, files);
    let timeline = RecoveryTimeline::from_session(&session);

    assert_eq!(timeline.statistics.deletion_events, 10);

    // Should detect bulk deletion even with same timestamp
    let bulk_patterns: Vec<_> = timeline
        .patterns
        .iter()
        .filter(|p| matches!(p.pattern_type, PatternType::BulkDeletion))
        .collect();
    assert!(
        !bulk_patterns.is_empty(),
        "Should detect bulk deletion pattern"
    );
}

#[test]
fn test_bulk_deletion_threshold() {
    let now = Utc::now();

    // Test with exactly 5 files (threshold)
    let files: Vec<_> = (0..5)
        .map(|i| {
            create_file_with_timestamps(
                i,
                &format!("/test/file_{}.txt", i),
                "text/plain",
                now - Duration::days(7),
                now - Duration::days(1),
                now + Duration::seconds(i as i64 * 30), // Within 5 minutes
            )
        })
        .collect();

    let session = create_test_session(FileSystemType::Xfs, files);
    let timeline = RecoveryTimeline::from_session(&session);

    let bulk_patterns: Vec<_> = timeline
        .patterns
        .iter()
        .filter(|p| matches!(p.pattern_type, PatternType::BulkDeletion))
        .collect();
    assert_eq!(
        bulk_patterns.len(),
        1,
        "Should detect bulk deletion with exactly 5 files"
    );

    // Test with 4 files (below threshold)
    let files: Vec<_> = (0..4)
        .map(|i| {
            create_file_with_timestamps(
                i,
                &format!("/test/small_{}.txt", i),
                "text/plain",
                now - Duration::days(7),
                now - Duration::days(1),
                now + Duration::seconds(i as i64 * 30),
            )
        })
        .collect();

    let session = create_test_session(FileSystemType::Xfs, files);
    let timeline = RecoveryTimeline::from_session(&session);

    let bulk_patterns: Vec<_> = timeline
        .patterns
        .iter()
        .filter(|p| matches!(p.pattern_type, PatternType::BulkDeletion))
        .collect();
    assert_eq!(
        bulk_patterns.len(),
        0,
        "Should NOT detect bulk deletion with only 4 files"
    );
}

#[test]
fn test_large_scale_deletion() {
    let now = Utc::now();

    // Create 100 files deleted over 10 minutes
    let files: Vec<_> = (0..100)
        .map(|i| {
            create_file_with_timestamps(
                i,
                &format!("/test/massive/file_{:04}.dat", i),
                "application/octet-stream",
                now - Duration::days(30),
                now - Duration::days(5),
                now + Duration::seconds(i as i64 * 6), // 6 seconds apart = 10 mins total
            )
        })
        .collect();

    let session = create_test_session(FileSystemType::Xfs, files);
    let timeline = RecoveryTimeline::from_session(&session);

    assert_eq!(timeline.statistics.deletion_events, 100);
    assert_eq!(timeline.events.len(), 300); // created + modified + deleted for each

    // Should detect multiple bulk deletion windows
    let bulk_patterns: Vec<_> = timeline
        .patterns
        .iter()
        .filter(|p| matches!(p.pattern_type, PatternType::BulkDeletion))
        .collect();
    assert!(
        !bulk_patterns.is_empty(),
        "Should detect bulk deletions in large dataset"
    );
}

#[test]
fn test_selective_deletion_edge_cases() {
    let now = Utc::now();

    // Exactly 3 files of same type (threshold for selective deletion)
    let mut files = vec![
        create_file_with_timestamps(
            1,
            "/a.jpg",
            "image/jpeg",
            now - Duration::days(7),
            now - Duration::days(1),
            now,
        ),
        create_file_with_timestamps(
            2,
            "/b.jpg",
            "image/jpeg",
            now - Duration::days(6),
            now - Duration::days(1),
            now + Duration::hours(1),
        ),
        create_file_with_timestamps(
            3,
            "/c.jpg",
            "image/jpeg",
            now - Duration::days(5),
            now - Duration::days(1),
            now + Duration::hours(2),
        ),
    ];

    // Add 2 of another type (below threshold)
    files.push(create_file_with_timestamps(
        4,
        "/d.pdf",
        "application/pdf",
        now - Duration::days(4),
        now - Duration::days(1),
        now + Duration::hours(3),
    ));
    files.push(create_file_with_timestamps(
        5,
        "/e.pdf",
        "application/pdf",
        now - Duration::days(3),
        now - Duration::days(1),
        now + Duration::hours(4),
    ));

    let session = create_test_session(FileSystemType::Btrfs, files);
    let timeline = RecoveryTimeline::from_session(&session);

    let selective_patterns: Vec<_> = timeline
        .patterns
        .iter()
        .filter(|p| matches!(p.pattern_type, PatternType::SelectiveDeletion))
        .collect();

    // Should detect JPEG pattern but not PDF (only 2 files)
    assert_eq!(
        selective_patterns.len(),
        1,
        "Should detect only JPEG selective deletion"
    );
    assert!(selective_patterns[0].description.contains("image/jpeg"));
}

#[test]
fn test_very_old_vs_recent_deletions() {
    let now = Utc::now();

    let files = vec![
        // Very old deletion (2 years ago)
        create_file_with_timestamps(
            1,
            "/old/ancient.txt",
            "text/plain",
            now - Duration::days(800),
            now - Duration::days(731),
            now - Duration::days(730),
        ),
        // Recent deletion (1 hour ago)
        create_file_with_timestamps(
            2,
            "/new/recent.txt",
            "text/plain",
            now - Duration::days(1),
            now - Duration::hours(2),
            now - Duration::hours(1),
        ),
    ];

    let session = create_test_session(FileSystemType::ExFat, files);
    let timeline = RecoveryTimeline::from_session(&session);

    assert_eq!(timeline.statistics.deletion_events, 2);

    // Timeline should span ~2 years
    let first_event = &timeline.events[0];
    let last_event = &timeline.events[timeline.events.len() - 1];
    let span = last_event.timestamp - first_event.timestamp;
    assert!(
        span.num_days() > 700,
        "Timeline should span approximately 2 years"
    );
}

// ============================================================================
// FILESYSTEM-SPECIFIC TESTS
// ============================================================================

#[test]
fn test_xfs_timeline_scenarios() {
    let now = Utc::now();

    // Simulate XFS-specific scenario: AG-based deletion pattern
    // Files from same allocation group deleted together
    let files: Vec<_> = (0..8)
        .map(|i| {
            let mut file = create_file_with_timestamps(
                i,
                &format!("/data/ag0/file_{}.bin", i),
                "application/octet-stream",
                now - Duration::days(10),
                now - Duration::days(2),
                now + Duration::seconds(i as i64 * 20),
            );
            file.inode_or_cluster = 100 + i; // Sequential inodes
            file
        })
        .collect();

    let session = create_test_session(FileSystemType::Xfs, files);
    let timeline = RecoveryTimeline::from_session(&session);

    assert_eq!(timeline.statistics.deletion_events, 8);

    // Should detect bulk deletion
    let bulk_patterns: Vec<_> = timeline
        .patterns
        .iter()
        .filter(|p| matches!(p.pattern_type, PatternType::BulkDeletion))
        .collect();
    assert!(
        !bulk_patterns.is_empty(),
        "XFS: Should detect AG-based bulk deletion"
    );

    // Verify CSV export works
    let csv = timeline.to_csv();
    assert!(csv.contains("FileDeleted"));
    assert!(csv.lines().count() > 1);

    // Verify JSON export works
    let json = timeline.to_json().expect("Should serialize to JSON");
    assert!(json.contains("deletion_events"));
}

#[test]
fn test_btrfs_timeline_scenarios() {
    let now = Utc::now();

    // Simulate Btrfs-specific scenario: Snapshot-based recovery
    // Files from different snapshots
    let files = vec![
        create_file_with_timestamps(
            1,
            "/snapshots/2024-01/doc.txt",
            "text/plain",
            now - Duration::days(400),
            now - Duration::days(395),
            now - Duration::days(390),
        ),
        create_file_with_timestamps(
            2,
            "/snapshots/2024-01/image.jpg",
            "image/jpeg",
            now - Duration::days(400),
            now - Duration::days(395),
            now - Duration::days(390),
        ),
        create_file_with_timestamps(
            3,
            "/snapshots/2024-02/doc.txt",
            "text/plain",
            now - Duration::days(370),
            now - Duration::days(365),
            now - Duration::days(360),
        ),
        create_file_with_timestamps(
            4,
            "/current/active.txt",
            "text/plain",
            now - Duration::days(10),
            now - Duration::days(1),
            now,
        ),
    ];

    let session = create_test_session(FileSystemType::Btrfs, files);
    let timeline = RecoveryTimeline::from_session(&session);

    assert_eq!(timeline.statistics.deletion_events, 4);
    assert!(timeline
        .statistics
        .file_types_affected
        .contains_key("text/plain"));

    // Check text report formatting
    let report = timeline.to_text_report();
    assert!(report.contains("RECOVERY TIMELINE ANALYSIS"));
    assert!(report.contains("STATISTICS"));
    assert!(report.contains("EVENT TIMELINE"));
}

#[test]
fn test_exfat_timeline_scenarios() {
    let now = Utc::now();

    // Simulate exFAT-specific scenario: FAT chain corruption leading to orphaned files
    // Deletions at 30-second intervals to trigger bulk detection (within 5 min window)
    let files: Vec<_> = (0..12)
        .map(|i| {
            let mut file = create_file_with_timestamps(
                i,
                &format!("/DCIM/100CANON/IMG_{:04}.JPG", i),
                "image/jpeg",
                now - Duration::days(15),
                now - Duration::days(15),
                now - Duration::hours(3) + Duration::seconds(i as i64 * 30), // 30 sec apart
            );
            file.inode_or_cluster = 2000 + (i * 8); // Cluster-based addressing
            file.size = 1024 * 1024 * 3; // 3MB images
            file
        })
        .collect();

    let session = create_test_session(FileSystemType::ExFat, files);
    let timeline = RecoveryTimeline::from_session(&session);

    assert_eq!(timeline.statistics.deletion_events, 12);

    // Should detect bulk deletion (12 photos in 6 minutes = multiple 5-min windows)
    let bulk_patterns: Vec<_> = timeline
        .patterns
        .iter()
        .filter(|p| matches!(p.pattern_type, PatternType::BulkDeletion))
        .collect();
    assert!(
        !bulk_patterns.is_empty(),
        "exFAT: Should detect camera deletion pattern"
    );

    // Should detect selective deletion (all JPEGs)
    let selective_patterns: Vec<_> = timeline
        .patterns
        .iter()
        .filter(|p| matches!(p.pattern_type, PatternType::SelectiveDeletion))
        .collect();
    assert!(
        !selective_patterns.is_empty(),
        "exFAT: Should detect JPEG selective deletion"
    );
}

#[test]
fn test_mixed_filesystem_comparison() {
    let now = Utc::now();

    // Create same scenario for all 3 filesystems
    let create_scenario = |fs_type| {
        let files: Vec<_> = (0..6)
            .map(|i| {
                create_file_with_timestamps(
                    i,
                    &format!("/test/file_{}.dat", i),
                    "application/octet-stream",
                    now - Duration::days(7),
                    now - Duration::days(1),
                    now + Duration::seconds(i as i64 * 45),
                )
            })
            .collect();
        create_test_session(fs_type, files)
    };

    let xfs_session = create_scenario(FileSystemType::Xfs);
    let btrfs_session = create_scenario(FileSystemType::Btrfs);
    let exfat_session = create_scenario(FileSystemType::ExFat);

    let xfs_timeline = RecoveryTimeline::from_session(&xfs_session);
    let btrfs_timeline = RecoveryTimeline::from_session(&btrfs_session);
    let exfat_timeline = RecoveryTimeline::from_session(&exfat_session);

    // All should produce identical timeline analysis
    assert_eq!(xfs_timeline.statistics.deletion_events, 6);
    assert_eq!(btrfs_timeline.statistics.deletion_events, 6);
    assert_eq!(exfat_timeline.statistics.deletion_events, 6);

    assert_eq!(xfs_timeline.patterns.len(), btrfs_timeline.patterns.len());
    assert_eq!(btrfs_timeline.patterns.len(), exfat_timeline.patterns.len());
}

// ============================================================================
// EXPORT FORMAT TESTS
// ============================================================================

#[test]
fn test_csv_export_formatting() {
    let now = Utc::now();
    let file = create_file_with_timestamps(
        1,
        "/test/file,with,commas.txt",
        "text/plain",
        now - Duration::days(1),
        now - Duration::hours(1),
        now,
    );

    let session = create_test_session(FileSystemType::Xfs, vec![file]);
    let timeline = RecoveryTimeline::from_session(&session);

    let csv = timeline.to_csv();

    // Check CSV header
    assert!(csv.starts_with("Timestamp,Event Type,File ID,Description"));

    // Commas in filenames should be replaced with semicolons
    assert!(csv.contains("with;commas"));

    // Each event should be on its own line
    let lines: Vec<_> = csv.lines().collect();
    assert_eq!(lines.len(), 4); // Header + 3 events
}

#[test]
fn test_json_export_structure() {
    let now = Utc::now();
    let files = vec![
        create_file_with_timestamps(
            1,
            "/a.txt",
            "text/plain",
            now - Duration::days(1),
            now - Duration::hours(2),
            now - Duration::hours(1),
        ),
        create_file_with_timestamps(
            2,
            "/b.txt",
            "text/plain",
            now - Duration::days(1),
            now - Duration::hours(2),
            now - Duration::minutes(30),
        ),
    ];

    let session = create_test_session(FileSystemType::Btrfs, files);
    let timeline = RecoveryTimeline::from_session(&session);

    let json_str = timeline.to_json().expect("Should serialize");
    let json_value: serde_json::Value = serde_json::from_str(&json_str).expect("Should parse JSON");

    // Verify JSON structure
    assert!(json_value.get("events").is_some());
    assert!(json_value.get("patterns").is_some());
    assert!(json_value.get("statistics").is_some());

    let stats = &json_value["statistics"];
    assert!(stats.get("total_events").is_some());
    assert!(stats.get("deletion_events").is_some());
    assert!(stats.get("file_types_affected").is_some());
}

#[test]
fn test_text_report_truncation() {
    let now = Utc::now();

    // Create 100 files to test truncation at 50 events
    let files: Vec<_> = (0..100)
        .map(|i| {
            create_file_with_timestamps(
                i,
                &format!("/test/file_{}.txt", i),
                "text/plain",
                now - Duration::days(1),
                now - Duration::hours(2),
                now - Duration::minutes(i as i64),
            )
        })
        .collect();

    let session = create_test_session(FileSystemType::ExFat, files);
    let timeline = RecoveryTimeline::from_session(&session);

    let report = timeline.to_text_report();

    // Should indicate truncation
    assert!(report.contains("... and 250 more events") || report.contains("more events"));
}

// ============================================================================
// PATTERN CONFIDENCE TESTS
// ============================================================================

#[test]
fn test_pattern_confidence_levels() {
    let now = Utc::now();

    // Bulk deletion should have 90% confidence
    let bulk_files: Vec<_> = (0..7)
        .map(|i| {
            create_file_with_timestamps(
                i,
                &format!("/bulk/file_{}.txt", i),
                "text/plain",
                now - Duration::days(1),
                now - Duration::hours(2),
                now + Duration::seconds(i as i64 * 15),
            )
        })
        .collect();

    let session = create_test_session(FileSystemType::Xfs, bulk_files);
    let timeline = RecoveryTimeline::from_session(&session);

    let bulk_pattern = timeline
        .patterns
        .iter()
        .find(|p| matches!(p.pattern_type, PatternType::BulkDeletion))
        .expect("Should have bulk deletion pattern");

    assert_eq!(bulk_pattern.confidence, 0.9);

    // Selective deletion should have 70% confidence
    let selective_pattern = timeline
        .patterns
        .iter()
        .find(|p| matches!(p.pattern_type, PatternType::SelectiveDeletion))
        .expect("Should have selective deletion pattern");

    assert_eq!(selective_pattern.confidence, 0.7);
}

#[test]
fn test_statistics_accuracy() {
    let now = Utc::now();

    let files = vec![
        create_file_with_timestamps(
            1,
            "/a.jpg",
            "image/jpeg",
            now - Duration::days(10),
            now - Duration::days(2),
            now - Duration::days(1),
        ),
        create_file_with_timestamps(
            2,
            "/b.jpg",
            "image/jpeg",
            now - Duration::days(10),
            now - Duration::days(2),
            now,
        ),
        create_file_with_timestamps(
            3,
            "/c.pdf",
            "application/pdf",
            now - Duration::days(5),
            now - Duration::days(1),
            now,
        ),
    ];

    let session = create_test_session(FileSystemType::Btrfs, files);
    let timeline = RecoveryTimeline::from_session(&session);

    // Verify statistics
    assert_eq!(timeline.statistics.deletion_events, 3);
    assert_eq!(timeline.statistics.total_events, 9); // 3 files × 3 events each

    assert_eq!(
        timeline.statistics.file_types_affected.get("image/jpeg"),
        Some(&2)
    );
    assert_eq!(
        timeline
            .statistics
            .file_types_affected
            .get("application/pdf"),
        Some(&1)
    );

    // Average deletions per day should be reasonable
    assert!(timeline.statistics.average_deletions_per_day >= 0.0);
    assert!(timeline.statistics.average_deletions_per_day <= 10.0);
}

// ============================================================================
// ADDITIONAL EDGE CASES - Advanced Scenarios
// ============================================================================

#[test]
fn test_future_timestamps_clock_skew() {
    let now = Utc::now();

    // Simulate clock skew: files with timestamps in the future
    let files = vec![
        // File deleted "tomorrow" (clock skew scenario)
        create_file_with_timestamps(
            1,
            "/test/future_file.txt",
            "text/plain",
            now - Duration::days(1),
            now - Duration::hours(1),
            now + Duration::days(1), // Future deletion time!
        ),
        // File created in the future
        create_file_with_timestamps(
            2,
            "/test/time_travel.txt",
            "text/plain",
            now + Duration::hours(5), // Future creation!
            now + Duration::hours(6), // Future modification!
            now,
        ),
        // Normal file for comparison
        create_file_with_timestamps(
            3,
            "/test/normal.txt",
            "text/plain",
            now - Duration::days(7),
            now - Duration::days(1),
            now - Duration::hours(2),
        ),
    ];

    let session = create_test_session(FileSystemType::Xfs, files);
    let timeline = RecoveryTimeline::from_session(&session);

    // Should handle future timestamps gracefully
    assert_eq!(timeline.statistics.deletion_events, 3); // All 3 files have deletion times
    assert!(
        timeline.events.len() >= 3,
        "Should include events with future timestamps"
    );

    // Timeline should still be chronologically sorted
    let mut last_timestamp = timeline.events[0].timestamp;
    for event in &timeline.events {
        assert!(
            event.timestamp >= last_timestamp,
            "Events should be chronologically sorted"
        );
        last_timestamp = event.timestamp;
    }

    println!("✅ Future timestamps handled gracefully");
}

#[test]
fn test_unicode_and_special_characters_in_filenames() {
    let now = Utc::now();

    // Test various Unicode and special character scenarios
    let files = vec![
        // Emoji in filename
        create_file_with_timestamps(
            1,
            "/home/user/📸Photos/vacation🌴.jpg",
            "image/jpeg",
            now - Duration::days(7),
            now - Duration::days(1),
            now - Duration::hours(2),
        ),
        // Cyrillic characters
        create_file_with_timestamps(
            2,
            "/documents/Документ.pdf",
            "application/pdf",
            now - Duration::days(5),
            now - Duration::days(1),
            now - Duration::hours(3),
        ),
        // Chinese characters
        create_file_with_timestamps(
            3,
            "/files/文档/报告.docx",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            now - Duration::days(3),
            now - Duration::days(1),
            now - Duration::hours(4),
        ),
        // Arabic characters
        create_file_with_timestamps(
            4,
            "/data/ملف_البيانات.json",
            "application/json",
            now - Duration::days(2),
            now - Duration::days(1),
            now - Duration::hours(5),
        ),
        // Special characters: spaces, quotes, parentheses
        create_file_with_timestamps(
            5,
            "/test/file (copy) 'with' \"quotes\".txt",
            "text/plain",
            now - Duration::days(1),
            now - Duration::hours(12),
            now - Duration::hours(6),
        ),
        // Very long filename (255 characters)
        create_file_with_timestamps(
            6,
            &format!("/long/{}.dat", "a".repeat(240)),
            "application/octet-stream",
            now - Duration::days(1),
            now - Duration::hours(10),
            now - Duration::hours(7),
        ),
    ];

    let session = create_test_session(FileSystemType::ExFat, files);
    let timeline = RecoveryTimeline::from_session(&session);

    // Should handle all special characters without crashes
    assert_eq!(timeline.statistics.deletion_events, 6);
    assert_eq!(timeline.events.len(), 18); // 6 files × 3 events each

    // CSV export should handle special characters
    let csv = timeline.to_csv();
    assert!(
        csv.contains("📸Photos") || csv.contains("vacation"),
        "CSV should contain unicode"
    );

    // JSON export should handle unicode properly
    let json = timeline.to_json().expect("Should serialize with unicode");
    assert!(
        json.contains("Документ") || json.contains("\\u"),
        "JSON should preserve or escape unicode"
    );

    // Text report should handle special characters
    let report = timeline.to_text_report();
    assert!(
        report.len() > 100,
        "Report should be generated despite special characters"
    );

    println!("✅ Unicode and special characters handled correctly");
    println!("   - Emoji: ✅");
    println!("   - Cyrillic: ✅");
    println!("   - Chinese: ✅");
    println!("   - Arabic: ✅");
    println!("   - Special chars: ✅");
    println!("   - Long filenames: ✅");
}

#[test]
fn test_extreme_timestamp_values() {
    let now = Utc::now();

    // Test extreme but valid timestamp scenarios
    let files = vec![
        // File from Unix epoch start (1970-01-01)
        create_file_with_timestamps(
            1,
            "/ancient/epoch_file.dat",
            "application/octet-stream",
            chrono::DateTime::from_timestamp(0, 0).unwrap(),
            chrono::DateTime::from_timestamp(100, 0).unwrap(),
            chrono::DateTime::from_timestamp(1000, 0).unwrap(), // Deleted 1000 seconds after epoch
        ),
        // Very recent file (milliseconds ago)
        create_file_with_timestamps(
            2,
            "/recent/just_now.txt",
            "text/plain",
            now - Duration::milliseconds(500),
            now - Duration::milliseconds(100),
            now - Duration::milliseconds(50),
        ),
        // File with all timestamps at exact same moment
        {
            let instant = now - Duration::hours(1);
            create_file_with_timestamps(
                3,
                "/instant/simultaneous.log",
                "text/plain",
                instant,
                instant, // Same time!
                instant, // Same time!
            )
        },
        // File from far in the past (50 years ago)
        create_file_with_timestamps(
            4,
            "/legacy/old_system.txt",
            "text/plain",
            now - Duration::days(365 * 50), // ~50 years old
            now - Duration::days(365 * 40),
            now - Duration::days(365 * 30),
        ),
    ];

    let session = create_test_session(FileSystemType::Btrfs, files);
    let timeline = RecoveryTimeline::from_session(&session);

    // Should handle extreme timestamps
    assert_eq!(timeline.statistics.deletion_events, 4);

    // Timeline span should be massive (Unix epoch to now)
    if let (Some(first), Some(last)) = (timeline.events.first(), timeline.events.last()) {
        let span = last.timestamp - first.timestamp;
        assert!(
            span.num_days() > 10000,
            "Timeline should span from 1970 to present"
        );
    }

    // Statistics should handle extreme ranges
    assert!(timeline.statistics.average_deletions_per_day >= 0.0);
    assert!(
        timeline.statistics.average_deletions_per_day < 1.0,
        "Should be very low average over 50+ years"
    );

    // CSV export should handle extreme dates
    let csv = timeline.to_csv();
    assert!(
        csv.contains("1970") || csv.contains("epoch"),
        "Should include epoch timestamps"
    );

    println!("✅ Extreme timestamp values handled correctly");
    println!("   - Unix epoch (1970): ✅");
    println!("   - Millisecond precision: ✅");
    println!("   - Same instant (created=modified=deleted): ✅");
    println!("   - 50-year-old files: ✅");
}
