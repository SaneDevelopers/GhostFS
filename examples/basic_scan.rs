//! Basic example demonstrating GhostFS scanning functionality
//!
//! Run with: cargo run --example basic_scan

use ghostfs_core::{scan_and_analyze, FileSystemType};
use std::path::Path;

fn main() -> anyhow::Result<()> {
    // Example: Scan an XFS image
    let image_path = Path::new("test-data/test-xfs.img");

    // Check if test image exists
    if !image_path.exists() {
        eprintln!("Test image not found: {}", image_path.display());
        eprintln!("   Create one using: scripts/create-test-xfs.sh");
        eprintln!();
        eprintln!("   Or specify your own image path.");
        return Ok(());
    }

    let fs_type = FileSystemType::Xfs;

    println!("GhostFS Basic Scan Example");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Image: {}", image_path.display());
    println!("Filesystem: {:?}", fs_type);
    println!("Confidence scoring: Automatic");
    println!();

    // Perform scan (software auto-calculates confidence)
    println!("Scanning for recoverable files...");
    let session = scan_and_analyze(image_path, fs_type)?;

    println!();
    println!("Scan Complete!");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Session ID: {}", session.id);
    println!("Total files found: {}", session.metadata.files_found);
    println!("Recoverable files: {}", session.metadata.recoverable_files);
    println!();

    // Display recoverable files
    let recoverable: Vec<_> = session
        .scan_results
        .iter()
        .filter(|f| f.is_recoverable)
        .collect();

    if recoverable.is_empty() {
        println!("📭 No recoverable files found above the confidence threshold.");
    } else {
        println!("📋 Recoverable Files:");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        for (i, file) in recoverable.iter().take(10).enumerate() {
            let path_str = file
                .original_path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| format!("inode_{}", file.inode_or_cluster));

            let file_type = file
                .metadata
                .mime_type
                .as_ref()
                .unwrap_or(&"unknown".to_string())
                .clone();

            println!();
            println!("  {}. {}", i + 1, path_str);
            println!("     Size: {} bytes", file.size);
            println!("     Confidence: {:.1}%", file.confidence_score * 100.0);
            println!("     Type: {}", file_type);

            if let Some(del_time) = file.deletion_time {
                println!("     Deleted: {}", del_time.format("%Y-%m-%d %H:%M:%S"));
            }
        }

        if recoverable.len() > 10 {
            println!();
            println!("  ... and {} more files", recoverable.len() - 10);
        }
    }

    println!();
    println!("To recover files, use the CLI:");
    println!(
        "   cargo run -p ghostfs-cli -- recover {} --fs xfs --out ./recovered",
        image_path.display()
    );

    Ok(())
}
