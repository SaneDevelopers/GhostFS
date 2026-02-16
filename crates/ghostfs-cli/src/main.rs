use std::io::{self, Write};
use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use ghostfs_core::{FileSystemType, XfsRecoveryConfig, SessionManager};

/// Parse user input for scan limit (e.g., "50%", "10GB", "all")
fn parse_scan_limit(input: &str, total_blocks: u64, block_size: u32) -> Option<u64> {
    let input = input.trim().to_lowercase();

    if input == "all" || input == "100%" {
        return Some(total_blocks);
    }

    // Handle percentage (e.g., "50%")
    if let Some(percent_str) = input.strip_suffix('%') {
        if let Ok(percent) = percent_str.parse::<f64>() {
            if percent > 0.0 && percent <= 100.0 {
                return Some((total_blocks as f64 * percent / 100.0) as u64);
            }
        }
    }

    // Handle storage size (e.g., "10GB", "500MB", "1TB")
    let (num_str, unit) = if input.ends_with("tb") {
        (input.trim_end_matches("tb"), 1024u64 * 1024 * 1024 * 1024)
    } else if input.ends_with("gb") {
        (input.trim_end_matches("gb"), 1024u64 * 1024 * 1024)
    } else if input.ends_with("mb") {
        (input.trim_end_matches("mb"), 1024u64 * 1024)
    } else if input.ends_with("kb") {
        (input.trim_end_matches("kb"), 1024u64)
    } else {
        return None;
    };

    if let Ok(size) = num_str.trim().parse::<f64>() {
        let bytes = (size * unit as f64) as u64;
        let blocks = bytes / block_size as u64;
        return Some(std::cmp::min(blocks, total_blocks));
    }

    None
}

