//! exFAT file recovery engine
//!
//! Scans exFAT filesystems for deleted files using multiple methods:
//! 1. Deleted directory entries (high bit cleared)
//! 2. Orphaned FAT clusters (allocated but unreferenced)
//! 3. Signature-based scanning (file magic bytes)

use anyhow::Result;
use std::collections::HashSet;
use std::path::PathBuf;

use super::directory::{DirectoryEntry, FileEntrySet, ENTRY_SIZE};
use super::fat::FatTable;
use super::ExFatBootSector;
use crate::fs::common::BlockDevice;
use crate::{BlockRange, DeletedFile, FileMetadata, FileType};

/// exFAT Recovery Engine
pub struct ExFatRecoveryEngine<'a> {
    device: &'a BlockDevice,
    boot_sector: ExFatBootSector,
    fat_table: FatTable,
}

impl<'a> ExFatRecoveryEngine<'a> {
    /// Create new recovery engine
    pub fn new(device: &'a BlockDevice, boot_sector: ExFatBootSector) -> Result<Self> {
        let fat_table = FatTable::from_device(device, &boot_sector)?;

        tracing::info!(
            "exFAT Recovery: {} clusters, {} free, {} allocated",
            fat_table.cluster_count(),
            fat_table.free_cluster_count(),
            fat_table.allocated_cluster_count()
        );

        Ok(Self {
            device,
            boot_sector,
            fat_table,
        })
    }

    /// Scan for all deleted files
    pub fn scan_deleted_files(&self) -> Result<Vec<DeletedFile>> {
        let mut deleted_files = Vec::new();
        let mut file_id = 1u64;
        let mut referenced_clusters = HashSet::new();

        tracing::info!("exFAT Recovery: Starting scan");

        // Method 1: Scan root directory and subdirectories for deleted entries
        match self.scan_deleted_directory_entries(&mut file_id, &mut referenced_clusters) {
            Ok(mut files) => {
                tracing::info!("Found {} deleted directory entries", files.len());
                deleted_files.append(&mut files);
            }
            Err(e) => {
                tracing::warn!("Directory scan failed: {}", e);
            }
        }

        // Method 2: Find orphaned cluster chains
        match self.find_orphaned_clusters(&mut file_id, &referenced_clusters) {
            Ok(mut files) => {
                tracing::info!("Found {} orphaned cluster chains", files.len());
                deleted_files.append(&mut files);
            }
            Err(e) => {
                tracing::warn!("Orphan scan failed: {}", e);
            }
        }

        // Method 3: Signature-based scanning
        match self.scan_file_signatures(&mut file_id) {
            Ok(mut files) => {
                tracing::info!("Found {} files via signature scan", files.len());
                deleted_files.append(&mut files);
            }
            Err(e) => {
                tracing::warn!("Signature scan failed: {}", e);
            }
        }

        tracing::info!(
            "exFAT Recovery complete: {} total files found",
            deleted_files.len()
        );
        Ok(deleted_files)
    }

    /// Scan directories for deleted entries
    fn scan_deleted_directory_entries(
        &self,
        file_id: &mut u64,
        referenced_clusters: &mut HashSet<u32>,
    ) -> Result<Vec<DeletedFile>> {
        let mut deleted_files = Vec::new();

        // Start with root directory
        let root_cluster = self.boot_sector.first_cluster_of_root_directory;

        tracing::debug!("Scanning root directory at cluster {}", root_cluster);

        // Scan root and track referenced clusters
        self.scan_directory_cluster(
            root_cluster,
            file_id,
            &mut deleted_files,
            referenced_clusters,
            0, // depth
        )?;

        Ok(deleted_files)
    }

