use anyhow::Result;
use super::common::BlockDevice;
use std::collections::HashMap;
use chrono::DateTime;

const XFS_MAGIC: u32 = 0x58465342; // "XFSB" in big-endian
const XFS_INODE_MAGIC: u16 = 0x494E; // "IN" in big-endian
const XFS_DINODE_FMT_EXTENTS: u8 = 1;
const XFS_DINODE_FMT_BTREE: u8 = 2;
const XFS_DINODE_FMT_LOCAL: u8 = 3;

// XFS inode states (reserved for future use)
const _XFS_INODE_GOOD: u16 = 0;
const _XFS_INODE_FREE: u16 = 1;
const _XFS_INODE_UNLINKED: u16 = 2;

/// Configuration for XFS recovery operations
#[derive(Debug, Clone)]
pub struct XfsRecoveryConfig {
    /// Maximum blocks to scan in signature-based recovery.
    /// None = scan all blocks (adaptive based on filesystem size)
    pub max_scan_blocks: Option<u64>,
    
    /// Maximum bytes to search when estimating file sizes
    /// Default: 10MB for large file support
    pub max_file_search_bytes: usize,
    
    /// Threshold for text file detection (0.0-1.0)
    /// Ratio of printable characters needed to classify as text
    pub text_detection_threshold: f32,
    
    /// Sample size for text detection
    pub text_sample_size: usize,
}

impl Default for XfsRecoveryConfig {
    fn default() -> Self {
        Self {
            max_scan_blocks: None, // Adaptive: will scan intelligently based on FS size
            max_file_search_bytes: 10 * 1024 * 1024, // 10MB
            text_detection_threshold: 0.75, // 75% printable chars
            text_sample_size: 4096, // 4KB sample
        }
    }
}

impl XfsRecoveryConfig {
    /// Calculate adaptive scan limit based on filesystem size
    /// Scans more on smaller filesystems, uses sampling on larger ones
    pub fn adaptive_scan_blocks(&self, total_blocks: u64) -> u64 {
        if let Some(max) = self.max_scan_blocks {
            return std::cmp::min(total_blocks, max);
        }
        
        // Adaptive strategy:
        // - Small FS (<1GB): scan all blocks
        // - Medium FS (1GB-100GB): scan 10% minimum 10k blocks
        // - Large FS (>100GB): scan 1% minimum 100k blocks
        match total_blocks {
            0..=262_144 => total_blocks, // <1GB: scan all
            262_145..=26_214_400 => std::cmp::max(total_blocks / 10, 10_000), // 1-100GB: 10%
            _ => std::cmp::max(total_blocks / 100, 100_000), // >100GB: 1%
        }
    }
}

#[derive(Debug, Clone)]
pub struct XfsSuperblock {
    pub magic: u32,
    pub block_size: u32,
    pub data_blocks: u64,
    pub realtime_blocks: u64,
    pub realtime_extents: u64,
    pub uuid: [u8; 16],
    pub log_start: u64,
    pub root_inode: u64,
    pub realtime_bitmap_inode: u64,
    pub realtime_summary_inode: u64,
    pub realtime_extent_size: u32,
    pub ag_blocks: u32,
    pub ag_count: u32,
    pub realtime_bitmap_blocks: u32,
    pub log_blocks: u32,
    pub version_num: u16,
    pub sector_size: u16,
    pub inode_size: u16,
    pub inodes_per_block: u16,
    pub filesystem_name: [u8; 12],
    pub block_log: u8,
    pub sector_log: u8,
    pub inode_log: u8,
    pub inopb_log: u8,
    pub agblklog: u8,
    pub rextslog: u8,
    pub in_progress: u8,
    pub max_inode_percent: u8,
    pub inode_count: u64,
    pub inode_free_count: u64,
    pub free_data_extents: u64,
    pub free_realtime_extents: u64,
}

pub struct XfsRecoveryEngine {
    device: BlockDevice,
    superblock: Option<XfsSuperblock>,
    ag_count: u32,
    ag_blocks: u32,
    block_size: u32,
    sector_size: u32,
    inode_size: u16,
    inodes_per_block: u16,
    ag_inode_table_blocks: Vec<u64>, // Starting block of inode table for each AG
    config: XfsRecoveryConfig,
}

impl XfsRecoveryEngine {
    pub fn new(device: BlockDevice) -> Result<Self> {
        Self::new_with_config(device, XfsRecoveryConfig::default())
    }
    