/// Prompt user for scan limit on large filesystems
fn prompt_scan_limit(total_blocks: u64, block_size: u32) -> Result<Option<u64>> {
    let total_size_gb = (total_blocks * block_size as u64) as f64 / (1024.0 * 1024.0 * 1024.0);

    println!(
        "\n⚠️  Large filesystem detected: {:.2} GB ({} blocks)",
        total_size_gb, total_blocks
    );
    println!("   Scanning all blocks may take considerable time.");
    println!("\n📊 Scan options:");
    println!("   • Type 'all' or '100%' to scan entire filesystem (thorough but slow)");
    println!("   • Type a percentage: e.g., '10%' to scan 10% of blocks");
    println!("   • Type storage size: e.g., '50GB', '500MB', '1TB'");
    println!("   • Press Enter for smart adaptive scan (recommended)");

    print!("\n🔍 How much do you want to scan? [adaptive]: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim();

    if input.is_empty() {
        println!("✅ Using adaptive scan (smart default)");
        return Ok(None); // Use adaptive default
    }

    match parse_scan_limit(input, total_blocks, block_size) {
        Some(blocks) => {
            let percent = (blocks as f64 / total_blocks as f64) * 100.0;
            let size_gb = (blocks * block_size as u64) as f64 / (1024.0 * 1024.0 * 1024.0);
            println!(
                "✅ Will scan {} blocks ({:.1}% / {:.2} GB)",
                blocks, percent, size_gb
            );
            Ok(Some(blocks))
        }
        None => {
            println!("❌ Invalid input. Using adaptive scan.");
            Ok(None)
        }
    }
}

#[derive(Parser, Debug)]
#[command(
    name = "ghostfs",
    version,
    about = "GhostFS CLI - Professional Data Recovery Tool"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum SessionAction {
    /// List all saved sessions
    List {
        /// Filter by filesystem type (xfs, btrfs, exfat)
        #[arg(long)]
        fs: Option<String>,
        /// Filter by device path
        #[arg(long)]
        device: Option<String>,
    },
    /// Show detailed information about a session
    Info {
        /// Session ID (full UUID or first 8+ characters)
        id: String,
    },
    /// Delete one or more sessions
    Delete {
        /// Session ID(s) to delete
        ids: Vec<String>,
    },
    /// Clean up old sessions (older than specified days)
    Cleanup {
        /// Delete sessions older than this many days
        #[arg(long, default_value = "30")]
        days: u32,
    },
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Scan an image file for recoverable entries
    Scan {
        /// Path to image file (use image files for safety; raw devices later)
        image: PathBuf,
        /// Filesystem type
        #[arg(long, value_parser = ["xfs", "btrfs", "exfat"], default_value = "xfs")]
        fs: String,
        /// Show detailed filesystem information
        #[arg(long)]
        info: bool,
        /// Disable interactive prompts (for CI/automation)
        #[arg(long)]
        no_interactive: bool,
        /// Save session after scanning
        #[arg(long)]
        save: bool,
        /// Optional session name (defaults to device path)
        #[arg(long)]
        name: Option<String>,
    },
    /// Detect filesystem type
    Detect {
        /// Path to image file
        image: PathBuf,
    },
    /// Recover files from an image
    Recover {
        /// Path to image file
        #[arg(long, conflicts_with = "session")]
        image: Option<PathBuf>,
        /// Load session from database (ID or first 8+ chars)
        #[arg(long, conflicts_with = "image")]
        session: Option<String>,
        /// Filesystem type (required with --image)
        #[arg(long, value_parser = ["xfs", "btrfs", "exfat"], required_if_eq("session", "None"))]
        fs: Option<String>,
        /// Output directory for recovered files
        #[arg(long)]
        out: PathBuf,
        /// File IDs to recover (if not specified, recovers all recoverable files)
        #[arg(long)]
        ids: Option<Vec<String>>,
        /// Disable interactive prompts (for CI/automation)
        #[arg(long)]
        no_interactive: bool,
        /// Enable forensics mode with audit trail
        #[arg(long)]
        forensics: bool,
        /// Enable audit trail logging (creates audit.jsonl)
        #[arg(long)]
        audit: bool,
        /// Enable hash verification (creates hash_manifest.json)
        #[arg(long)]
        verify_hash: bool,
        /// Hash algorithm for verification (sha256, sha512, sha1, md5)
        #[arg(long, default_value = "sha256")]
        hash_algorithm: String,
        /// Enable partial file recovery
        #[arg(long)]
        partial: bool,
        /// Enable smart extent reconstruction
        #[arg(long)]
        reconstruct: bool,
    },
    /// Show a timeline of file deletion activity
    Timeline {
        /// Path to image file (required to generate timeline)
        image: PathBuf,
        /// Filesystem type
        #[arg(long, value_parser = ["xfs", "btrfs", "exfat"], default_value = "xfs")]
        fs: String,
        /// Export timeline to JSON file
        #[arg(long)]
        json: Option<PathBuf>,
        /// Export timeline to CSV file
        #[arg(long)]
        csv: Option<PathBuf>,
    },
    /// Manage saved recovery sessions
    Sessions {
        #[command(subcommand)]
        action: SessionAction,
    },
}

/// Get XFS recovery config with optional user prompts for large filesystems
fn get_xfs_config_for_scan(
    image: &PathBuf,
    interactive: bool,
) -> Result<Option<XfsRecoveryConfig>> {
    use ghostfs_core::fs::common::BlockDevice;
    use ghostfs_core::fs::xfs;

    // Try to get filesystem size
    let device = match BlockDevice::open(image) {
        Ok(d) => d,
        Err(_) => return Ok(None), // Can't open device, use defaults
    };

    let (total_blocks, block_size) = match xfs::get_filesystem_size(&device) {
        Ok(size) => size,
        Err(_) => return Ok(None), // Not XFS or can't read, use defaults
    };

    let total_size_gb = (total_blocks * block_size as u64) as f64 / (1024.0 * 1024.0 * 1024.0);

    // Prompt user only if filesystem is large (>100GB) and interactive mode
    if interactive && total_size_gb > 100.0 {
        if let Ok(Some(custom_blocks)) = prompt_scan_limit(total_blocks, block_size) {
            let config = XfsRecoveryConfig {
                max_scan_blocks: Some(custom_blocks),
                ..Default::default()
            };
            return Ok(Some(config));
        }
    }

    Ok(None) // Use adaptive defaults
}

fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    match cli.command {
        Commands::Scan {
            image,
            fs,
            info,
            no_interactive,
            save,
            name,
        } => {
            let fs_type = match fs.as_str() {
                "xfs" => FileSystemType::Xfs,
                "btrfs" => FileSystemType::Btrfs,
                "exfat" => FileSystemType::ExFat,
                _ => unreachable!(),
            };

            if info {
                // Show filesystem information
                match ghostfs_core::fs::get_filesystem_info(&image, fs_type) {
                    Ok(info_str) => {
                        println!("📋 File System Information:");
                        println!("{}", info_str);
                        println!();
                    }
                    Err(e) => {
                        eprintln!("❌ Failed to read filesystem info: {}", e);
                        return Err(e);
                    }
                }
            }

            // Get XFS config - only prompt interactively if stdin is a TTY and not --no-interactive
            let interactive = !no_interactive && atty::is(atty::Stream::Stdin);
            let xfs_config = if fs_type == FileSystemType::Xfs {
                get_xfs_config_for_scan(&image, interactive)?
            } else {
                None
            };

            // Perform scan (no threshold - software auto-calculates confidence)
            let session = ghostfs_core::scan_and_analyze_with_config(&image, fs_type, xfs_config)?;

            println!("Scan completed successfully!");
            println!("Session ID: {}", session.id);
            println!("File System: {}", session.fs_type);
            println!(
                "Device Size: {} MB",
                session.metadata.device_size / (1024 * 1024)
            );
            println!("Files Found: {}", session.metadata.files_found);
            println!(
                "Recoverable Files: {} (confidence >= 40%)",
                session.metadata.recoverable_files
            );

            // Show detailed file list with auto-calculated confidence
            if !session.scan_results.is_empty() {
                println!("\nFound Files:");
                println!(
                    "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
                );
                for file in &session.scan_results {
                    let path_str = file
                        .original_path
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| format!("inode_{}", file.inode_or_cluster));

                    let recommendation = if file.confidence_score >= 0.8 {
                        "✅ High - Excellent recovery prospects"
                    } else if file.confidence_score >= 0.6 {
                        "🟡 Medium - Good recovery prospects"
                    } else if file.confidence_score >= 0.4 {
                        "🟠 Low - Fair recovery prospects"
                    } else {
                        "❌ Poor - Not recommended"
                    };

                    println!("\n  ID: {} | {}", file.id, path_str);
                    println!(
                        "     Size: {} bytes | Confidence: {:.1}%",
                        file.size,
                        file.confidence_score * 100.0
                    );
                    println!("     {}", recommendation);
                }
            }

            // Save session if requested
            if save {
                let manager = SessionManager::new()?;
                manager.save(&session)?;
                
                let session_name = name.unwrap_or_else(|| image.display().to_string());
                println!("\n💾 Session saved!");
                println!("   ID: {}", &session.id.to_string()[..8]);
                println!("   Name: {}", session_name);
                println!("\n💡 Recover later with: ghostfs recover --session {} --out ./recovered", &session.id.to_string()[..8]);
            }
        }
        Commands::Detect { image } => {
            println!("Detecting file system type for: {}", image.display());

            match ghostfs_core::fs::detect_filesystem(&image)? {
                Some(fs_type) => {
                    println!("Detected: {}", fs_type);

                    // Show basic info
                    if let Ok(info) = ghostfs_core::fs::get_filesystem_info(&image, fs_type) {
                        println!();
                        println!("{}", info);
                    }
                }
                None => {
                    println!("Unknown or unsupported file system");
                }
            }
        }
        Commands::Recover {
            image,
            session: session_id,
            fs,
            out,
            ids,
            no_interactive,
            forensics,
            audit,
            verify_hash,
            hash_algorithm,
            partial,
            reconstruct,
        } => {
            // Validate that either image or session is provided
            if image.is_none() && session_id.is_none() {
                eprintln!("❌ Error: Either --image or --session must be provided");
                std::process::exit(1);
            }

            // Create output directory if it doesn't exist
            std::fs::create_dir_all(&out)?;

            // Load or scan for session
            let (session, device_path) = if let Some(session_id_str) = session_id {
                // Load from saved session
                println!("📂 Loading session: {}", session_id_str);
                let manager = SessionManager::new()?;
                let loaded_session = manager.load(&session_id_str)?;
                let device = loaded_session.device_path.display().to_string();
                println!("✅ Session loaded: {}", loaded_session.fs_type);
                println!("   Device: {}", device);
                println!("   Recoverable Files: {}", loaded_session.metadata.recoverable_files);
                (loaded_session, device)
            } else {
                // Scan image to create new session
                let image_path = image.as_ref().unwrap();
                let fs_str = fs.as_ref().expect("--fs is required when using --image");
                
                println!("Starting recovery process for: {}", image_path.display());
                println!("Output directory: {}", out.display());

                // Parse filesystem type
                let fs_type = match fs_str.as_str() {
                    "xfs" => ghostfs_core::FileSystemType::Xfs,
                    "btrfs" => ghostfs_core::FileSystemType::Btrfs,
                    "exfat" => ghostfs_core::FileSystemType::ExFat,
                    _ => {
                        eprintln!("Unsupported filesystem type: {}", fs_str);
                        std::process::exit(1);
                    }
                };

                // Get XFS config - only prompt interactively if stdin is a TTY and not --no-interactive
                let interactive = !no_interactive && atty::is(atty::Stream::Stdin);
                let xfs_config = if fs_type == ghostfs_core::FileSystemType::Xfs {
                    get_xfs_config_for_scan(&image_path, interactive)?
                } else {
                    None
                };

                // Perform scan to identify recoverable files (auto-confidence)
                println!("Scanning for recoverable files...");
                let scanned_session = ghostfs_core::scan_and_analyze_with_config(&image_path, fs_type, xfs_config)?;
                let device = scanned_session.device_path.display().to_string();

                if scanned_session.metadata.recoverable_files == 0 {
                    println!("No recoverable files found (confidence >= 40%)");
                    return Ok(());
                }

                println!(
                    "Found {} recoverable files",
                    scanned_session.metadata.recoverable_files
                );
                
                (scanned_session, device)
            };

            // Determine if forensics mode is enabled
            let use_forensics = forensics || audit || verify_hash || partial || reconstruct;

            if use_forensics {
                println!("\n🔒 Forensics mode enabled:");
                if forensics || audit {
                    println!("   • Audit trail logging");
                }
                if forensics || verify_hash {
                    let algo = match hash_algorithm.as_str() {
                        "sha256" => ghostfs_core::HashAlgorithm::SHA256,
                        "sha512" => ghostfs_core::HashAlgorithm::SHA512,
                        "sha1" => ghostfs_core::HashAlgorithm::SHA1,
                        "md5" => ghostfs_core::HashAlgorithm::MD5,
                        _ => {
                            eprintln!("Invalid hash algorithm: {}", hash_algorithm);
                            std::process::exit(1);
                        }
                    };
                    println!("   • Hash verification ({})", hash_algorithm.to_uppercase());
                }
                if partial {
                    println!("   • Partial file recovery");
                }
                if reconstruct {
                    println!("   • Smart extent reconstruction");
                }
                println!();
            }

            // Perform recovery with or without forensics
            println!("Starting file recovery...");

            let file_ids_u64: Option<Vec<u64>> = ids.as_ref().map(|ids_vec| {
                ids_vec.iter().filter_map(|id| id.parse().ok()).collect()
            });

            if use_forensics {
                // Build forensics config
                let mut config = if forensics {
                    ghostfs_core::ForensicsConfig::full_forensics(&out)
                } else {
                    ghostfs_core::ForensicsConfig::default()
                };

                if !forensics {
                    // Apply individual flags
                    if audit {
                        config.enable_audit = true;
                        config.audit_log_path = Some(out.join("audit.jsonl"));
                    }
                    if verify_hash {
                        config.enable_hash_verification = true;
                        config.hash_algorithm = match hash_algorithm.as_str() {
                            "sha256" => ghostfs_core::HashAlgorithm::SHA256,
                            "sha512" => ghostfs_core::HashAlgorithm::SHA512,
                            "sha1" => ghostfs_core::HashAlgorithm::SHA1,
                            "md5" => ghostfs_core::HashAlgorithm::MD5,
                            _ => ghostfs_core::HashAlgorithm::SHA256,
                        };
                        config.manifest_path = Some(out.join("hash_manifest.json"));
                    }
                    if partial {
                        config.enable_partial_recovery = true;
                    }
                    if reconstruct {
                        config.enable_extent_reconstruction = true;
                    }
                }

                // Forensics recovery - use device path from session
                let device_path_buf = PathBuf::from(&device_path);
                let forensics_report = ghostfs_core::recover_files_with_forensics(
                    &device_path_buf,
                    &session,
                    &out,
                    file_ids_u64,
                    config,
                )?;

                let recovery_report = forensics_report.report;

                // Display recovery results
                println!("\n🎉 Recovery Report:");
                println!(
                    "Successfully recovered: {}",
                    recovery_report.recovered_files
                );
                println!("Failed recoveries: {}", recovery_report.failed_files);
                println!("Total files processed: {}", recovery_report.total_files);

                if forensics_report.partial_recoveries > 0 {
                    println!(
                        "Partial recoveries: {} files",
                        forensics_report.partial_recoveries
                    );
                }

                if forensics_report.extent_reconstructions > 0 {
                    println!(
                        "Extent reconstructions: {} files",
                        forensics_report.extent_reconstructions
                    );
                }

                if let Some(ref audit_path) = forensics_report.audit_log_path {
                    println!("\n📝 Audit trail: {}", audit_path.display());
                }

                if let Some(ref manifest_path) = forensics_report.manifest_path {
                    println!("🔐 Hash manifest: {}", manifest_path.display());
                }

                if !recovery_report.recovery_details.is_empty() {
                    println!("\nDetailed Results:");
                    for result in &recovery_report.recovery_details {
                        match &result.status {
                            ghostfs_core::RecoveryStatus::Success => {
                                println!(
                                    "  ✅ {} -> {}",
                                    result.file_id,
                                    result.recovered_path.display()
                                );
                            }
                            ghostfs_core::RecoveryStatus::Failed(error) => {
                                println!("  ❌ {} -> Failed: {}", result.file_id, error);
                            }
                        }
                    }
                }

                if recovery_report.recovered_files > 0 {
                    println!("\n✨ Recovery completed! Files saved to: {}", out.display());
                }
            } else {
                // Standard recovery (no forensics) - use device path from session
                let device_path_buf = PathBuf::from(&device_path);
                let recovery_report = ghostfs_core::recover_files(
                    &device_path_buf,
                    &session,
                    &out,
                    file_ids_u64,
                )?;

                // Display recovery results
                println!("\nRecovery Report:");
                println!(
                    "Successfully recovered: {}",
                    recovery_report.recovered_files
                );
                println!("Failed recoveries: {}", recovery_report.failed_files);
                println!("Total files processed: {}", recovery_report.total_files);

                if !recovery_report.recovery_details.is_empty() {
                    println!("\nDetailed Results:");
                    for result in &recovery_report.recovery_details {
                        match &result.status {
                            ghostfs_core::RecoveryStatus::Success => {
                                println!(
                                    "  {} -> {}",
                                    result.file_id,
                                    result.recovered_path.display()
                                );
                            }
                            ghostfs_core::RecoveryStatus::Failed(error) => {
                                println!("  {} -> Failed: {}", result.file_id, error);
                            }
                        }
                    }
                }

                if recovery_report.recovered_files > 0 {
                    println!("Recovery completed! Files saved to: {}", out.display());
                }
            }
        }
        Commands::Timeline {
            image,
            fs,
            json,
            csv,
        } => {
            println!("📅 Generating Recovery Timeline...\n");

            let fs_type = match fs.as_str() {
                "xfs" => FileSystemType::Xfs,
                "btrfs" => FileSystemType::Btrfs,
                "exfat" => FileSystemType::ExFat,
                _ => unreachable!(),
            };

            // Perform scan to get recovery session
            println!("🔍 Scanning {} filesystem...", fs_type);
            let session = match ghostfs_core::scan_and_analyze(&image, fs_type) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("❌ Failed to scan image: {}", e);
                    return Err(e);
                }
            };

            println!(
                "✅ Scan complete: {} files found\n",
                session.scan_results.len()
            );

            // Generate timeline
            use ghostfs_core::RecoveryTimeline;
            let timeline = RecoveryTimeline::from_session(&session);

            // Display text report
            println!("{}", timeline.to_text_report());

            // Export to JSON if requested
            if let Some(ref json_path) = json {
                match timeline.to_json() {
                    Ok(json_data) => {
                        if let Err(e) = std::fs::write(json_path, json_data) {
                            eprintln!("⚠️  Failed to write JSON file: {}", e);
                        } else {
                            println!("\n💾 Timeline saved to: {}", json_path.display());
                        }
                    }
                    Err(e) => {
                        eprintln!("⚠️  Failed to serialize timeline to JSON: {}", e);
                    }
                }
            }

            // Export to CSV if requested
            if let Some(ref csv_path) = csv {
                let csv_data = timeline.to_csv();
                if let Err(e) = std::fs::write(csv_path, csv_data) {
                    eprintln!("⚠️  Failed to write CSV file: {}", e);
                } else {
                    println!("💾 Timeline saved to: {}", csv_path.display());
                }
            }

            // Provide helpful next steps
            if !timeline.events.is_empty() {
                println!("\n💡 Next Steps:");
                println!("   • Use 'ghostfs recover' to restore files");
                if !timeline.patterns.is_empty() {
                    println!("   • Review suspicious patterns above for forensic analysis");
                }
                if json.is_none() && csv.is_none() {
                    println!("   • Add --json or --csv flags to export timeline data");
                }
            }
        }
        Commands::Sessions { action } => {
            let manager = SessionManager::new()?;

            match action {
                SessionAction::List { fs, device } => {
                    let fs_type = fs.as_ref().map(|fs_str| match fs_str.as_str() {
                        "xfs" => FileSystemType::Xfs,
                        "btrfs" => FileSystemType::Btrfs,
                        "exfat" => FileSystemType::ExFat,
                        _ => unreachable!(),
                    });

                    let sessions = if let Some(ref dev) = device {
                        manager.list_sessions_by_device(dev)?
                    } else if let Some(fs_t) = fs_type {
                        manager.list_sessions_by_fs(fs_t)?
                    } else {
                        manager.list()?
                    };

                    if sessions.is_empty() {
                        println!("📭 No saved sessions found");
                        println!("\n💡 Save a session with: ghostfs scan disk.img --fs xfs --save");
                        return Ok(());
                    }

                    println!("📂 Saved Sessions ({} total)\n", sessions.len());
                    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

                    for session in sessions {
                        let short_id = &session.id.to_string()[..8];
                        let size_mb = session.device_size / (1024 * 1024);
                        
                        println!("\n📋 ID: {}", short_id);
                        println!("   FS: {} | Device: {}", session.fs_type, session.device_path.display());
                        println!("   Size: {} MB | Files: {} | Recoverable: {}", 
                            size_mb, session.files_found, session.recoverable_files);
                        println!("   Created: {}", session.created_at);
                    }

                    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                    println!("\n💡 Use 'ghostfs sessions info <id>' for detailed information");
                    println!("💡 Use 'ghostfs recover --session <id> --out ./recovered' to recover files");
                }
                SessionAction::Info { id } => {
                    let session = manager.load(&id)?;
                    let short_id = &session.id.to_string()[..8];

                    println!("📋 Session Details\n");
                    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                    println!("\nID: {}", short_id);
                    println!("Full ID: {}", session.id);
                    println!("File System: {}", session.fs_type);
                    println!("Device: {}", session.device_path.display());
                    println!("Device Size: {} MB", session.metadata.device_size / (1024 * 1024));
                    println!("Created: {}", session.created_at);
                    println!("\nScan Results:");
                    println!("  • Total Files Found: {}", session.metadata.files_found);
                    println!("  • Recoverable Files: {}", session.metadata.recoverable_files);
                    println!("  • Files in Session: {}", session.scan_results.len());

                    if !session.scan_results.is_empty() {
                        println!("\nTop Recoverable Files:");
                        let mut sorted_files = session.scan_results.clone();
                        sorted_files.sort_by(|a, b| b.confidence_score.partial_cmp(&a.confidence_score).unwrap());
                        
                        for (i, file) in sorted_files.iter().take(10).enumerate() {
                            let path_str = file.original_path.as_ref()
                                .map(|p| p.display().to_string())
                                .unwrap_or_else(|| format!("inode_{}", file.inode_or_cluster));
                            
                            let confidence_icon = if file.confidence_score >= 0.8 {
                                "✅"
                            } else if file.confidence_score >= 0.6 {
                                "🟡"
                            } else if file.confidence_score >= 0.4 {
                                "🟠"
                            } else {
                                "❌"
                            };

                            println!("  {}. {} {} ({:.1}%) - {} bytes", 
                                i + 1, confidence_icon, path_str, 
                                file.confidence_score * 100.0, file.size);
                        }

                        if sorted_files.len() > 10 {
                            println!("  ... and {} more files", sorted_files.len() - 10);
                        }
                    }

                    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                    println!("\n💡 Recover with: ghostfs recover --session {} --out ./recovered", short_id);
                }
                SessionAction::Delete { ids } => {
                    let mut deleted_count = 0;
                    let mut failed_count = 0;

                    for id in ids {
                        match manager.delete(&id) {
                            Ok(_) => {
                                println!("✅ Deleted session: {}", id);
                                deleted_count += 1;
                            }
                            Err(e) => {
                                eprintln!("❌ Failed to delete session {}: {}", id, e);
                                failed_count += 1;
                            }
                        }
                    }

                    println!("\n📊 Summary: {} deleted, {} failed", deleted_count, failed_count);
                }
                SessionAction::Cleanup { days } => {
                    println!("🧹 Cleaning up sessions older than {} days...", days);
                    let deleted = manager.cleanup(days)?;
                    
                    if deleted == 0 {
                        println!("✨ No old sessions found - database is clean!");
                    } else {
                        println!("✅ Cleaned up {} old session(s)", deleted);
                    }
                }
            }
        }
    }
    Ok(())
}
