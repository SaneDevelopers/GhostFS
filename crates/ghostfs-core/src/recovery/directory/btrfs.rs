//! Btrfs directory reconstruction
//!
//! Parses Btrfs B-tree structures to recover original filenames and paths.
//! Uses DIR_ITEM (type 84), DIR_INDEX (type 96), and INODE_REF (type 12) entries.

use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;

use super::{DirectoryReconstructor, ReconstructionStats};
use crate::fs::common::BlockDevice;

// Btrfs file types
const BTRFS_FT_REG_FILE: u8 = 1;
#[cfg(test)]
const BTRFS_FT_DIR: u8 = 2;

/// Btrfs directory entry information
#[derive(Debug, Clone)]
pub struct BtrfsDirEntry {
    /// Inode number
    pub inode: u64,
    /// Filename
    pub name: String,
    /// File type
    pub file_type: u8,
    /// Parent inode
    pub parent_inode: u64,
}

/// Btrfs directory reconstructor
pub struct BtrfsDirReconstructor {
    /// Map: inode -> entry info
    entries: HashMap<u64, BtrfsDirEntry>,
    /// Map: inode -> reconstructed path
    paths: HashMap<u64, PathBuf>,
    /// Root inode (usually 256)
    root_inode: u64,
}

impl BtrfsDirReconstructor {
    /// Create a new Btrfs directory reconstructor
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            paths: HashMap::new(),
            root_inode: 256, // FS_TREE_OBJECTID
        }
    }

    /// Add a directory entry
    pub fn add_entry(&mut self, entry: BtrfsDirEntry) {
        self.entries.insert(entry.inode, entry);
    }

    /// Parse DIR_ITEM data from B-tree
    pub fn parse_dir_item(&self, parent_inode: u64, data: &[u8]) -> Option<BtrfsDirEntry> {
        if data.len() < 30 {
            return None;
        }

        // Child location (first 8 bytes is child inode)
        let child_inode = u64::from_le_bytes([
            data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
        ]);

        // Skip: type (1), offset (8), transid (8), data_len (2)
        // Name length at offset 27-28
        if data.len() < 30 {
            return None;
        }

        let name_len = u16::from_le_bytes([data[27], data[28]]) as usize;
        let file_type = data[29];

        if data.len() < 30 + name_len {
            return None;
        }

        let name_bytes = &data[30..30 + name_len];
        let name = String::from_utf8_lossy(name_bytes).to_string();

        // Skip . and ..
        if name == "." || name == ".." {
            return None;
        }

        Some(BtrfsDirEntry {
            inode: child_inode,
            name,
            file_type,
            parent_inode,
        })
    }

    /// Parse INODE_REF data (contains parent inode and name)
    pub fn parse_inode_ref(
        &self,
        child_inode: u64,
        parent_inode: u64,
        data: &[u8],
    ) -> Option<BtrfsDirEntry> {
        if data.len() < 10 {
            return None;
        }

        // Index (8 bytes) + name_len (2 bytes) + name
        let name_len = u16::from_le_bytes([data[8], data[9]]) as usize;

        if data.len() < 10 + name_len {
            return None;
        }

        let name_bytes = &data[10..10 + name_len];
        let name = String::from_utf8_lossy(name_bytes).to_string();

        Some(BtrfsDirEntry {
            inode: child_inode,
            name,
            file_type: BTRFS_FT_REG_FILE, // Will be updated if we find more info
            parent_inode,
        })
    }

    /// Build path by walking parent chain
    fn build_path(&mut self, inode: u64, max_depth: u32) -> Option<PathBuf> {
        // Check cache
        if let Some(path) = self.paths.get(&inode) {
            return Some(path.clone());
        }

        // Prevent infinite loops
        if max_depth == 0 {
            return None;
        }

        // Check if at root
        if inode == self.root_inode {
            let path = PathBuf::from("/");
            self.paths.insert(inode, path.clone());
            return Some(path);
        }

        let entry = self.entries.get(&inode)?.clone();

        // Recurse to parent
        if entry.parent_inode != 0 && entry.parent_inode != inode {
            if let Some(parent_path) = self.build_path(entry.parent_inode, max_depth - 1) {
                let path = parent_path.join(&entry.name);
                self.paths.insert(inode, path.clone());
                return Some(path);
            }
        }

        // Fallback to just filename
        let path = PathBuf::from(&entry.name);
        self.paths.insert(inode, path.clone());
        Some(path)
    }
}

impl Default for BtrfsDirReconstructor {
    fn default() -> Self {
        Self::new()
    }
}

impl DirectoryReconstructor for BtrfsDirReconstructor {
    fn scan_directories(&mut self, _device: &BlockDevice) -> Result<usize> {
        tracing::info!("🔍 Scanning Btrfs B-tree for directory entries");

        // Note: Actual B-tree traversal is done in the recovery engine
        // This reconstructor is populated by the recovery engine as it traverses

        tracing::info!(
            "✅ Btrfs directory reconstructor ready ({} entries)",
            self.entries.len()
        );
        Ok(self.entries.len())
    }

    fn reconstruct_path(&mut self, inode: u64) -> Option<PathBuf> {
        self.build_path(inode, 100)
    }

    fn get_filename(&self, inode: u64) -> Option<String> {
        self.entries.get(&inode).map(|e| e.name.clone())
    }

    fn stats(&self) -> ReconstructionStats {
        ReconstructionStats {
            total_entries: self.entries.len(),
            paths_reconstructed: self.paths.len(),
            root_id: Some(self.root_inode),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_btrfs_reconstructor_creation() {
        let reconstructor = BtrfsDirReconstructor::new();
        assert_eq!(reconstructor.root_inode, 256);
    }

    #[test]
    fn test_btrfs_path_reconstruction() {
        let mut reconstructor = BtrfsDirReconstructor::new();

        // Add entries: /home (inode 100) -> /home/user (inode 200) -> /home/user/file.txt (inode 300)
        reconstructor.add_entry(BtrfsDirEntry {
            inode: 100,
            name: "home".to_string(),
            file_type: BTRFS_FT_DIR,
            parent_inode: 256,
        });

        reconstructor.add_entry(BtrfsDirEntry {
            inode: 200,
            name: "user".to_string(),
            file_type: BTRFS_FT_DIR,
            parent_inode: 100,
        });

        reconstructor.add_entry(BtrfsDirEntry {
            inode: 300,
            name: "file.txt".to_string(),
            file_type: BTRFS_FT_REG_FILE,
            parent_inode: 200,
        });

        let path = reconstructor.reconstruct_path(300);
        assert_eq!(path, Some(PathBuf::from("/home/user/file.txt")));
    }
}