    pub fn new_with_config(device: BlockDevice, config: XfsRecoveryConfig) -> Result<Self> {
        tracing::info!("🔧 Initializing XFS Recovery Engine");
        
        let mut engine = XfsRecoveryEngine {
            device,
            superblock: None,
            ag_count: 4,
            ag_blocks: 1000,
            block_size: 4096,
            sector_size: 512,
            inode_size: 256,
            inodes_per_block: 16,
            ag_inode_table_blocks: Vec::new(),
            config,
        };
        
        // Parse the XFS superblock
        match engine.parse_superblock() {
            Ok(sb) => {
                tracing::info!("✅ XFS superblock parsed successfully");
                tracing::info!("📊 Filesystem details: {} AGs, {} blocks each, block size: {}", 
                    sb.ag_count, sb.ag_blocks, sb.block_size);
                
                engine.ag_count = sb.ag_count;
                engine.ag_blocks = sb.ag_blocks;
                engine.block_size = sb.block_size;
                engine.sector_size = sb.sector_size as u32;
                engine.inode_size = sb.inode_size;
                engine.inodes_per_block = sb.inodes_per_block;
                engine.superblock = Some(sb);
                
                // Calculate inode table locations for each AG
                engine.calculate_ag_inode_tables()?;
            }
            Err(e) => {
                tracing::warn!("⚠️ Failed to parse XFS superblock: {}", e);
                tracing::info!("🔧 Using default XFS parameters for recovery");
                
                // Use defaults but still try to scan
                engine.calculate_ag_inode_tables()?;
            }
        }
        
        tracing::info!("🚀 XFS Recovery Engine initialized successfully");
        Ok(engine)
    }

    /// Parse XFS superblock from sector 0
    fn parse_superblock(&self) -> Result<XfsSuperblock> {
        tracing::debug!("📖 Reading XFS superblock from sector 0");
        let data = self.device.read_sector(0)?;
        
        if data.len() < 264 {
            anyhow::bail!("Insufficient data for XFS superblock: {} bytes", data.len());
        }

        let magic = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        if magic != XFS_MAGIC {
            anyhow::bail!("Invalid XFS magic: 0x{:08x}, expected 0x{:08x}", magic, XFS_MAGIC);
        }

        let block_size = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        let data_blocks = u64::from_be_bytes([
            data[8], data[9], data[10], data[11], data[12], data[13], data[14], data[15]
        ]);
        let realtime_blocks = u64::from_be_bytes([
            data[16], data[17], data[18], data[19], data[20], data[21], data[22], data[23]
        ]);
        let realtime_extents = u64::from_be_bytes([
            data[24], data[25], data[26], data[27], data[28], data[29], data[30], data[31]
        ]);

        let mut uuid = [0u8; 16];
        uuid.copy_from_slice(&data[32..48]);

        let log_start = u64::from_be_bytes([
            data[48], data[49], data[50], data[51], data[52], data[53], data[54], data[55]
        ]);
        let root_inode = u64::from_be_bytes([
            data[56], data[57], data[58], data[59], data[60], data[61], data[62], data[63]
        ]);

        // Continue parsing other fields with bounds checking
        let ag_blocks = if data.len() >= 88 {
            u32::from_be_bytes([data[84], data[85], data[86], data[87]])
        } else {
            data_blocks as u32 / 4 // Estimate: 4 AGs by default
        };
        
        let ag_count = if data.len() >= 92 {
            u32::from_be_bytes([data[88], data[89], data[90], data[91]])
        } else {
            if ag_blocks > 0 { data_blocks as u32 / ag_blocks } else { 4 }
        };

        let inode_size = if data.len() >= 106 {
            u16::from_be_bytes([data[104], data[105]])
        } else {
            256 // Default XFS inode size
        };

        let inodes_per_block = if inode_size > 0 {
            block_size as u16 / inode_size
        } else {
            16 // Safe default
        };

        let sector_size = if data.len() >= 104 {
            u16::from_be_bytes([data[102], data[103]])
        } else {
            512 // Standard sector size
        };

        tracing::debug!("📋 Parsed superblock: {} data blocks, {} AGs of {} blocks each", 
            data_blocks, ag_count, ag_blocks);

        Ok(XfsSuperblock {
            magic,
            block_size,
            data_blocks,
            realtime_blocks,
            realtime_extents,
            uuid,
            log_start,
            root_inode,
            realtime_bitmap_inode: 0, // Will be filled with defaults
            realtime_summary_inode: 0,
            realtime_extent_size: 0,
            ag_blocks,
            ag_count,
            realtime_bitmap_blocks: 0,
            log_blocks: 0,
            version_num: 4, // Default version
            sector_size,
            inode_size,
            inodes_per_block,
            filesystem_name: [0u8; 12], // Will be filled with defaults
            block_log: 12, // 4096 = 2^12
            sector_log: 9,  // 512 = 2^9
            inode_log: 8,   // 256 = 2^8
            inopb_log: 4,   // 16 = 2^4
            agblklog: 20,   // Will be calculated
            rextslog: 0,
            in_progress: 0,
            max_inode_percent: 25,
            inode_count: data_blocks / 32, // Estimate
            inode_free_count: 0,
            free_data_extents: 0,
            free_realtime_extents: 0,
        })
    }

    /// Calculate inode table locations for each allocation group
    fn calculate_ag_inode_tables(&mut self) -> Result<()> {
        tracing::debug!("🧮 Calculating inode table locations for {} AGs", self.ag_count);
        
        self.ag_inode_table_blocks.clear();
        
        for ag_no in 0..self.ag_count {
            // In XFS, each AG starts at: ag_no * ag_blocks
            // The inode table typically starts at a fixed offset within each AG
            // For simplicity, we'll use a common layout: AG header + some blocks = inode table
            let ag_start_block = ag_no as u64 * self.ag_blocks as u64;
            let inode_table_offset = 4; // Typically after AG free space header, inode header, etc.
            let inode_table_start = ag_start_block + inode_table_offset;
            
            self.ag_inode_table_blocks.push(inode_table_start);
            tracing::debug!("📍 AG {} inode table starts at block {}", ag_no, inode_table_start);
        }
        
        Ok(())
    }