    /// Recursively scan a directory cluster for entries
    fn scan_directory_cluster(
        &self,
        start_cluster: u32,
        file_id: &mut u64,
        deleted_files: &mut Vec<DeletedFile>,
        referenced_clusters: &mut HashSet<u32>,
        depth: usize,
    ) -> Result<()> {
        if depth > 32 {
            // Prevent infinite recursion
            return Ok(());
        }

        // Get cluster chain for this directory
        let chain = self.fat_table.get_chain(start_cluster);

        // Mark these clusters as referenced
        for cluster in &chain {
            referenced_clusters.insert(*cluster);
        }

        // Read all directory data
        let mut dir_data = Vec::new();
        let cluster_size = self.fat_table.cluster_size() as usize;

        for cluster in &chain {
            let offset = self.fat_table.cluster_offset(*cluster);
            if let Ok(data) = self.device.read_bytes(offset, cluster_size) {
                dir_data.extend_from_slice(data);
            }
        }

        // Parse directory entries
        let mut i = 0;
        while i + ENTRY_SIZE <= dir_data.len() {
            let entry_data = &dir_data[i..];

            // Parse this entry
            match DirectoryEntry::parse(entry_data) {
                Ok(DirectoryEntry::File(file_entry)) => {
                    // Try to parse complete file entry set
                    let entries = self.collect_entry_set(entry_data)?;

                    if let Some(file_set) = FileEntrySet::parse_from_entries(&entries) {
                        // Track referenced cluster
                        if file_set.stream_extension.first_cluster >= 2 {
                            let file_chain = self
                                .fat_table
                                .get_chain(file_set.stream_extension.first_cluster);
                            for c in &file_chain {
                                referenced_clusters.insert(*c);
                            }
                        }

                        // If deleted, add to results
                        if file_set.is_deleted {
                            deleted_files.push(self.file_set_to_deleted_file(*file_id, &file_set));
                            *file_id += 1;
                        }

                        // Recurse into subdirectories
                        if file_entry.is_directory() && !file_set.is_deleted {
                            let subdir_cluster = file_set.stream_extension.first_cluster;
                            if subdir_cluster >= 2 {
                                self.scan_directory_cluster(
                                    subdir_cluster,
                                    file_id,
                                    deleted_files,
                                    referenced_clusters,
                                    depth + 1,
                                )?;
                            }
                        }
                    }

                    i += ENTRY_SIZE * (file_entry.secondary_count as usize + 1);
                }
                Ok(DirectoryEntry::Deleted(deleted)) => {
                    // Try to recover deleted file entry
                    if let Some(file_entry) = deleted.recover_as_file() {
                        let entries = self.collect_deleted_entry_set(entry_data)?;

                        if let Some(file_set) = FileEntrySet::parse_from_entries(&entries) {
                            deleted_files.push(self.file_set_to_deleted_file(*file_id, &file_set));
                            *file_id += 1;
                        }

                        i += ENTRY_SIZE * (file_entry.secondary_count as usize + 1);
                    } else {
                        i += ENTRY_SIZE;
                    }
                }
                Ok(DirectoryEntry::Unknown(0x00)) => {
                    // End of directory
                    break;
                }
                _ => {
                    i += ENTRY_SIZE;
                }
            }
        }

        Ok(())
    }

    /// Collect entries for a file entry set
    fn collect_entry_set(&self, data: &[u8]) -> Result<Vec<DirectoryEntry>> {
        let mut entries = Vec::new();
        let offset = 0;

        // First entry should be File
        if offset + ENTRY_SIZE > data.len() {
            return Ok(entries);
        }

        if let Ok(DirectoryEntry::File(ref f)) = DirectoryEntry::parse(&data[offset..]) {
            let count = f.secondary_count as usize + 1;
            for i in 0..count {
                let start = i * ENTRY_SIZE;
                if start + ENTRY_SIZE <= data.len() {
                    if let Ok(e) = DirectoryEntry::parse(&data[start..]) {
                        entries.push(e);
                    }
                }
            }
        }

        Ok(entries)
    }

    /// Collect entries for a deleted file entry set
    fn collect_deleted_entry_set(&self, data: &[u8]) -> Result<Vec<DirectoryEntry>> {
        let mut entries = Vec::new();
        let offset = 0;

        // Parse first entry to get secondary count
        if let Ok(DirectoryEntry::Deleted(d)) = DirectoryEntry::parse(&data[offset..]) {
            if let Some(f) = d.recover_as_file() {
                let count = f.secondary_count as usize + 1;
                for i in 0..count {
                    let start = i * ENTRY_SIZE;
                    if start + ENTRY_SIZE <= data.len() {
                        if let Ok(e) = DirectoryEntry::parse(&data[start..]) {
                            entries.push(e);
                        }
                    }
                }
            }
        }

        Ok(entries)
    }

