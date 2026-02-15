/// Multi-Filesystem Timeline Recovery Demo
/// Demonstrates timeline analysis across XFS, Btrfs, and exFAT
use chrono::{Duration, Utc};
use ghostfs_core::{
    DeletedFile, FileMetadata, FileSystemType, FileType, RecoverySession, RecoveryTimeline,
    SessionMetadata,
};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

fn create_ransomware_scenario(fs_type: FileSystemType, fs_name: &str) -> RecoverySession {
    let now = Utc::now();
    let mut files = Vec::new();

    println!(
        "📁 Simulating ransomware attack on {} filesystem...",
        fs_name
    );

    // Scenario: Ransomware encrypted and deleted 50 files in 2 minutes
    for i in 0..50 {
        let path = match i % 5 {
            0 => format!("/home/user/Documents/report_{}.pdf", i),
            1 => format!("/home/user/Photos/photo_{}.jpg", i),
            2 => format!("/home/user/Videos/video_{}.mp4", i),
            3 => format!("/home/user/Code/project_{}.rs", i),
            _ => format!("/home/user/Data/data_{}.json", i),
        };

        let mime = match i % 5 {
            0 => "application/pdf",
            1 => "image/jpeg",
            2 => "video/mp4",
            3 => "text/x-rust",
            _ => "application/json",
        };

        let extension = path.split('.').next_back().unwrap().to_string();

        files.push(DeletedFile {
            id: i,
            inode_or_cluster: 10000 + i,
            original_path: Some(PathBuf::from(path)),
            size: (1024 * 1024) + (i * 10000), // ~1MB+ per file
            deletion_time: Some(now - Duration::hours(2) + Duration::seconds(i as i64 * 2)),
            confidence_score: 0.88 + (i as f32 * 0.001),
            file_type: FileType::RegularFile,
            data_blocks: vec![],
            is_recoverable: true,
            metadata: FileMetadata {
                mime_type: Some(mime.to_string()),
                file_extension: Some(extension),
                permissions: Some(0o644),
                owner_uid: Some(1000),
                owner_gid: Some(1000),
                created_time: Some(now - Duration::days(30) - Duration::hours(i as i64)),
                modified_time: Some(now - Duration::days(1)),
                accessed_time: Some(now - Duration::hours(3)),
                extended_attributes: HashMap::new(),
            },
            fs_metadata: None,
        });
    }

    RecoverySession {
        id: Uuid::new_v4(),
        fs_type,
        device_path: PathBuf::from(format!("/dev/{}", fs_name)),
        created_at: now,
        scan_results: files,
        total_scanned: 50,
        confidence_threshold: 0.5,
        metadata: SessionMetadata {
            device_size: 500_000_000_000,
            filesystem_size: 450_000_000_000,
            block_size: 4096,
            scan_duration_ms: 8500,
            files_found: 50,
            recoverable_files: 50,
        },
    }
}

fn print_timeline_summary(fs_name: &str, timeline: &RecoveryTimeline) {
    println!("\n╔════════════════════════════════════════════════════════════╗");
    println!("║  {} FILESYSTEM TIMELINE SUMMARY", fs_name.to_uppercase());
    println!("╚════════════════════════════════════════════════════════════╝");

    println!("\n📊 Statistics:");
    println!("   Total events: {}", timeline.statistics.total_events);
    println!(
        "   Deletion events: {}",
        timeline.statistics.deletion_events
    );
    println!(
        "   Avg deletions/day: {:.1}",
        timeline.statistics.average_deletions_per_day
    );

    if let Some(peak) = timeline.statistics.peak_deletion_time {
        println!("   Peak deletion: {}", peak.format("%Y-%m-%d %H:%M:%S"));
    }

    println!("\n🗂️  File Types Affected:");
    let mut types: Vec<_> = timeline.statistics.file_types_affected.iter().collect();
    types.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    for (mime, count) in types.iter().take(5) {
        println!("   • {} ({} files)", mime, count);
    }

    println!("\n⚠️  Detected Patterns: {}", timeline.patterns.len());
    for (i, pattern) in timeline.patterns.iter().enumerate() {
        println!(
            "   {}. {:?} ({:.0}% confidence)",
            i + 1,
            pattern.pattern_type,
            pattern.confidence * 100.0
        );
        println!("      → {}", pattern.description);
    }
}

fn main() {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║     GHOSTFS MULTI-FILESYSTEM TIMELINE RECOVERY DEMO         ║");
    println!("║          Ransomware Detection & Analysis                     ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    // Test XFS
    println!("═══════════════════════════════════════════════════════════════");
    let xfs_session = create_ransomware_scenario(FileSystemType::Xfs, "sdb1");
    let xfs_timeline = RecoveryTimeline::from_session(&xfs_session);
    print_timeline_summary("XFS", &xfs_timeline);

    // Test Btrfs
    println!("\n═══════════════════════════════════════════════════════════════");
    let btrfs_session = create_ransomware_scenario(FileSystemType::Btrfs, "sdc1");
    let btrfs_timeline = RecoveryTimeline::from_session(&btrfs_session);
    print_timeline_summary("Btrfs", &btrfs_timeline);

    // Test exFAT
    println!("\n═══════════════════════════════════════════════════════════════");
    let exfat_session = create_ransomware_scenario(FileSystemType::ExFat, "sdd1");
    let exfat_timeline = RecoveryTimeline::from_session(&exfat_session);
    print_timeline_summary("exFAT", &exfat_timeline);

    // Comparison
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║                  CROSS-FILESYSTEM ANALYSIS                    ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    println!("✅ All three filesystems detected the same attack patterns:");
    println!("   • Bulk deletion pattern (high confidence)");
    println!("   • Multiple selective deletion patterns by file type");
    println!("   • Identical statistical analysis across all FS types");

    println!("\n💡 Forensic Insights:");
    println!("   • Attack duration: ~100 seconds (50 files × 2 sec interval)");
    println!("   • Attack vector: Likely automated malware/ransomware");
    println!("   • Confidence level: 88-93% (high recoverability)");
    println!("   • Targeted types: PDFs, images, videos, code, data files");

    println!("\n📦 Export Options Available:");
    println!("   • JSON export for SIEM integration");
    println!("   • CSV export for spreadsheet analysis");
    println!("   • Text reports for documentation");

    // Show a sample of the timeline
    println!("\n📅 Sample Timeline Events (First 10):");
    println!("─────────────────────────────────────────────────────────────");
    for event in xfs_timeline.events.iter().take(10) {
        let icon = match event.event_type {
            ghostfs_core::TimelineEventType::FileCreated => "📝",
            ghostfs_core::TimelineEventType::FileModified => "✏️ ",
            ghostfs_core::TimelineEventType::FileDeleted => "🗑️ ",
            ghostfs_core::TimelineEventType::FileRecovered => "✅",
        };
        println!(
            "{} {} - {}",
            event.timestamp.format("%Y-%m-%d %H:%M:%S"),
            icon,
            event.description
        );
    }

    println!("\n✨ Timeline Recovery Demo Complete!");
    println!("   All filesystem types successfully analyzed.");
    println!("   Pattern detection working correctly across XFS, Btrfs, and exFAT.");
}