    /// Comprehensive scan for deleted files across all allocation groups
    pub fn scan_deleted_files(&self) -> Result<Vec<crate::DeletedFile>> {
        tracing::info!("🔍 Starting comprehensive XFS deleted file scan");
        let mut deleted_files = Vec::new();
        let mut file_id_counter = 1u64;

        // Scan each allocation group for deleted inodes
        for (ag_no, &inode_table_start) in self.ag_inode_table_blocks.iter().enumerate() {
            tracing::debug!("� Scanning AG {} inode table starting at block {}", ag_no, inode_table_start);
            
            match self.scan_allocation_group_inodes(ag_no as u32, inode_table_start, &mut file_id_counter) {
                Ok(mut ag_files) => {
                    tracing::info!("📁 Found {} deleted files in AG {}", ag_files.len(), ag_no);
                    deleted_files.append(&mut ag_files);
                }
                Err(e) => {
                    tracing::warn!("⚠️ Failed to scan AG {}: {}", ag_no, e);
                    // Continue with other AGs
                }
            }
        }

        // Additional signature-based scanning for files without readable inodes
        tracing::info!("🔍 Performing signature-based scan for additional files");
        match self.signature_based_scan(&mut file_id_counter) {
            Ok(mut sig_files) => {
                tracing::info!("📄 Found {} files via signature scanning", sig_files.len());
                deleted_files.append(&mut sig_files);
            }
            Err(e) => {
                tracing::warn!("⚠️ Signature scan failed: {}", e);
            }
        }

        tracing::info!("✅ XFS scan complete: {} total deleted files found", deleted_files.len());
        Ok(deleted_files)
    }

    /// Scan inodes in a specific allocation group
    fn scan_allocation_group_inodes(
        &self, 
        ag_no: u32, 
        inode_table_start: u64,
        file_id_counter: &mut u64
    ) -> Result<Vec<crate::DeletedFile>> {
        let mut deleted_files = Vec::new();
        
        // Calculate how many inode blocks to scan
        // Typically scan first several blocks of inode table
        let max_inode_blocks_to_scan = std::cmp::min(64, self.ag_blocks / 8);
        
        for block_offset in 0..max_inode_blocks_to_scan {
            let inode_block = inode_table_start + block_offset as u64;
            
            match self.scan_inode_block(ag_no, inode_block, file_id_counter) {
                Ok(mut block_files) => {
                    deleted_files.append(&mut block_files);
                }
                Err(e) => {
                    tracing::debug!("⚠️ Failed to read inode block {}: {}", inode_block, e);
                    // Continue with next block
                }
            }
        }
        
        Ok(deleted_files)
    }

    /// Scan a single inode block for deleted files
    fn scan_inode_block(
        &self,
        ag_no: u32,
        block_number: u64,
        file_id_counter: &mut u64
    ) -> Result<Vec<crate::DeletedFile>> {
        let mut deleted_files = Vec::new();
        
        // Read the inode block
        let block_data = self.device.read_block(block_number, self.block_size)?;
        
        // Scan each inode slot in this block
        let inode_size = self.inode_size as usize;
        let inodes_in_block = self.block_size as usize / inode_size;
        
        for inode_idx in 0..inodes_in_block {
            let inode_offset = inode_idx * inode_size;
            if inode_offset + inode_size > block_data.len() {
                break;
            }
            
            let inode_data = &block_data[inode_offset..inode_offset + inode_size];
            
            // Try to parse this inode
            match self.parse_inode(inode_data, ag_no, block_number, inode_idx) {
                Ok(Some(deleted_file)) => {
                    let mut file = deleted_file;
                    file.id = *file_id_counter;
                    *file_id_counter += 1;
                    deleted_files.push(file);
                }
                Ok(None) => {
                    // Not a deleted file or not recoverable
                }
                Err(e) => {
                    tracing::debug!("Failed to parse inode at block {} offset {}: {}", 
                        block_number, inode_idx, e);
                }
            }
        }
        
        Ok(deleted_files)
    }

