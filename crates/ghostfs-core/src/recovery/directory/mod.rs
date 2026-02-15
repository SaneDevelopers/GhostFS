//! Directory reconstruction module for recovering original filenames and paths
//!
//! Provides filesystem-specific implementations for scanning directory structures
//! and reconstructing full paths from inode/cluster numbers.

mod btrfs;
mod exfat;
mod xfs;

pub use btrfs::{BtrfsDirEntry, BtrfsDirReconstructor};
pub use exfat::{ExFatDirEntry, ExFatDirReconstructor};
pub use xfs::{XfsDirEntry, XfsDirReconstructor};

use crate::fs::common::BlockDevice;
use anyhow::Result;
use std::path::PathBuf;

/// Statistics from directory reconstruction
#[derive(Debug, Default)]
pub struct ReconstructionStats {
    /// Total directory entries found
    pub total_entries: usize,
    /// Number of paths successfully reconstructed
    pub paths_reconstructed: usize,
    /// Root inode/cluster if detected
    pub root_id: Option<u64>,
}

/// Common trait for directory reconstruction across all filesystems
pub trait DirectoryReconstructor {
    /// Scan the device for directory entries
    fn scan_directories(&mut self, device: &BlockDevice) -> Result<usize>;

    /// Reconstruct the full path for a given inode/cluster
    fn reconstruct_path(&mut self, id: u64) -> Option<PathBuf>;

    /// Get just the filename (without path) for a given inode/cluster
    fn get_filename(&self, id: u64) -> Option<String>;

    /// Get reconstruction statistics
    fn stats(&self) -> ReconstructionStats;
}
