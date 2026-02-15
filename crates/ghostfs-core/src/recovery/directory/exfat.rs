//! exFAT directory reconstruction
//!
//! Parses exFAT directory entries to recover original filenames and paths.
//! exFAT uses 32-byte directory entries with multi-entry file sets:
//! - File Entry (0x85): Contains attributes, timestamps
//! - Stream Extension (0xC0): Contains first cluster, file size
//! - Filename Entry (0xC1): Contains UTF-16 filename chunks (15 chars each)

use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;

use super::{DirectoryReconstructor, ReconstructionStats};
use crate::fs::common::BlockDevice;

// exFAT directory entry type codes
const EXFAT_TYPE_FILE: u8 = 0x85;
const EXFAT_TYPE_STREAM: u8 = 0xC0;
const EXFAT_TYPE_NAME: u8 = 0xC1;
const EXFAT_TYPE_FILE_DELETED: u8 = 0x05;

// File attributes
const ATTR_DIRECTORY: u16 = 0x0010;

/// exFAT directory entry information
#[derive(Debug, Clone)]
pub struct ExFatDirEntry {
    /// First cluster of the file/directory
    pub first_cluster: u64,
    /// Reconstructed filename
    pub name: String,
    /// Parent directory cluster (where this entry was found)
    pub parent_cluster: u64,
    /// Is this a directory?
    pub is_directory: bool,
    /// File size
    pub size: u64,
}

/// exFAT directory reconstructor
pub struct ExFatDirReconstructor {
    /// Map: cluster -> entry info
    entries: HashMap<u64, ExFatDirEntry>,
    /// Map: cluster -> reconstructed path
    paths: HashMap<u64, PathBuf>,
    /// Root directory cluster
    root_cluster: u64,
    /// Cluster size in bytes
    cluster_size: u32,
    /// Cluster heap offset in bytes
    cluster_heap_offset: u64,
}

impl ExFatDirReconstructor {
    /// Create a new exFAT directory reconstructor
    pub fn new(cluster_size: u32, root_cluster: u64, cluster_heap_offset: u64) -> Self {
        Self {
            entries: HashMap::new(),
            paths: HashMap::new(),
            root_cluster,
            cluster_size,
            cluster_heap_offset,
        }
    }

    /// Calculate byte offset for a cluster
    fn cluster_offset(&self, cluster: u64) -> u64 {
        self.cluster_heap_offset + (cluster.saturating_sub(2)) * self.cluster_size as u64
    }

    /// Parse a directory cluster and extract entries
    fn parse_directory_cluster(
        &self,
        cluster_data: &[u8],
        parent_cluster: u64,
    ) -> Vec<ExFatDirEntry> {
        let mut entries = Vec::new();
        let mut offset = 0;

        while offset + 32 <= cluster_data.len() {
            let entry_type = cluster_data[offset];

            // Check for file entry (start of multi-entry set)
            if entry_type == EXFAT_TYPE_FILE || entry_type == EXFAT_TYPE_FILE_DELETED {
                if let Some(entry) =
                    self.parse_file_entry_set(&cluster_data[offset..], parent_cluster)
                {
                    // Skip the entire entry set
                    let secondary_count = cluster_data[offset + 1] as usize;
                    offset += 32 * (1 + secondary_count);
                    entries.push(entry);
                    continue;
                }
            }

            offset += 32;
        }

        entries
    }

    /// Parse a complete file entry set (File + Stream + Name entries)
    fn parse_file_entry_set(&self, data: &[u8], parent_cluster: u64) -> Option<ExFatDirEntry> {
        // Minimum: File(32) + Stream(32) + Name(32) = 96 bytes
        if data.len() < 96 {
            return None;
        }

        let entry_type = data[0];

        // Deleted entries have 0x05 instead of 0x85
        let _is_deleted = entry_type == EXFAT_TYPE_FILE_DELETED;

        // File entry (offset 0)
        let secondary_count = data[1] as usize;
        if secondary_count < 2 || data.len() < 32 * (1 + secondary_count) {
            return None;
        }

        let attributes = u16::from_le_bytes([data[4], data[5]]);
        let is_directory = (attributes & ATTR_DIRECTORY) != 0;

        // Stream extension entry (offset 32)
        if data[32] != EXFAT_TYPE_STREAM && data[32] != 0x40 {
            return None; // Not a stream extension
        }

        let name_len = data[32 + 3] as usize; // Name length in characters
        let first_cluster =
            u32::from_le_bytes([data[32 + 20], data[32 + 21], data[32 + 22], data[32 + 23]]) as u64;
        let size = u64::from_le_bytes([
            data[32 + 24],
            data[32 + 25],
            data[32 + 26],
            data[32 + 27],
            data[32 + 28],
            data[32 + 29],
            data[32 + 30],
            data[32 + 31],
        ]);

        // Name entries (offset 64+)
        let mut name = String::new();
        for i in 0..(secondary_count.saturating_sub(1)) {
            let name_offset = 64 + i * 32;
            if name_offset + 32 > data.len() {
                break;
            }

            let name_type = data[name_offset];
            if name_type != EXFAT_TYPE_NAME && name_type != 0x41 {
                break; // Not a filename entry (0x41 is deleted filename)
            }

            // Extract UTF-16 characters (15 chars per entry, 2 bytes each)
            for j in 0..15 {
                if name.chars().count() >= name_len {
                    break;
                }

                let char_offset = name_offset + 2 + (j * 2);
                if char_offset + 1 >= data.len() {
                    break;
                }

                let char_code = u16::from_le_bytes([data[char_offset], data[char_offset + 1]]);
                if char_code == 0 {
                    break;
                }

                if let Some(ch) = char::from_u32(char_code as u32) {
                    name.push(ch);
                } else {
                    name.push('\u{FFFD}'); // Replacement character
                }
            }
        }

        if name.is_empty() || first_cluster == 0 {
            return None;
        }

        Some(ExFatDirEntry {
            first_cluster,
            name: name.trim_end_matches('\0').to_string(),
            parent_cluster,
            is_directory,
            size,
        })
    }

