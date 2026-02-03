/// File system detection and module organization
use anyhow::Result;
use std::path::Path;

pub mod btrfs;
pub mod common;
pub mod exfat;
pub mod xfs;

use crate::FileSystemType;
use common::BlockDevice;

/// Detect file system type from device/image
pub fn detect_filesystem(device_path: &Path) -> Result<Option<FileSystemType>> {
    let device = BlockDevice::open(device_path)?;

    // Try to detect file system by reading superblocks at known locations

    // Check for XFS (superblock at sector 0)
    if let Ok(sector0) = device.read_sector(0) {
        if xfs::is_xfs_superblock(sector0) {
            return Ok(Some(FileSystemType::Xfs));
        }
    }

    // Check for Btrfs (superblock at 64KB)
    if let Ok(btrfs_sb) = device.read_bytes(65536, 4096) {
        if btrfs::is_btrfs_superblock(btrfs_sb) {
            return Ok(Some(FileSystemType::Btrfs));
        }
    }

    // Check for exFAT (boot sector at sector 0)
    if let Ok(sector0) = device.read_sector(0) {
        if exfat::is_exfat_boot_sector(sector0) {
            return Ok(Some(FileSystemType::ExFat));
        }
    }

    Ok(None)
}

/// Get human-readable file system information
pub fn get_filesystem_info(device_path: &Path, fs_type: FileSystemType) -> Result<String> {
    let device = BlockDevice::open(device_path)?;

    match fs_type {
        FileSystemType::Xfs => xfs::get_filesystem_info(&device),
        FileSystemType::Btrfs => btrfs::get_filesystem_info(&device),
        FileSystemType::ExFat => exfat::get_filesystem_info(&device),
    }
}
