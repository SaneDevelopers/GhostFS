/// Demonstration of GhostFS Timeline Recovery Analysis
use chrono::{Duration, Utc};
use ghostfs_core::{
    DeletedFile, FileMetadata, FileSystemType, FileType, RecoverySession, RecoveryTimeline,
    SessionMetadata,
};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

fn main() {
    println!("🔍 GhostFS Timeline Recovery Demo\n");

    // Simulate a scenario: bulk deletion of photos followed by selective document deletion
    let now = Utc::now();
    let mut files = vec![];

    // Scenario 1: Bulk photo deletion (5 photos deleted within 2 minutes - suspicious!)
    for i in 0..5 {
        files.push(DeletedFile {
            id: i,
            inode_or_cluster: 1000 + i,
            original_path: Some(PathBuf::from(format!("/home/user/Photos/img_{}.jpg", i))),
            size: 2_048_000 + (i * 100_000),
            deletion_time: Some(now - Duration::days(2) + Duration::seconds(i as i64 * 20)),
            confidence_score: 0.85 + (i as f32 * 0.02),
            file_type: FileType::RegularFile,
            data_blocks: vec![],
            is_recoverable: true,
            metadata: FileMetadata {
                mime_type: Some("image/jpeg".to_string()),
                file_extension: Some("jpg".to_string()),
                permissions: Some(0o644),
                owner_uid: Some(1000),
                owner_gid: Some(1000),
                created_time: Some(now - Duration::days(30)),
                modified_time: Some(now - Duration::days(10)),
                accessed_time: Some(now - Duration::days(3)),
                extended_attributes: HashMap::new(),
            },
            fs_metadata: None,
        });
    }

    // Scenario 2: Selective document deletion (3 PDF files)
    for i in 0..3 {
        files.push(DeletedFile {
            id: 10 + i,
            inode_or_cluster: 2000 + i,
            original_path: Some(PathBuf::from(format!(
                "/home/user/Documents/report_{}.pdf",
                i
            ))),
            size: 512_000 + (i * 50_000),
            deletion_time: Some(now - Duration::hours(6) + Duration::minutes(i as i64 * 15)),
            confidence_score: 0.90,
            file_type: FileType::RegularFile,
            data_blocks: vec![],
            is_recoverable: true,
            metadata: FileMetadata {
                mime_type: Some("application/pdf".to_string()),
                file_extension: Some("pdf".to_string()),
                permissions: Some(0o644),
                owner_uid: Some(1000),
                owner_gid: Some(1000),
                created_time: Some(now - Duration::days(7)),
                modified_time: Some(now - Duration::days(1)),
                accessed_time: Some(now - Duration::hours(8)),
                extended_attributes: HashMap::new(),
            },
            fs_metadata: None,
        });
    }

    // Scenario 3: Some random text file deletions
    for i in 0..2 {
        files.push(DeletedFile {
            id: 20 + i,
            inode_or_cluster: 3000 + i,
            original_path: Some(PathBuf::from(format!("/home/user/notes_{}.txt", i))),
            size: 4096,
            deletion_time: Some(now - Duration::days(5) + Duration::hours(i as i64 * 3)),
            confidence_score: 0.75,
            file_type: FileType::RegularFile,
            data_blocks: vec![],
            is_recoverable: true,
            metadata: FileMetadata {
                mime_type: Some("text/plain".to_string()),
                file_extension: Some("txt".to_string()),
                permissions: Some(0o644),
                owner_uid: Some(1000),
                owner_gid: Some(1000),
                created_time: Some(now - Duration::days(20)),
                modified_time: Some(now - Duration::days(6)),
                accessed_time: Some(now - Duration::days(5)),
                extended_attributes: HashMap::new(),
            },
            fs_metadata: None,
        });
    }

    // Create recovery session
    let session = RecoverySession {
        id: Uuid::new_v4(),
        fs_type: FileSystemType::Xfs,
        device_path: PathBuf::from("/dev/sdb1"),
        created_at: now,
        scan_results: files,
        total_scanned: 1000,
        confidence_threshold: 0.5,
        metadata: SessionMetadata {
            device_size: 500_000_000_000, // 500 GB
            filesystem_size: 450_000_000_000,
            block_size: 4096,
            scan_duration_ms: 12500,
            files_found: 10,
            recoverable_files: 10,
        },
    };

    // Generate timeline analysis
    let timeline = RecoveryTimeline::from_session(&session);

    // Display the full report
    println!("{}", timeline.to_text_report());

    // Show pattern detection details
    println!("\n🔬 DETAILED PATTERN ANALYSIS\n");
    println!("───────────────────────────────────────────────────────");
    for (i, pattern) in timeline.patterns.iter().enumerate() {
        println!("\nPattern #{}", i + 1);
        println!("  Type: {:?}", pattern.pattern_type);
        println!("  Confidence: {:.0}%", pattern.confidence * 100.0);
        println!("  Description: {}", pattern.description);
        println!("  Files affected: {}", pattern.affected_files.len());
        println!(
            "  Time window: {} to {}",
            pattern.timeframe.0.format("%Y-%m-%d %H:%M:%S"),
            pattern.timeframe.1.format("%Y-%m-%d %H:%M:%S")
        );
    }

    // Export examples
    println!("\n💾 EXPORT CAPABILITIES\n");
    println!("───────────────────────────────────────────────────────");

    // JSON export
    match timeline.to_json() {
        Ok(json) => {
            println!("✅ JSON export available ({} bytes)", json.len());
            println!("   Use: --json timeline.json");
        }
        Err(e) => println!("❌ JSON export failed: {}", e),
    }

    // CSV export
    let csv = timeline.to_csv();
    println!("✅ CSV export available ({} rows)", csv.lines().count());
    println!("   Use: --csv timeline.csv");

    println!("\n✨ Demo complete!");
}