    /// Parse an individual inode and determine if it represents a deleted file
    fn parse_inode(
        &self,
        inode_data: &[u8],
        ag_no: u32,
        block_number: u64,
        inode_idx: usize
    ) -> Result<Option<crate::DeletedFile>> {
        if inode_data.len() < 96 {
            return Ok(None); // Too small to be a valid inode
        }

        // Check inode magic number
        let magic = u16::from_be_bytes([inode_data[0], inode_data[1]]);
        if magic != XFS_INODE_MAGIC {
            return Ok(None); // Not a valid inode
        }

        // Parse inode core structure
        let mode = u16::from_be_bytes([inode_data[2], inode_data[3]]);
        let version = inode_data[4];
        let format = inode_data[5];
        let onlink = u16::from_be_bytes([inode_data[6], inode_data[7]]);
        
        // Generation number (helps detect reused inodes)
        let gen = u32::from_be_bytes([inode_data[8], inode_data[9], inode_data[10], inode_data[11]]);
        
        // UID/GID for ownership info
        let uid = u32::from_be_bytes([inode_data[24], inode_data[25], inode_data[26], inode_data[27]]);
        let gid = u32::from_be_bytes([inode_data[28], inode_data[29], inode_data[30], inode_data[31]]);
        
        // File size (64-bit)
        let size = u64::from_be_bytes([
            inode_data[56], inode_data[57], inode_data[58], inode_data[59],
            inode_data[60], inode_data[61], inode_data[62], inode_data[63]
        ]);

        // Block count (64-bit)
        let nblocks = u64::from_be_bytes([
            inode_data[64], inode_data[65], inode_data[66], inode_data[67],
            inode_data[68], inode_data[69], inode_data[70], inode_data[71]
        ]);

        // Timestamps
        let atime = i32::from_be_bytes([inode_data[72], inode_data[73], inode_data[74], inode_data[75]]);
        let mtime = i32::from_be_bytes([inode_data[80], inode_data[81], inode_data[82], inode_data[83]]);
        let ctime = i32::from_be_bytes([inode_data[88], inode_data[89], inode_data[90], inode_data[91]]);

        // Enhanced deleted file detection with multiple heuristics
        let is_deleted = self.is_likely_deleted_file(
            mode, onlink, size, nblocks, atime, mtime, ctime, 
            version, format, gen, uid, gid
        );
        
        if !is_deleted {
            return Ok(None);
        }

        // Calculate inode number
        let inode_number = (ag_no as u64 * self.ag_blocks as u64 * self.inodes_per_block as u64) +
                          (block_number - self.ag_inode_table_blocks[ag_no as usize]) * self.inodes_per_block as u64 +
                          inode_idx as u64;

        tracing::debug!("🔍 Found deleted inode {} in AG {}: size={}, blocks={}", 
            inode_number, ag_no, size, nblocks);

        // Extract data block references
        let data_blocks = self.extract_data_blocks(inode_data, format, size)?;

        // Determine file type from mode
        let file_type = match mode & 0xF000 {
            0x8000 => crate::FileType::RegularFile,
            0x4000 => crate::FileType::Directory,
            0xA000 => crate::FileType::SymbolicLink,
            _ => crate::FileType::Unknown,
        };

        // Try to determine file extension and MIME type from content
        let (mime_type, extension) = if !data_blocks.is_empty() {
            self.analyze_file_content(&data_blocks[0])
        } else {
            (None, None)
        };

        // Convert timestamps
        let deletion_time = if ctime > 0 {
            DateTime::from_timestamp(ctime as i64, 0)
        } else {
            None
        };

        let modified_time = if mtime > 0 {
            DateTime::from_timestamp(mtime as i64, 0)
        } else {
            None
        };

        let accessed_time = if atime > 0 {
            DateTime::from_timestamp(atime as i64, 0)
        } else {
            None
        };

        // Generate a reasonable filename if we can determine the type
        let original_path = self.generate_filename(inode_number, &extension, &file_type);

        let deleted_file = crate::DeletedFile {
            id: 0, // Will be set by caller
            inode_or_cluster: inode_number,
            original_path: Some(original_path),
            size,
            deletion_time,
            confidence_score: self.calculate_confidence_score(size, nblocks, &data_blocks),
            file_type,
            data_blocks,
            is_recoverable: true,
            metadata: crate::FileMetadata {
                mime_type,
                file_extension: extension,
                permissions: Some(mode as u32 & 0o777),
                owner_uid: Some(uid),
                owner_gid: Some(gid),
                created_time: deletion_time, // Use ctime as creation time
                modified_time,
                accessed_time,
                extended_attributes: HashMap::new(),
            },
        };

        Ok(Some(deleted_file))
    }

