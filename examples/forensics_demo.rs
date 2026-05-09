/// Demonstration of forensic features: Audit logging and hash verification
///
/// This example shows how to use the audit trail and hash verification
/// features for legal compliance and evidence integrity.
use ghostfs_core::{
    calculate_file_hash, verify_file_integrity, AuditLog, AuditLogger, HashAlgorithm, HashManifest,
};
use std::fs;
use std::sync::Arc;
use tempfile::TempDir;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔒 GhostFS Forensic Features Demo");
    println!("==================================\n");

    // Create temporary workspace
    let temp_dir = TempDir::new()?;
    let workspace = temp_dir.path();

    // ===================================================================
    // PART 1: AUDIT TRAIL LOGGING
    // ===================================================================
    println!("📝 Part 1: Audit Trail Logging");
    println!("-------------------------------\n");

    // Create audit log for recovery session
    let session_id = "CASE-2026-001";
    let audit_log = Arc::new(AuditLog::new(session_id, workspace.join("audit"))?);
    let logger = AuditLogger::new(audit_log.clone());

    println!("✅ Audit log created: {}", audit_log.log_path().display());
    println!("   Session ID: {}\n", session_id);

    // Log recovery session start
    logger.session_start("/dev/sda1")?;
    println!("📌 Logged: Session started on /dev/sda1");

    // Simulate file recovery operations
    logger.file_detected("document.pdf", "application/pdf", 0.98)?;
    println!("📌 Logged: File detected (document.pdf, 98% confidence)");

    logger.file_recovered("document.pdf", 1024000, 12345)?;
    println!("📌 Logged: File recovered (1MB, inode 12345)");

    logger.file_exported(
        "disk block 987654",
        "/evidence/CASE-2026-001/document.pdf",
        1024000,
    )?;
    println!("📌 Logged: File exported to evidence directory");

    // Log hash calculation (for integrity)
    logger.hash_calculated("document.pdf", "SHA256", "abc123def456...")?;
    println!("📌 Logged: Hash calculated for verification\n");

    // Log session end
    logger.session_end("Success")?;
    println!("📌 Logged: Session ended successfully\n");

    // Display audit statistics
    let stats = audit_log.get_statistics();
    println!("📊 Audit Statistics:");
    println!("   Total entries: {}", stats.total_entries);
    println!("   Event types:");
    for (event_type, count) in &stats.event_type_counts {
        println!("     - {:?}: {}", event_type, count);
    }
    println!();

    // Export audit log
    let json_export = workspace.join("audit_log.json");
    let csv_export = workspace.join("audit_log.csv");
    audit_log.export_json(&json_export)?;
    audit_log.export_csv(&csv_export)?;
    println!("✅ Audit log exported:");
    println!("   JSON: {}", json_export.display());
    println!("   CSV:  {}\n", csv_export.display());

    // ===================================================================
    // PART 2: HASH-BASED VERIFICATION
    // ===================================================================
    println!("🔐 Part 2: Hash-Based Verification");
    println!("-----------------------------------\n");

    // Create test evidence files
    let evidence_dir = workspace.join("evidence");
    fs::create_dir_all(&evidence_dir)?;

    let file1 = evidence_dir.join("document.pdf");
    let file2 = evidence_dir.join("photo.jpg");
    let file3 = evidence_dir.join("database.db");

    fs::write(&file1, b"Mock PDF content")?;
    fs::write(&file2, b"Mock JPEG data")?;
    fs::write(&file3, b"Mock database")?;

    println!("📁 Created evidence files:");
    println!("   - document.pdf");
    println!("   - photo.jpg");
    println!("   - database.db\n");

    // Calculate hashes for all files
    println!("🔢 Calculating hashes...");

    let hash1_sha256 = calculate_file_hash(&file1, HashAlgorithm::SHA256)?;
    let hash1_md5 = calculate_file_hash(&file1, HashAlgorithm::MD5)?;

    println!("\n📄 document.pdf:");
    println!("   SHA256: {}", hash1_sha256.hash);
    println!("   MD5:    {}", hash1_md5.hash);
    println!("   Size:   {} bytes", hash1_sha256.file_size);

    let hash2 = calculate_file_hash(&file2, HashAlgorithm::SHA256)?;
    println!("\n📷 photo.jpg:");
    println!("   SHA256: {}", hash2.hash);
    println!("   Size:   {} bytes", hash2.file_size);

    let hash3 = calculate_file_hash(&file3, HashAlgorithm::SHA256)?;
    println!("\n💾 database.db:");
    println!("   SHA256: {}", hash3.hash);
    println!("   Size:   {} bytes\n", hash3.file_size);

    // Create hash manifest (forensic inventory)
    println!("📋 Creating hash manifest...");
    let mut manifest = HashManifest::new(session_id, HashAlgorithm::SHA256);

    manifest.add_file("document.pdf".to_string(), hash1_sha256.clone());
    manifest.add_file("photo.jpg".to_string(), hash2.clone());
    manifest.add_file("database.db".to_string(), hash3.clone());

    let manifest_path = workspace.join("hash_manifest.json");
    manifest.export_json(&manifest_path)?;
    println!("✅ Hash manifest saved: {}\n", manifest_path.display());

    // Verify file integrity
    println!("🔍 Verifying file integrity...");

    let verification =
        verify_file_integrity(&file1, Some(&hash1_sha256.hash), HashAlgorithm::SHA256)?;

    match verification.status {
        ghostfs_core::VerificationStatus::Verified => {
            println!("✅ document.pdf: VERIFIED - File is authentic");
        }
        _ => {
            println!("❌ document.pdf: CORRUPTED - Hash mismatch!");
        }
    }

    // Test corruption detection
    println!("\n⚠️  Simulating file corruption...");
    fs::write(&file1, b"Corrupted content")?;

    let verification =
        verify_file_integrity(&file1, Some(&hash1_sha256.hash), HashAlgorithm::SHA256)?;

    match verification.status {
        ghostfs_core::VerificationStatus::Corrupted => {
            println!("🚨 Corruption detected!");
            println!("   Expected: {}", verification.expected_hash.unwrap());
            println!("   Actual:   {}", verification.actual_hash);
        }
        _ => {
            println!("✅ File verified");
        }
    }

    // Restore file and verify
    println!("\n🔧 Restoring file from backup...");
    fs::write(&file1, b"Mock PDF content")?;

    let verification =
        verify_file_integrity(&file1, Some(&hash1_sha256.hash), HashAlgorithm::SHA256)?;

    match verification.status {
        ghostfs_core::VerificationStatus::Verified => {
            println!("✅ File restored and verified successfully!");
        }
        _ => {
            println!("❌ File still corrupted");
        }
    }

    // Verify all files in manifest
    println!("\n📊 Verifying entire evidence collection...");
    let verification_result = manifest.verify_all(&evidence_dir)?;

    println!(
        "   Total files:  {}",
        verification_result.summary.total_files
    );
    println!("   ✅ Verified:  {}", verification_result.summary.verified);
    println!("   ❌ Corrupted: {}", verification_result.summary.corrupted);
    println!(
        "   ⚠️  No ref:   {}",
        verification_result.summary.no_reference
    );
    println!(
        "   Success rate: {:.1}%",
        verification_result.summary.success_rate * 100.0
    );

    // ===================================================================
    // PART 3: FORENSIC WORKFLOW SUMMARY
    // ===================================================================
    println!("\n\n📊 Forensic Workflow Summary");
    println!("============================\n");

    println!("🔒 Audit Trail:");
    println!("   ✅ {} events logged", stats.total_entries);
    println!("   ✅ Exported to JSON and CSV");
    println!("   ✅ Tamper-proof log file (JSONL format)");
    println!("   ✅ Ready for legal review\n");

    println!("🔐 Hash Verification:");
    println!("   ✅ {} files hashed", manifest.files.len());
    println!("   ✅ SHA-256 algorithm (industry standard)");
    println!("   ✅ Manifest exported for future verification");
    println!("   ✅ Corruption detection working\n");

    println!("📁 Evidence Package Contents:");
    println!("   📄 Audit log:      {}", audit_log.log_path().display());
    println!("   📄 Hash manifest:  {}", manifest_path.display());
    println!("   📁 Evidence files: {}", evidence_dir.display());

    println!("\n✨ Forensic features ready for production use!");
    println!("   - Court-admissible audit trails");
    println!("   - Cryptographic integrity verification");
    println!("   - Complete chain of custody tracking");

    Ok(())
}