    /// Scan a directory cluster chain and all subdirectories
    fn scan_directory_chain(
        &mut self,
        device: &BlockDevice,
        start_cluster: u64,
        depth: usize,
    ) -> Result<()> {
        if depth > 32 {
            return Ok(()); // Prevent infinite recursion
        }

        // Read the directory cluster
        let offset = self.cluster_offset(start_cluster);
        let cluster_data = device.read_bytes(offset, self.cluster_size as usize)?;

        // Parse entries in this cluster
        let entries = self.parse_directory_cluster(cluster_data.as_ref(), start_cluster);

        // Process each entry
        for entry in entries {
            let cluster = entry.first_cluster;
            let is_dir = entry.is_directory;

            // Store the entry
            self.entries.insert(cluster, entry);

            // Recursively scan subdirectories
            if is_dir && cluster >= 2 {
                let _ = self.scan_directory_chain(device, cluster, depth + 1);
            }
        }

        Ok(())
    }

    /// Build path by walking parent chain
    fn build_path(&mut self, cluster: u64, max_depth: u32) -> Option<PathBuf> {
        // Check cache
        if let Some(path) = self.paths.get(&cluster) {
            return Some(path.clone());
        }

        // Prevent infinite loops
        if max_depth == 0 {
            return None;
        }

        let entry = self.entries.get(&cluster)?.clone();

        // Check if at root
        if cluster == self.root_cluster || entry.parent_cluster == self.root_cluster {
            let path = PathBuf::from(format!("/{}", entry.name));
            self.paths.insert(cluster, path.clone());
            return Some(path);
        }

        // Recurse to parent
        if entry.parent_cluster >= 2 && entry.parent_cluster != cluster {
            if let Some(parent_path) = self.build_path(entry.parent_cluster, max_depth - 1) {
                let path = parent_path.join(&entry.name);
                self.paths.insert(cluster, path.clone());
                return Some(path);
            }
        }

        // Fallback to relative path
        let path = PathBuf::from(&entry.name);
        self.paths.insert(cluster, path.clone());
        Some(path)
    }
}

impl DirectoryReconstructor for ExFatDirReconstructor {
    fn scan_directories(&mut self, device: &BlockDevice) -> Result<usize> {
        tracing::info!(
            "🔍 Scanning exFAT directories starting from cluster {}",
            self.root_cluster
        );

        // Start scanning from root directory
        self.scan_directory_chain(device, self.root_cluster, 0)?;

        tracing::info!("✅ Found {} exFAT directory entries", self.entries.len());
        Ok(self.entries.len())
    }

    fn reconstruct_path(&mut self, cluster: u64) -> Option<PathBuf> {
        self.build_path(cluster, 100)
    }

    fn get_filename(&self, cluster: u64) -> Option<String> {
        self.entries.get(&cluster).map(|e| e.name.clone())
    }

    fn stats(&self) -> ReconstructionStats {
        ReconstructionStats {
            total_entries: self.entries.len(),
            paths_reconstructed: self.paths.len(),
            root_id: Some(self.root_cluster),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exfat_entry_constants() {
        assert_eq!(EXFAT_TYPE_FILE, 0x85);
        assert_eq!(EXFAT_TYPE_STREAM, 0xC0);
        assert_eq!(EXFAT_TYPE_NAME, 0xC1);
    }

    #[test]
    fn test_cluster_offset_calculation() {
        let reconstructor = ExFatDirReconstructor::new(4096, 5, 1048576);

        // Cluster 2 should be at heap offset
        assert_eq!(reconstructor.cluster_offset(2), 1048576);

        // Cluster 3 should be at heap offset + cluster_size
        assert_eq!(reconstructor.cluster_offset(3), 1048576 + 4096);
    }
}
