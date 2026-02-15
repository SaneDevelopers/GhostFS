//! XFS directory reconstruction
//!
//! Parses XFS directory blocks to recover original filenames and paths.
//! XFS uses 3 directory formats:
//! - Short Form: Stored in inode (< ~156 bytes)
//! - Block Form: Single data block (< 64KB)
//! - Leaf/Node Form: B+tree structure (> 64KB)

use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;

use super::{DirectoryReconstructor, ReconstructionStats};
use crate::fs::common::BlockDevice;

// XFS directory block magic numbers
const XFS_DIR2_BLOCK_MAGIC: u32 = 0x58443244; // "XD2D"
const XFS_DIR3_BLOCK_MAGIC: u32 = 0x58443344; // "XD3D"

// XFS file types (v5 filesystems)
pub const XFS_DIR3_FT_UNKNOWN: u8 = 0;
#[cfg(test)]
const XFS_DIR3_FT_REG_FILE: u8 = 1;
pub const XFS_DIR3_FT_DIR: u8 = 2;

/// XFS directory entry information
#[derive(Debug, Clone)]
pub struct XfsDirEntry {
    /// Inode number this entry points to
    pub inode: u64,
    /// Filename (NOT including path)
    pub name: String,
    /// File type
    pub file_type: u8,
    /// Parent directory inode
    pub parent_inode: u64,
    /// Block number where this entry was found
    pub source_block: u64,
}

/// XFS directory reconstructor
pub struct XfsDirReconstructor {
    /// Map: inode -> entry info
    entries: HashMap<u64, XfsDirEntry>,
    /// Map: inode -> reconstructed path
    paths: HashMap<u64, PathBuf>,
    /// Block size
    block_size: u32,
    /// Root inode (usually 64 or 128)
    root_inode: Option<u64>,
}

impl XfsDirReconstructor {
    /// Create a new XFS directory reconstructor
    pub fn new(block_size: u32) -> Self {
        Self {
            entries: HashMap::new(),
            paths: HashMap::new(),
            block_size,
            root_inode: None,
        }
    }

    /// Add entries from parsing
    pub fn add_entries(&mut self, entries: Vec<XfsDirEntry>) {
        let is_empty = entries.is_empty();
        for entry in entries {
            self.entries.insert(entry.inode, entry);
        }
        // Auto-detect root after adding entries
        if self.root_inode.is_none() && !is_empty {
            self.detect_root_inode();
        }
    }

    /// Set the root inode manually (for testing or when known)
    pub fn set_root_inode(&mut self, inode: u64) {
        self.root_inode = Some(inode);
        tracing::debug!("🌳 Root inode set to: {}", inode);
    }

    /// Parse a single directory block
    pub fn parse_dir_block(
        &self,
        block_data: &[u8],
        block_number: u64,
    ) -> Result<Vec<XfsDirEntry>> {
        if block_data.len() < 16 {
            anyhow::bail!("Block too small for directory header");
        }

        // Check magic number
        let magic =
            u32::from_be_bytes([block_data[0], block_data[1], block_data[2], block_data[3]]);

        let is_v5 = match magic {
            XFS_DIR3_BLOCK_MAGIC => true,
            XFS_DIR2_BLOCK_MAGIC => false,
            _ => return Ok(Vec::new()), // Not a directory block
        };

        // Extract owner inode (parent directory)
        let owner_inode = if is_v5 && block_data.len() >= 16 {
            u64::from_be_bytes([
                block_data[8],
                block_data[9],
                block_data[10],
                block_data[11],
                block_data[12],
                block_data[13],
                block_data[14],
                block_data[15],
            ])
        } else {
            0
        };

        // Start parsing entries after header
        let mut entries = Vec::new();
        let header_size = if is_v5 { 64 } else { 16 };
        let mut offset = header_size;

        while offset + 11 < block_data.len() {
            // Read inode number (8 bytes, big-endian)
            if offset + 8 > block_data.len() {
                break;
            }

            let inode = u64::from_be_bytes([
                block_data[offset],
                block_data[offset + 1],
                block_data[offset + 2],
                block_data[offset + 3],
                block_data[offset + 4],
                block_data[offset + 5],
                block_data[offset + 6],
                block_data[offset + 7],
            ]);
            offset += 8;

            // Check for invalid inode (end of entries or free space)
            if inode == 0 || inode == 0xFFFFFFFFFFFFFFFF {
                break;
            }

            // Read name length (1 byte)
            if offset >= block_data.len() {
                break;
            }
            let namelen = block_data[offset] as usize;
            offset += 1;

            if namelen == 0 || namelen > 255 {
                break;
            }

            // Read filename
            if offset + namelen > block_data.len() {
                break;
            }

            let name_bytes = &block_data[offset..offset + namelen];
            let name = String::from_utf8_lossy(name_bytes).to_string();
            offset += namelen;

            // Read file type (v5 only)
            let ftype = if is_v5 && offset < block_data.len() {
                let ft = block_data[offset];
                offset += 1;
                ft
            } else {
                XFS_DIR3_FT_UNKNOWN
            };

            // Align to 8-byte boundary
            offset = (offset + 7) & !7;

            // Skip . and ..
            if name == "." || name == ".." {
                continue;
            }

            entries.push(XfsDirEntry {
                inode,
                name,
                file_type: ftype,
                parent_inode: owner_inode,
                source_block: block_number,
            });
        }

        Ok(entries)
    }

