//! exFAT File Allocation Table (FAT) parsing and chain traversal
//!
//! The FAT table maps cluster chains for files in exFAT.
//!
//! FAT Entry Values:
//! - 0x00000000: Free cluster
//! - 0x00000001: Reserved
//! - 0x00000002-0xFFFFFFF6: Next cluster in chain
//! - 0xFFFFFFF7: Bad cluster
//! - 0xFFFFFFF8-0xFFFFFFFF: End of chain

use anyhow::Result;
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Cursor;

use super::ExFatBootSector;
use crate::fs::common::BlockDevice;

/// Special FAT entry values
pub const FAT_ENTRY_FREE: u32 = 0x00000000;
pub const FAT_ENTRY_BAD: u32 = 0xFFFFFFF7;
pub const FAT_ENTRY_EOC_MIN: u32 = 0xFFFFFFF8; // End of chain minimum
pub const FAT_ENTRY_EOC_MAX: u32 = 0xFFFFFFFF; // End of chain maximum

/// exFAT File Allocation Table
#[derive(Debug)]
pub struct FatTable {
    /// FAT entries (one per cluster)
    entries: Vec<u32>,
    /// Bytes per cluster
    cluster_size: u32,
    /// Cluster heap offset in sectors
    cluster_heap_offset: u32,
    /// Bytes per sector
    bytes_per_sector: u32,
}

impl FatTable {
    /// Read FAT table from device
    pub fn from_device(device: &BlockDevice, boot: &ExFatBootSector) -> Result<Self> {
        let bytes_per_sector = boot.bytes_per_sector();
        let fat_offset_bytes = boot.fat_offset as u64 * bytes_per_sector as u64;
        let fat_size_bytes = boot.fat_length as u64 * bytes_per_sector as u64;

        tracing::debug!(
            "Reading FAT: offset=0x{:x}, size={} bytes",
            fat_offset_bytes,
            fat_size_bytes
        );

        let fat_data = device.read_bytes(fat_offset_bytes, fat_size_bytes as usize)?;

        // Parse FAT entries (4 bytes each)
        let mut entries = Vec::with_capacity(fat_data.len() / 4);
        let mut cursor = Cursor::new(&fat_data);

        while cursor.position() < fat_data.len() as u64 {
            match cursor.read_u32::<LittleEndian>() {
                Ok(entry) => entries.push(entry),
                Err(_) => break,
            }
        }

        tracing::debug!("Parsed {} FAT entries", entries.len());

        Ok(FatTable {
            entries,
            cluster_size: boot.bytes_per_cluster(),
            cluster_heap_offset: boot.cluster_heap_offset,
            bytes_per_sector,
        })
    }

    /// Check if a cluster is free
    pub fn is_free(&self, cluster: u32) -> bool {
        if cluster < 2 || cluster as usize >= self.entries.len() {
            return false;
        }
        self.entries[cluster as usize] == FAT_ENTRY_FREE
    }

    /// Check if a cluster is allocated (part of a chain)
    pub fn is_allocated(&self, cluster: u32) -> bool {
        if cluster < 2 || cluster as usize >= self.entries.len() {
            return false;
        }
        let entry = self.entries[cluster as usize];
        entry != FAT_ENTRY_FREE && entry != FAT_ENTRY_BAD
    }

    /// Check if entry is end of chain
    pub fn is_end_of_chain(&self, entry: u32) -> bool {
        entry >= FAT_ENTRY_EOC_MIN
    }

    /// Get the next cluster in chain (None if end of chain or invalid)
    pub fn next_cluster(&self, cluster: u32) -> Option<u32> {
        if cluster < 2 || cluster as usize >= self.entries.len() {
            return None;
        }

        let next = self.entries[cluster as usize];

        if (2..FAT_ENTRY_BAD).contains(&next) {
            Some(next)
        } else {
            None // End of chain or bad cluster
        }
    }