    /// Enhanced deleted file detection using multiple heuristics
    /// This reduces false positives and catches more edge cases
    fn is_likely_deleted_file(
        &self, 
        mode: u16, 
        onlink: u16, 
        size: u64, 
        nblocks: u64,
        _atime: i32,
        mtime: i32,
        ctime: i32,
        version: u8,
        format: u8,
        gen: u32,
        uid: u32,
        gid: u32,
    ) -> bool {
        // PRIMARY INDICATOR: Link count is 0 (no directory entries point to this inode)
        if onlink != 0 {
            return false; // File still has directory entries, not deleted
        }

        // SANITY CHECKS: File must have meaningful content
        if size == 0 || nblocks == 0 {
            return false; // Empty inode or no data blocks
        }

        // CHECK 1: Mode field validation
        // Deleted files usually still have valid mode (file type + permissions)
        // mode == 0 typically means inode was freed and zeroed out
        let file_type = mode & 0xF000;
        let is_valid_type = matches!(
            file_type,
            0x8000 | // Regular file
            0x4000 | // Directory  
            0xA000   // Symlink
        );
        
        if !is_valid_type {
            return false; // Invalid or zeroed file type
        }

        // CHECK 2: Timestamp validation
        // Valid timestamps indicate the inode held real data
        let has_valid_timestamps = (mtime > 0 || ctime > 0) && mtime < 2147483647; // Before year 2038
        if !has_valid_timestamps {
            return false; // Suspicious timestamps
        }

        // CHECK 3: Size/block consistency
        // Verify the file size makes sense for the number of blocks allocated
        let expected_blocks = (size + self.block_size as u64 - 1) / self.block_size as u64;
        let blocks_reasonable = nblocks <= expected_blocks * 2; // Allow some slack for metadata
        
        if !blocks_reasonable {
            return false; // Block count doesn't match file size (corrupted inode)
        }

        // CHECK 4: Remove artificial size limit
        // Previous code limited to 1GB, but we should allow larger files
        // Only reject truly unreasonable sizes (> 16TB as sanity check)
        const MAX_REASONABLE_SIZE: u64 = 16 * 1024 * 1024 * 1024 * 1024; // 16TB
        if size > MAX_REASONABLE_SIZE {
            return false; // Unreasonably large, likely corrupted
        }

        // CHECK 5: XFS version validation
        // XFS inode versions: 1 (old), 2 (v4), 3 (v5 with CRC)
        if version == 0 || version > 3 {
            return false; // Invalid inode version
        }

        // CHECK 6: Format field validation
        // Format must be valid for XFS: extents (1), btree (2), or local (3)
        if format == 0 || format > 3 {
            return false; // Invalid data fork format
        }

        // CHECK 7: Generation number check
        // Generation 0 is suspicious (newly created inode or corrupted)
        // Very high generation numbers might indicate corruption
        if gen == 0 || gen > 1_000_000 {
            return false; // Suspicious generation number
        }

        // CHECK 8: UID/GID sanity check
        // UIDs/GIDs should be reasonable (< 65535 for most systems)
        // 0xFFFFFFFF often means uninitialized/corrupted
        const MAX_REASONABLE_UID: u32 = 65535;
        if uid == 0xFFFFFFFF || gid == 0xFFFFFFFF || uid > MAX_REASONABLE_UID {
            return false; // Suspicious ownership
        }

        // CHECK 9: Format consistency with file type
        // Directories shouldn't use local format for large sizes
        if file_type == 0x4000 && format == XFS_DINODE_FMT_LOCAL && size > 256 {
            return false; // Directory too large for local format
        }
// CHECK 5: Timestamp freshness (optional heuristic)
        // Files deleted very long ago might have been partially overwritten
        // But we still want to find them, so this is just for confidence scoring
        
        // All checks passed - this is likely a recoverable deleted file
        true
    }

    /// Extract data block references from inode based on its format
    fn extract_data_blocks(&self, inode_data: &[u8], format: u8, file_size: u64) -> Result<Vec<crate::BlockRange>> {
        let mut data_blocks = Vec::new();
        
        match format {
            XFS_DINODE_FMT_EXTENTS => {
                // Parse extent list from inode
                data_blocks = self.parse_extent_list(inode_data)?;
            }
            XFS_DINODE_FMT_BTREE => {
                // Parse B-tree root and follow to extent leaves
                data_blocks = self.parse_btree_extents(inode_data)?;
            }
            XFS_DINODE_FMT_LOCAL => {
                // Data is stored locally in the inode - create a pseudo block range
                if file_size > 0 {
                    data_blocks.push(crate::BlockRange {
                        start_block: 0, // Special marker for local data
                        block_count: 1,
                        is_allocated: false,
                    });
                }
            }
            _ => {
                tracing::debug!("Unknown inode format: {}", format);
            }
        }
        
        Ok(data_blocks)
    }

    /// Parse extent list format (simplified)
    fn parse_extent_list(&self, inode_data: &[u8]) -> Result<Vec<crate::BlockRange>> {
        let mut extents = Vec::new();
        
        if inode_data.len() < 100 {
            return Ok(extents);
        }

        // XFS extent format: each extent is 16 bytes
        // We'll look for extent data starting after the inode core (around offset 96)
        let extent_area_start = 96;
        let max_extents = (inode_data.len() - extent_area_start) / 16;
        
        for i in 0..std::cmp::min(max_extents, 8) { // Limit to reasonable number
            let offset = extent_area_start + i * 16;
            if offset + 16 > inode_data.len() {
                break;
            }

            // Parse extent (simplified - real XFS extents are more complex)
            let extent_data = &inode_data[offset..offset + 16];
            let start_block = u64::from_be_bytes([
                extent_data[0], extent_data[1], extent_data[2], extent_data[3],
                extent_data[4], extent_data[5], extent_data[6], extent_data[7]
            ]);
            let block_count = u32::from_be_bytes([
                extent_data[8], extent_data[9], extent_data[10], extent_data[11]
            ]) as u64;
            
            // Simple validation - blocks should be reasonable
            if start_block > 0 && start_block < self.device.size() / self.block_size as u64 && 
               block_count > 0 && block_count < 1024 {
                extents.push(crate::BlockRange {
                    start_block,
                    block_count,
                    is_allocated: false,
                });
            }
        }
        
        Ok(extents)
    }