    /// Detect root inode by finding lowest directory inode
    fn detect_root_inode(&mut self) {
        // Collect all directory inodes
        let mut candidate_roots: Vec<u64> = self
            .entries
            .values()
            .filter(|e| e.file_type == XFS_DIR3_FT_DIR)
            .map(|e| e.inode)
            .collect();

        // Also check parent inodes that might be the root
        for entry in self.entries.values() {
            if entry.parent_inode > 0 {
                candidate_roots.push(entry.parent_inode);
            }
        }

        candidate_roots.sort();
        candidate_roots.dedup();

        // Common XFS root inodes: 64, 128
        for candidate in [64u64, 128u64] {
            if candidate_roots.contains(&candidate) {
                self.root_inode = Some(candidate);
                tracing::debug!("🌳 Detected XFS root inode: {}", candidate);
                return;
            }
        }

        // Fallback: use smallest inode
        if let Some(&root) = candidate_roots.first() {
            self.root_inode = Some(root);
            tracing::debug!("🌳 Using inode {} as root (lowest directory inode)", root);
        }
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

        let entry = self.entries.get(&inode)?.clone();

        // Check if at root
        if Some(inode) == self.root_inode {
            let path = PathBuf::from("/");
            self.paths.insert(inode, path.clone());
            return Some(path);
        }

        // Check if parent is root
        let parent_inode = entry.parent_inode;
        if Some(parent_inode) == self.root_inode {
            let path = PathBuf::from("/").join(&entry.name);
            self.paths.insert(inode, path.clone());
            return Some(path);
        }

        // Recurse to parent
        if parent_inode != 0 && parent_inode != inode {
            if let Some(parent_path) = self.build_path(parent_inode, max_depth - 1) {
                let path = parent_path.join(&entry.name);
                self.paths.insert(inode, path.clone());
                return Some(path);
            }
        }

        // Fallback to relative path
        let path = PathBuf::from(&entry.name);
        self.paths.insert(inode, path.clone());
        Some(path)
    }
}

impl DirectoryReconstructor for XfsDirReconstructor {
    fn scan_directories(&mut self, device: &BlockDevice) -> Result<usize> {
        tracing::info!("🔍 Scanning XFS blocks for directory entries");

        // Scan blocks looking for directory magic
        let max_blocks = std::cmp::min(device.size() / self.block_size as u64, 100000);

        for block_num in 0..max_blocks {
            let offset = block_num * self.block_size as u64;
            if let Ok(block_data) = device.read_bytes(offset, self.block_size as usize) {
                if let Ok(entries) = self.parse_dir_block(block_data.as_ref(), block_num) {
                    if !entries.is_empty() {
                        tracing::debug!(
                            "📂 Found {} entries in block {}",
                            entries.len(),
                            block_num
                        );
                        self.add_entries(entries);
                    }
                }
            }
        }

        // Detect root inode
        self.detect_root_inode();

        tracing::info!("✅ Found {} XFS directory entries", self.entries.len());
        Ok(self.entries.len())
    }

    fn reconstruct_path(&mut self, inode: u64) -> Option<PathBuf> {
        if self.root_inode.is_none() {
            self.detect_root_inode();
        }
        self.build_path(inode, 100)
    }

    fn get_filename(&self, inode: u64) -> Option<String> {
        self.entries.get(&inode).map(|e| e.name.clone())
    }

    fn stats(&self) -> ReconstructionStats {
        ReconstructionStats {
            total_entries: self.entries.len(),
            paths_reconstructed: self.paths.len(),
            root_id: self.root_inode,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xfs_magic_constants() {
        assert_eq!(XFS_DIR3_BLOCK_MAGIC, 0x58443344);
        assert_eq!(XFS_DIR2_BLOCK_MAGIC, 0x58443244);
    }

    #[test]
    fn test_xfs_path_reconstruction() {
        let mut reconstructor = XfsDirReconstructor::new(4096);
        reconstructor.root_inode = Some(64);

        // Add entries
        reconstructor.add_entries(vec![
            XfsDirEntry {
                inode: 100,
                name: "home".to_string(),
                file_type: XFS_DIR3_FT_DIR,
                parent_inode: 64,
                source_block: 0,
            },
            XfsDirEntry {
                inode: 200,
                name: "user".to_string(),
                file_type: XFS_DIR3_FT_DIR,
                parent_inode: 100,
                source_block: 1,
            },
            XfsDirEntry {
                inode: 300,
                name: "file.txt".to_string(),
                file_type: XFS_DIR3_FT_REG_FILE,
                parent_inode: 200,
                source_block: 2,
            },
        ]);

        let path = reconstructor.reconstruct_path(300);
        assert_eq!(path, Some(PathBuf::from("/home/user/file.txt")));
    }

    #[test]
    fn test_parse_v5_dir_block() {
        let reconstructor = XfsDirReconstructor::new(4096);

        // Construct a minimal v5 directory block
        let mut block = vec![0u8; 4096];

        // Header: magic
        block[0..4].copy_from_slice(&XFS_DIR3_BLOCK_MAGIC.to_be_bytes());
        // Owner inode at offset 8
        block[8..16].copy_from_slice(&123u64.to_be_bytes());

        // Entry at offset 64: inode=456, namelen=8, name="test.txt", ftype=1
        let offset = 64;
        block[offset..offset + 8].copy_from_slice(&456u64.to_be_bytes());
        block[offset + 8] = 8; // namelen
        block[offset + 9..offset + 17].copy_from_slice(b"test.txt");
        block[offset + 17] = XFS_DIR3_FT_REG_FILE;

        let entries = reconstructor.parse_dir_block(&block, 100).unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].inode, 456);
        assert_eq!(entries[0].name, "test.txt");
        assert_eq!(entries[0].file_type, XFS_DIR3_FT_REG_FILE);
        assert_eq!(entries[0].parent_inode, 123);
    }
}
