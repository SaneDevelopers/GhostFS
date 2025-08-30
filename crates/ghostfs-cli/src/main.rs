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
	/// Recover files (stub for now)
	Recover {
		#[arg(long)]
		out: PathBuf,
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
		Commands::Recover { out } => {
			println!("Recover to: {}", out.display());
			println!("⚠️  Recovery functionality not yet implemented");
		}
		Commands::Timeline => {
			println!("📅 Timeline analysis");
			println!("⚠️  Timeline functionality not yet implemented");
		}
	}
	Ok(())
}