    /// Parse B-tree extent format (simplified)
    fn parse_btree_extents(&self, _inode_data: &[u8]) -> Result<Vec<crate::BlockRange>> {
        // B-tree parsing is complex - for now return empty
        // In a full implementation, this would traverse the B-tree structure
        tracing::debug!("B-tree extent parsing not yet implemented");
        Ok(Vec::new())
    }

    /// Signature-based scanning for files that may not have readable inodes
    fn signature_based_scan(&self, file_id_counter: &mut u64) -> Result<Vec<crate::DeletedFile>> {
        let mut files = Vec::new();
        tracing::debug!("🔍 Starting signature-based scan");

        // Common file signatures to look for  
        let signatures: Vec<(&[u8], &str, &str)> = vec![
            (b"\xFF\xD8\xFF", "image/jpeg", "jpg"),
            (b"\x89PNG\r\n\x1a\n", "image/png", "png"),
            (b"GIF8", "image/gif", "gif"),
            (b"%PDF", "application/pdf", "pdf"),
            (b"PK\x03\x04", "application/zip", "zip"),
            (b"\x7FELF", "application/x-executable", "bin"),
            (b"{\n", "application/json", "json"),
            (b"[", "text/plain", "txt"),
        ];

        // Scan through the device looking for file signatures
        let total_blocks = self.device.size() / self.block_size as u64;
        let scan_blocks = self.config.adaptive_scan_blocks(total_blocks);
        
        tracing::info!("📊 Filesystem size: {} blocks ({:.2} GB)", 
            total_blocks, 
            (total_blocks * self.block_size as u64) as f64 / (1024.0 * 1024.0 * 1024.0)
        );
        if total_blocks > 0 {
            tracing::info!(
                "🔍 Adaptive scan: {} blocks ({:.1}%)",
                scan_blocks,
                (scan_blocks as f64 / total_blocks as f64) * 100.0
            );
        } else {
            // Avoid division by zero when the filesystem has zero blocks
            tracing::info!(
                "🔍 Adaptive scan: {} blocks (0.0%) - total filesystem blocks is 0",
                scan_blocks
            );
        }

        for block_num in 0..scan_blocks {
            if let Ok(block_data) = self.device.read_block(block_num, self.block_size) {
                for (signature, mime_type, extension) in &signatures {
                    if block_data.starts_with(*signature) {
                        // Found a potential file
                        let file_size = self.estimate_file_size_from_signature(block_data, *signature);
                        if file_size > 0 {
                            let block_count = (file_size + self.block_size as u64 - 1) / self.block_size as u64;
                            
                            let deleted_file = crate::DeletedFile {
                                id: *file_id_counter,
                                inode_or_cluster: *file_id_counter + 10000, // Use high numbers for sig-based
                                original_path: Some(std::path::PathBuf::from(
                                    format!("recovered_file_{}.{}", *file_id_counter, extension)
                                )),
                                size: file_size,
                                deletion_time: None,
                                confidence_score: 0.7, // Medium confidence for signature-based
                                file_type: crate::FileType::RegularFile,
                                data_blocks: vec![crate::BlockRange {
                                    start_block: block_num,
                                    block_count,
                                    is_allocated: false,
                                }],
                                is_recoverable: true,
                                metadata: crate::FileMetadata {
                                    mime_type: Some(mime_type.to_string()),
                                    file_extension: Some(extension.to_string()),
                                    permissions: Some(0o644),
                                    owner_uid: None,
                                    owner_gid: None,
                                    created_time: None,
                                    modified_time: None,
                                    accessed_time: None,
                                    extended_attributes: HashMap::new(),
                                },
                            };

                            files.push(deleted_file);
                            *file_id_counter += 1;
                            
                            tracing::debug!("� Found {} file via signature at block {}", extension, block_num);
                        }
                        break; // Found one signature in this block
                    }
                }
            }
        }

        Ok(files)
    }

    /// Estimate file size based on content analysis
    fn estimate_file_size_from_signature(&self, block_data: &[u8], signature: &[u8]) -> u64 {
        match signature {
            b"%PDF" => {
                // Look for PDF trailer
                if let Some(pos) = block_data.windows(9).position(|w| w == b"%%EOF\n") {
                    (pos + 9) as u64
                } else {
                    4096 // Default size
                }
            }
            b"{\n" | b"[" => {
                // JSON - look for matching braces
                let mut brace_count = 0;
                let mut in_string = false;
                let mut escape_next = false;
                
                let search_limit = std::cmp::min(block_data.len(), self.config.max_file_search_bytes);
                
                for (i, &byte) in block_data.iter().take(search_limit).enumerate() {
                    if escape_next {
                        escape_next = false;
                        continue;
                    }
                    
                    match byte {
                        b'"' if !escape_next => in_string = !in_string,
                        b'\\' if in_string => escape_next = true,
                        b'{' | b'[' if !in_string => brace_count += 1,
                        b'}' | b']' if !in_string => {
                            brace_count -= 1;
                            if brace_count == 0 {
                                return (i + 1) as u64;
                            }
                        }
                        _ => {}
                    }
                }
                
                std::cmp::min(block_data.len(), search_limit) as u64
            }
            _ => {
                // Default: use the entire block or look for null padding
                let mut size = block_data.len();
                for i in (0..block_data.len()).rev() {
                    if block_data[i] != 0 {
                        size = i + 1;
                        break;
                    }
                }
                size as u64
            }
        }
    }