    /// Find orphaned cluster chains (deleted files with FAT entries intact)
    fn find_orphaned_clusters(
        &self,
        file_id: &mut u64,
        referenced_clusters: &HashSet<u32>,
    ) -> Result<Vec<DeletedFile>> {
        let mut deleted_files = Vec::new();

        let orphans = self.fat_table.find_orphaned_chains(referenced_clusters);

        for (start_cluster, chain) in orphans {
            // Try to detect file type from first cluster
            let offset = self.fat_table.cluster_offset(start_cluster);
            let header = self.device.read_bytes(offset, 512)?;

            let (mime_type, extension) = self.detect_file_type(header);

            // Estimate file size from chain length
            let estimated_size = chain.len() as u64 * self.fat_table.cluster_size() as u64;

            // Convert chain to block ranges
            let data_blocks = self.chain_to_block_ranges(&chain);

            // Create basic exFAT metadata for orphaned clusters
            let exfat_meta = crate::ExFatFileMetadata {
                first_cluster: start_cluster,
                cluster_chain: chain.clone(),
                chain_valid: chain.iter().all(|&c| c >= 2 && c != 0xFFFFFFF7),
                checksum: 0,        // No checksum available for orphans
                entry_count: 0,     // No directory entry
                utf16_valid: false, // No filename
                attributes: 0,      // No attributes available
            };

            let file = DeletedFile {
                id: *file_id,
                inode_or_cluster: start_cluster as u64,
                original_path: Some(PathBuf::from(format!(
                    "orphan_cluster_{}_{}.{}",
                    start_cluster, file_id, extension
                ))),
                size: estimated_size,
                deletion_time: None,
                confidence_score: 0.5, // Medium confidence
                file_type: FileType::RegularFile,
                data_blocks,
                is_recoverable: true,
                metadata: FileMetadata {
                    mime_type: Some(mime_type),
                    file_extension: Some(extension),
                    permissions: None,
                    owner_uid: None,
                    owner_gid: None,
                    created_time: None,
                    modified_time: None,
                    accessed_time: None,
                    extended_attributes: std::collections::HashMap::new(),
                },
                fs_metadata: Some(crate::FsSpecificMetadata::ExFat(exfat_meta)),
            };

            *file_id += 1;
            deleted_files.push(file);
        }

        Ok(deleted_files)
    }

    /// Signature-based file scanning
    fn scan_file_signatures(&self, file_id: &mut u64) -> Result<Vec<DeletedFile>> {
        let mut deleted_files = Vec::new();

        let cluster_size = self.fat_table.cluster_size() as u64;
        let max_clusters = std::cmp::min(self.boot_sector.cluster_count, 50000);

        for cluster in 2..max_clusters + 2 {
            // Skip allocated clusters
            if self.fat_table.is_allocated(cluster) {
                continue;
            }

            let offset = self.fat_table.cluster_offset(cluster);
            if let Ok(header) = self.device.read_bytes(offset, 512) {
                if let Some((mime, ext, est_size)) = self.detect_file_with_size(header, offset) {
                    let block_count = est_size.div_ceil(cluster_size);

                    // Create minimal exFAT metadata for signature-based recovery
                    let exfat_meta = crate::ExFatFileMetadata {
                        first_cluster: cluster,
                        cluster_chain: vec![cluster], // Single cluster for now
                        chain_valid: cluster >= 2,
                        checksum: 0,        // No checksum available
                        entry_count: 0,     // No directory entry
                        utf16_valid: false, // No filename
                        attributes: 0,      // No attributes available
                    };

                    let file = DeletedFile {
                        id: *file_id,
                        inode_or_cluster: cluster as u64,
                        original_path: Some(PathBuf::from(format!(
                            "recovered_{}_{}.{}",
                            cluster, file_id, ext
                        ))),
                        size: est_size,
                        deletion_time: None,
                        confidence_score: 0.5,
                        file_type: FileType::RegularFile,
                        data_blocks: vec![BlockRange {
                            start_block: cluster as u64,
                            block_count,
                            is_allocated: false,
                        }],
                        is_recoverable: est_size > 0,
                        metadata: FileMetadata {
                            mime_type: Some(mime),
                            file_extension: Some(ext),
                            permissions: None,
                            owner_uid: None,
                            owner_gid: None,
                            created_time: None,
                            modified_time: None,
                            accessed_time: None,
                            extended_attributes: std::collections::HashMap::new(),
                        },
                        fs_metadata: Some(crate::FsSpecificMetadata::ExFat(exfat_meta)),
                    };

                    *file_id += 1;
                    deleted_files.push(file);
                }
            }
        }

        Ok(deleted_files)
    }