    /// Get entire cluster chain starting from a cluster
    pub fn get_chain(&self, start_cluster: u32) -> Vec<u32> {
        let mut chain = Vec::new();
        let mut current = start_cluster;
        let mut visited = std::collections::HashSet::new();

        // Follow chain with loop detection
        while current >= 2 && (current as usize) < self.entries.len() {
            if !visited.insert(current) {
                tracing::warn!("FAT chain loop detected at cluster {}", current);
                break;
            }

            chain.push(current);

            let next = self.entries[current as usize];
            if self.is_end_of_chain(next) || next == FAT_ENTRY_FREE || next == FAT_ENTRY_BAD {
                break;
            }

            current = next;
        }

        chain
    }

    /// Get cluster size in bytes
    pub fn cluster_size(&self) -> u32 {
        self.cluster_size
    }

    /// Calculate byte offset for a cluster
    pub fn cluster_offset(&self, cluster: u32) -> u64 {
        // Cluster 2 is the first data cluster
        let cluster_heap_offset_bytes =
            self.cluster_heap_offset as u64 * self.bytes_per_sector as u64;
        cluster_heap_offset_bytes + ((cluster - 2) as u64 * self.cluster_size as u64)
    }

    /// Find orphaned cluster chains (allocated but not referenced by any directory entry)
    /// Returns list of (start_cluster, chain) for potential deleted files
    pub fn find_orphaned_chains(
        &self,
        referenced_clusters: &std::collections::HashSet<u32>,
    ) -> Vec<(u32, Vec<u32>)> {
        let mut orphans = Vec::new();
        let mut processed = std::collections::HashSet::new();

        for cluster in 2..self.entries.len() as u32 {
            // Skip already processed or referenced clusters
            if processed.contains(&cluster) || referenced_clusters.contains(&cluster) {
                continue;
            }

            // Check if this is an allocated cluster
            if !self.is_allocated(cluster) {
                continue;
            }

            // Get the chain starting from this cluster
            let chain = self.get_chain(cluster);

            if !chain.is_empty() {
                // Check if any cluster in chain is referenced
                let is_orphan = !chain.iter().any(|c| referenced_clusters.contains(c));

                if is_orphan {
                    orphans.push((cluster, chain.clone()));
                }

                // Mark all clusters in chain as processed
                for c in &chain {
                    processed.insert(*c);
                }
            }
        }

        tracing::info!("Found {} orphaned cluster chains", orphans.len());
        orphans
    }

    /// Get total cluster count
    pub fn cluster_count(&self) -> usize {
        self.entries.len()
    }

    /// Count free clusters
    pub fn free_cluster_count(&self) -> usize {
        self.entries
            .iter()
            .skip(2) // Skip first two reserved entries
            .filter(|&&e| e == FAT_ENTRY_FREE)
            .count()
    }

    /// Count allocated clusters
    pub fn allocated_cluster_count(&self) -> usize {
        self.entries
            .iter()
            .skip(2)
            .filter(|&&e| e != FAT_ENTRY_FREE && e != FAT_ENTRY_BAD)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fat_entry_constants() {
        // Verify FAT entry marker values
        assert_eq!(FAT_ENTRY_FREE, 0);
        assert_eq!(FAT_ENTRY_BAD, 0xFFFFFFF7);
        assert_eq!(FAT_ENTRY_EOC_MIN, 0xFFFFFFF8);
        assert_eq!(FAT_ENTRY_EOC_MAX, 0xFFFFFFFF);
    }

    #[test]
    fn test_is_end_of_chain() {
        let fat = FatTable {
            entries: vec![0; 10],
            cluster_size: 4096,
            cluster_heap_offset: 0,
            bytes_per_sector: 512,
        };

        assert!(fat.is_end_of_chain(0xFFFFFFFF));
        assert!(fat.is_end_of_chain(0xFFFFFFF8));
        assert!(!fat.is_end_of_chain(0x00000003));
        assert!(!fat.is_end_of_chain(0x00000000));
    }

    #[test]
    fn test_get_chain() {
        // Create a FAT with a simple chain: 2 -> 3 -> 4 -> EOC
        let mut entries = vec![0u32; 10];
        entries[2] = 3;
        entries[3] = 4;
        entries[4] = 0xFFFFFFFF; // End of chain

        let fat = FatTable {
            entries,
            cluster_size: 4096,
            cluster_heap_offset: 0,
            bytes_per_sector: 512,
        };

        let chain = fat.get_chain(2);
        assert_eq!(chain, vec![2, 3, 4]);
    }
}