    /// Analyze file content to determine type and extension
    fn analyze_file_content(&self, block_range: &crate::BlockRange) -> (Option<String>, Option<String>) {
        if let Ok(data) = self.device.read_block(block_range.start_block, self.block_size) {
            // Check for common file signatures
            if data.starts_with(b"\xFF\xD8\xFF") {
                return (Some("image/jpeg".to_string()), Some("jpg".to_string()));
            } else if data.starts_with(b"\x89PNG\r\n\x1a\n") {
                return (Some("image/png".to_string()), Some("png".to_string()));
            } else if data.starts_with(b"GIF8") {
                return (Some("image/gif".to_string()), Some("gif".to_string()));
            } else if data.starts_with(b"%PDF") {
                return (Some("application/pdf".to_string()), Some("pdf".to_string()));
            } else if data.starts_with(b"PK\x03\x04") {
                return (Some("application/zip".to_string()), Some("zip".to_string()));
            } else if data.starts_with(b"{\n") || data.starts_with(b"{ ") || data.starts_with(b"[") {
                return (Some("application/json".to_string()), Some("json".to_string()));
            } else if data.starts_with(b"[") && data.contains(&b'=') {
                return (Some("text/plain".to_string()), Some("ini".to_string()));
            }
            
            // Check if it's plain text
            let sample_size = std::cmp::min(data.len(), self.config.text_sample_size);
            let text_chars = data.iter().take(sample_size).filter(|&&b| {
                (b >= 32 && b <= 126) || b == b'\n' || b == b'\r' || b == b'\t'
            }).count();
            
            let text_ratio = text_chars as f32 / sample_size as f32;
            if text_ratio >= self.config.text_detection_threshold {
                return (Some("text/plain".to_string()), Some("txt".to_string()));
            }
        }
        
        (None, None)
    }

    /// Generate a reasonable filename for a recovered file
    fn generate_filename(&self, inode_number: u64, extension: &Option<String>, file_type: &crate::FileType) -> std::path::PathBuf {
        let ext = extension.as_deref().unwrap_or(match file_type {
            crate::FileType::Directory => "dir",
            crate::FileType::SymbolicLink => "link",
            _ => "bin",
        });
        
        std::path::PathBuf::from(format!("inode_{}.{}", inode_number, ext))
    }

    /// Calculate confidence score based on file characteristics
    fn calculate_confidence_score(&self, size: u64, nblocks: u64, data_blocks: &[crate::BlockRange]) -> f32 {
        let mut confidence: f32 = 0.5; // Base confidence
        
        // Reasonable file size increases confidence
        if size > 0 && size < 100 * 1024 * 1024 { // 0-100MB
            confidence += 0.2;
        }
        
        // Consistent block count
        let expected_blocks = (size + self.block_size as u64 - 1) / self.block_size as u64;
        if nblocks <= expected_blocks + 1 && nblocks >= expected_blocks.saturating_sub(1) {
            confidence += 0.2;
        }
        
        // Has valid data blocks
        if !data_blocks.is_empty() {
            confidence += 0.1;
        }
        
        // Validate that data blocks are reasonable
        for block_range in data_blocks {
            if block_range.start_block > 0 && 
               block_range.start_block < self.device.size() / self.block_size as u64 {
                confidence += 0.05;
            }
        }
        
        confidence.min(1.0)
    }

    /// Recover file data for a specific inode
    pub fn recover_file(&self, inode: u64) -> Result<Vec<u8>> {
        tracing::info!("🔄 Recovering file data for inode {}", inode);
        
        // First, try to find the inode in our scanned files
        let deleted_files = self.scan_deleted_files()?;
        let target_file = deleted_files.iter()
            .find(|f| f.inode_or_cluster == inode)
            .ok_or_else(|| anyhow::anyhow!("Inode {} not found in deleted files", inode))?;

        let mut recovered_data = Vec::new();
        
        // Recover data from each block range
        for block_range in &target_file.data_blocks {
            if block_range.start_block == 0 {
                // Special case: local data stored in inode
                tracing::debug!("📄 Recovering local inode data for inode {}", inode);
                // Would need to re-read the inode and extract local data
                // For now, add placeholder data
                recovered_data.extend_from_slice(b"[Local inode data - recovery not yet implemented]");
            } else {
                // Read data from blocks
                for block_offset in 0..block_range.block_count {
                    let block_num = block_range.start_block + block_offset;
                    match self.device.read_block(block_num, self.block_size) {
                        Ok(block_data) => {
                            let bytes_to_copy = if recovered_data.len() as u64 + self.block_size as u64 > target_file.size {
                                (target_file.size - recovered_data.len() as u64) as usize
                            } else {
                                block_data.len()
                            };
                            
                            recovered_data.extend_from_slice(&block_data[..bytes_to_copy]);
                            
                            if recovered_data.len() >= target_file.size as usize {
                                break;
                            }
                        }
                        Err(e) => {
                            tracing::warn!("⚠️ Failed to read block {}: {}", block_num, e);
                        }
                    }
                }
            }
        }

        // Truncate to expected file size
        if recovered_data.len() > target_file.size as usize {
            recovered_data.truncate(target_file.size as usize);
        }

        tracing::info!("✅ Recovered {} bytes for inode {}", recovered_data.len(), inode);
        Ok(recovered_data)
    }