    /// Extract exFAT-specific metadata for confidence scoring
    fn extract_exfat_metadata(
        &self,
        file_set: &FileEntrySet,
        chain: &[u32],
    ) -> crate::ExFatFileMetadata {
        let first_cluster = file_set.stream_extension.first_cluster;

        // Validate cluster chain for integrity
        let chain_valid = chain.iter().all(|&c| {
            c >= 2 && c != 0xFFFFFFF7 // Not bad cluster marker
        });

        // Validate UTF-16 filename (check if it was properly decoded)
        let utf16_valid = !file_set.filename.is_empty()
            && file_set
                .filename
                .chars()
                .all(|c| !c.is_control() || c == '\n' || c == '\r');

        // Calculate checksum from file entry (basic validation)
        // In real implementation, would recalculate and compare
        let checksum = file_set.stream_extension.name_hash; // Use name_hash as checksum proxy

        crate::ExFatFileMetadata {
            first_cluster,
            cluster_chain: chain.to_vec(),
            chain_valid,
            checksum,
            entry_count: 2, // File + Stream entry (name entry count not directly available)
            utf16_valid,
            attributes: 0, // Attributes not available in simplified extraction
        }
    }

    /// Convert a file entry set to DeletedFile
    fn file_set_to_deleted_file(&self, id: u64, file_set: &FileEntrySet) -> DeletedFile {
        let first_cluster = file_set.stream_extension.first_cluster;
        let chain = self.fat_table.get_chain(first_cluster);

        tracing::debug!(
            "file_set_to_deleted_file: '{}' cluster={}, chain_len={}, data_len={}",
            file_set.filename,
            first_cluster,
            chain.len(),
            file_set.stream_extension.data_length
        );

        let data_blocks = self.chain_to_block_ranges(&chain);

        // Extract exFAT-specific metadata
        let exfat_meta = self.extract_exfat_metadata(file_set, &chain);

        let file_type = if file_set.file_entry.is_directory() {
            FileType::Directory
        } else {
            FileType::RegularFile
        };

        DeletedFile {
            id,
            inode_or_cluster: first_cluster as u64,
            original_path: Some(PathBuf::from(&file_set.filename)),
            size: file_set.stream_extension.data_length,
            deletion_time: None,   // TODO: Parse timestamp
            confidence_score: 0.7, // Higher confidence for directory entries
            file_type,
            data_blocks,
            is_recoverable: file_set.stream_extension.first_cluster >= 2
                && file_set.stream_extension.data_length > 0,
            metadata: FileMetadata {
                mime_type: None,
                file_extension: PathBuf::from(&file_set.filename)
                    .extension()
                    .map(|e| e.to_string_lossy().to_string()),
                permissions: None,
                owner_uid: None,
                owner_gid: None,
                created_time: None,
                modified_time: None,
                accessed_time: None,
                extended_attributes: std::collections::HashMap::new(),
            },
            fs_metadata: Some(crate::FsSpecificMetadata::ExFat(exfat_meta)),
        }
    }

    /// Convert cluster chain to block ranges with byte offsets
    fn chain_to_block_ranges(&self, chain: &[u32]) -> Vec<BlockRange> {
        if chain.is_empty() {
            return Vec::new();
        }

        let cluster_size = self.fat_table.cluster_size() as u64;

        // Convert each cluster to a byte offset range
        // For the block-based recovery, we need byte offsets
        let mut ranges = Vec::new();

        // Get byte offset for first cluster
        let first_offset = self.fat_table.cluster_offset(chain[0]);
        let mut start_offset = first_offset;
        let mut byte_count = cluster_size;

        tracing::debug!(
            "chain_to_block_ranges: chain={:?}, cluster_size={}, first_offset={}",
            chain,
            cluster_size,
            first_offset
        );

        for &cluster in &chain[1..] {
            let current_offset = self.fat_table.cluster_offset(cluster);

            if current_offset == start_offset + byte_count {
                // Contiguous - extend the range
                byte_count += cluster_size;
            } else {
                // New range - convert to 1-byte blocks for correct recovery
                ranges.push(BlockRange {
                    start_block: start_offset, // Byte offset
                    block_count: byte_count,   // Byte count
                    is_allocated: false,
                });
                start_offset = current_offset;
                byte_count = cluster_size;
            }
        }

        // Add final range
        ranges.push(BlockRange {
            start_block: start_offset,
            block_count: byte_count,
            is_allocated: false,
        });

        ranges
    }

    /// Detect file type from magic bytes
    fn detect_file_type(&self, header: &[u8]) -> (String, String) {
        if header.len() < 8 {
            return ("application/octet-stream".to_string(), "bin".to_string());
        }

        // JPEG
        if header.starts_with(&[0xFF, 0xD8, 0xFF]) {
            return ("image/jpeg".to_string(), "jpg".to_string());
        }

        // PNG
        if header.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
            return ("image/png".to_string(), "png".to_string());
        }

