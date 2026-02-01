use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use ghostfs_core::{scan_and_analyze, FileSystemType};

#[derive(Parser, Debug)]
#[command(name = "ghostfs", version, about = "GhostFS CLI - Professional Data Recovery Tool")]
struct Cli {
	#[command(subcommand)]
	command: Commands,
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
		/// Minimum confidence score (0.0-1.0)
		#[arg(long, default_value = "0.5")]
		confidence: f32,
		/// Show detailed filesystem information
		#[arg(long)]
		info: bool,
	},
	/// Detect filesystem type
	Detect {
		/// Path to image file
		image: PathBuf,
	},
	/// Recover files from an image
	Recover {
		/// Path to image file
		image: PathBuf,
		/// Filesystem type
		#[arg(long, value_parser = ["xfs", "btrfs", "exfat"], default_value = "xfs")]
		fs: String,
		/// Minimum confidence score (0.0-1.0)
		#[arg(long, default_value = "0.5")]
		confidence: f32,
		/// Output directory for recovered files
		#[arg(long)]
		out: PathBuf,
		/// File IDs to recover (if not specified, recovers all recoverable files)
		#[arg(long)]
		ids: Option<Vec<String>>,
	},
	/// Show a timeline (stub for now)
	Timeline,
}

fn main() -> Result<()> {
	// Initialize tracing
	tracing_subscriber::fmt::init();

	let cli = Cli::parse();
	match cli.command {
		Commands::Scan { image, fs, confidence, info } => {
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

			// Perform scan
			let session = scan_and_analyze(&image, fs_type, confidence)?;
			
			println!("✅ Scan completed successfully!");
			println!("📊 Session ID: {}", session.id);
			println!("📁 File System: {}", session.fs_type);
			println!("💾 Device Size: {} MB", session.metadata.device_size / (1024 * 1024));
			println!("🎯 Confidence Threshold: {:.1}%", confidence * 100.0);
			println!("📈 Files Found: {}", session.metadata.files_found);
			println!("🔄 Recoverable Files: {}", session.metadata.recoverable_files);
		}
		Commands::Detect { image } => {
			println!("🔍 Detecting file system type for: {}", image.display());
			
			match ghostfs_core::fs::detect_filesystem(&image)? {
				Some(fs_type) => {
					println!("✅ Detected: {}", fs_type);
					
					// Show basic info
					if let Ok(info) = ghostfs_core::fs::get_filesystem_info(&image, fs_type) {
						println!();
						println!("{}", info);
					}
				}
				None => {
					println!("❌ Unknown or unsupported file system");
				}
			}
		}
		Commands::Recover { image, fs, confidence, out, ids } => {
			println!("🔄 Starting recovery process for: {}", image.display());
			println!("📁 Output directory: {}", out.display());
			
			// Parse filesystem type
			let fs_type = match fs.as_str() {
				"xfs" => ghostfs_core::FileSystemType::Xfs,
				"btrfs" => ghostfs_core::FileSystemType::Btrfs,
				"exfat" => ghostfs_core::FileSystemType::ExFat,
				_ => {
					eprintln!("❌ Unsupported filesystem type: {}", fs);
					std::process::exit(1);
				}
			};

			// Validate confidence range
			if confidence < 0.0 || confidence > 1.0 {
				eprintln!("❌ Confidence must be between 0.0 and 1.0");
				std::process::exit(1);
			}

			// Create output directory if it doesn't exist
			std::fs::create_dir_all(&out)?;

			// First perform scan to identify recoverable files
			println!("🔍 Scanning for recoverable files...");
			let session = scan_and_analyze(&image, fs_type, confidence)?;
			
			if session.metadata.recoverable_files == 0 {
				println!("❌ No recoverable files found with confidence >= {:.1}%", confidence * 100.0);
				return Ok(());
			}

			println!("✅ Found {} recoverable files", session.metadata.recoverable_files);
			
			// Perform recovery
			println!("🚀 Starting file recovery...");
			let recovery_report = match ids {
				Some(file_ids) => {
					println!("📝 Recovering specific files: {:?}", file_ids);
					// Convert String IDs to u64 IDs
					let file_ids_u64: Vec<u64> = file_ids.iter()
						.filter_map(|id| id.parse().ok())
						.collect();
					ghostfs_core::recover_files(&image, &session, &out, Some(file_ids_u64))?
				}
				None => {
					println!("📁 Recovering all recoverable files...");
					ghostfs_core::recover_files(&image, &session, &out, None)?
				}
			};

			// Display recovery results
			println!("\n📊 Recovery Report:");
			println!("✅ Successfully recovered: {}", recovery_report.recovered_files);
			println!("❌ Failed recoveries: {}", recovery_report.failed_files);
			println!("📁 Total files processed: {}", recovery_report.total_files);

			if !recovery_report.recovery_details.is_empty() {
				println!("\n📋 Detailed Results:");
				for result in &recovery_report.recovery_details {
					match &result.status {
						ghostfs_core::RecoveryStatus::Success => {
							println!("  ✅ {} -> {}", result.file_id, result.recovered_path.display());
						}
						ghostfs_core::RecoveryStatus::Failed(error) => {
							println!("  ❌ {} -> Failed: {}", result.file_id, error);
						}
					}
				}
			}

			if recovery_report.recovered_files > 0 {
				println!("\n🎉 Recovery completed! Files saved to: {}", out.display());
			}
		}
		Commands::Timeline => {
			println!("📅 Timeline analysis");
			println!("⚠️  Timeline functionality not yet implemented");
		}
	}
	Ok(())
}