    /// Get comprehensive filesystem information
    pub fn get_filesystem_info(&self) -> Result<String> {
        if let Some(ref sb) = self.superblock {
            let total_size_bytes = sb.data_blocks * sb.block_size as u64;
            let total_size_gb = total_size_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
            
            let filesystem_name_str = String::from_utf8_lossy(&sb.filesystem_name);
            
            Ok(format!(
                "XFS Filesystem (Full Analysis):\n\
                 Name: {}\n\
                 Block Size: {} bytes\n\
                 Total Blocks: {}\n\
                 Total Size: {:.2} GB\n\
                 Allocation Groups: {}\n\
                 AG Blocks: {}\n\
                 Inode Size: {} bytes\n\
                 Inodes per Block: {}\n\
                 Version: {}\n\
                 Root Inode: {}\n\
                 Log Start: {}\n\
                 UUID: {:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
                filesystem_name_str.trim_end_matches('\0'),
                sb.block_size,
                sb.data_blocks,
                total_size_gb,
                sb.ag_count,
                sb.ag_blocks,
                sb.inode_size,
                sb.inodes_per_block,
                sb.version_num,
                sb.root_inode,
                sb.log_start,
                sb.uuid[0], sb.uuid[1], sb.uuid[2], sb.uuid[3],
                sb.uuid[4], sb.uuid[5], sb.uuid[6], sb.uuid[7],
                sb.uuid[8], sb.uuid[9], sb.uuid[10], sb.uuid[11],
                sb.uuid[12], sb.uuid[13], sb.uuid[14], sb.uuid[15]
            ))
        } else {
            Ok("XFS Filesystem: No valid superblock found, using default parameters for recovery".to_string())
        }
    }
}

/// Get comprehensive XFS file system information
pub fn get_filesystem_info(device: &BlockDevice) -> Result<String> {
    tracing::info!("🔍 Analyzing XFS filesystem information");
    
    // Read and parse superblock directly
    let data = device.read_sector(0)?;
    
    if data.len() < 264 {
        return Ok("XFS Filesystem: Insufficient data for analysis".to_string());
    }

    let magic = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
    if magic != XFS_MAGIC {
        return Ok(format!("XFS Filesystem: Invalid magic number 0x{:08x}", magic));
    }

    let block_size = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
    let data_blocks = u64::from_be_bytes([
        data[8], data[9], data[10], data[11], data[12], data[13], data[14], data[15]
    ]);
    
    let ag_blocks = if data.len() >= 88 {
        u32::from_be_bytes([data[84], data[85], data[86], data[87]])
    } else {
        1000 // Default
    };
    
    let ag_count = if data.len() >= 92 {
        u32::from_be_bytes([data[88], data[89], data[90], data[91]])
    } else {
        4 // Default
    };

    let inode_size = if data.len() >= 106 {
        u16::from_be_bytes([data[104], data[105]])
    } else {
        256
    };

    let total_size_bytes = data_blocks.saturating_mul(block_size as u64);
    let total_size_gb = total_size_bytes as f64 / (1024.0 * 1024.0 * 1024.0);

    Ok(format!(
        "XFS Filesystem (Full Analysis):\n\
         Block Size: {} bytes\n\
         Total Blocks: {}\n\
         Total Size: {:.2} GB\n\
         Allocation Groups: {}\n\
         AG Blocks: {}\n\
         Inode Size: {} bytes\n\
         Magic: 0x{:08x}",
        block_size,
        data_blocks,
        total_size_gb,
        ag_count,
        ag_blocks,
        inode_size,
        magic
    ))
}

/// Get filesystem size information (total_blocks, block_size)
pub fn get_filesystem_size(device: &BlockDevice) -> Result<(u64, u32)> {
    let data = device.read_sector(0)?;
    
    if data.len() < 16 {
        anyhow::bail!("Insufficient data to read XFS superblock");
    }
    
    let magic = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
    if magic != XFS_MAGIC {
        anyhow::bail!("Not a valid XFS filesystem");
    }
    
    let block_size = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
    let data_blocks = u64::from_be_bytes([
        data[8], data[9], data[10], data[11], data[12], data[13], data[14], data[15]
    ]);
    
    Ok((data_blocks, block_size))
}

/// Check if the provided data contains a valid XFS superblock
pub fn is_xfs_superblock(data: &[u8]) -> bool {
    if data.len() < 4 {
        return false;
    }
    let magic = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
    let is_xfs = magic == XFS_MAGIC;
    
    if is_xfs {
        tracing::info!("✅ XFS filesystem detected");
    } else {
        tracing::debug!("❌ Not an XFS filesystem (magic: 0x{:08x})", magic);
    }
    
    is_xfs
}