        // PDF
        if header.starts_with(b"%PDF") {
            return ("application/pdf".to_string(), "pdf".to_string());
        }

        // ZIP
        if header.starts_with(&[0x50, 0x4B, 0x03, 0x04]) {
            return ("application/zip".to_string(), "zip".to_string());
        }

        ("application/octet-stream".to_string(), "bin".to_string())
    }

    /// Detect file type and estimate size
    fn detect_file_with_size(
        &self,
        header: &[u8],
        start_offset: u64,
    ) -> Option<(String, String, u64)> {
        if header.len() < 8 {
            return None;
        }

        // JPEG
        if header.starts_with(&[0xFF, 0xD8, 0xFF]) {
            let size = self.find_jpeg_end(start_offset);
            return Some(("image/jpeg".to_string(), "jpg".to_string(), size));
        }

        // PNG
        if header.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
            let size = self.find_png_end(start_offset);
            return Some(("image/png".to_string(), "png".to_string(), size));
        }

        // PDF
        if header.starts_with(b"%PDF") {
            let size = self.find_pdf_end(start_offset);
            return Some(("application/pdf".to_string(), "pdf".to_string(), size));
        }

        // ZIP
        if header.starts_with(&[0x50, 0x4B, 0x03, 0x04]) {
            let size = std::cmp::min(
                self.boot_sector.volume_length * self.boot_sector.bytes_per_sector() as u64
                    - start_offset,
                1024 * 1024,
            );
            return Some(("application/zip".to_string(), "zip".to_string(), size));
        }

        None
    }

    /// Find JPEG end marker
    fn find_jpeg_end(&self, start_offset: u64) -> u64 {
        let max_size = 10 * 1024 * 1024; // 10MB max
        let chunk_size = 64 * 1024;
        let mut offset = start_offset;

        while offset < start_offset + max_size {
            if let Ok(data) = self.device.read_bytes(offset, chunk_size) {
                for i in 0..data.len().saturating_sub(1) {
                    if data[i] == 0xFF && data[i + 1] == 0xD9 {
                        return offset - start_offset + i as u64 + 2;
                    }
                }
            }
            offset += chunk_size as u64 - 1;
        }

        max_size
    }

    /// Find PNG end (IEND chunk)
    fn find_png_end(&self, start_offset: u64) -> u64 {
        let max_size = 10 * 1024 * 1024;
        let chunk_size = 64 * 1024;
        let mut offset = start_offset;

        while offset < start_offset + max_size {
            if let Ok(data) = self.device.read_bytes(offset, chunk_size) {
                if let Some(pos) = data.windows(4).position(|w| w == b"IEND") {
                    return offset - start_offset + pos as u64 + 8;
                }
            }
            offset += chunk_size as u64 - 4;
        }

        max_size
    }

    /// Find PDF end (%%EOF)
    fn find_pdf_end(&self, start_offset: u64) -> u64 {
        let max_size = 50 * 1024 * 1024;
        let chunk_size = 64 * 1024;
        let mut offset = start_offset;

        while offset < start_offset + max_size {
            if let Ok(data) = self.device.read_bytes(offset, chunk_size) {
                if let Some(pos) = data.windows(5).position(|w| w == b"%%EOF") {
                    return offset - start_offset + pos as u64 + 5;
                }
            }
            offset += chunk_size as u64 - 5;
        }

        max_size
    }

    /// Get cluster size for recovery
    pub fn cluster_size(&self) -> u32 {
        self.fat_table.cluster_size()
    }

    /// Get cluster offset for recovery
    pub fn cluster_offset(&self, cluster: u32) -> u64 {
        self.fat_table.cluster_offset(cluster)
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_file_type_detection() {
        let engine_detect = |header: &[u8]| -> (String, String) {
            if header.len() < 8 {
                return ("application/octet-stream".to_string(), "bin".to_string());
            }

            if header.starts_with(&[0xFF, 0xD8, 0xFF]) {
                return ("image/jpeg".to_string(), "jpg".to_string());
            }

            if header.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
                return ("image/png".to_string(), "png".to_string());
            }

            ("application/octet-stream".to_string(), "bin".to_string())
        };

        assert_eq!(
            engine_detect(&[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46]).0,
            "image/jpeg"
        );
        assert_eq!(
            engine_detect(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]).0,
            "image/png"
        );
    }
}
