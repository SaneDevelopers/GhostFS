use std::collections::HashMap;
use anyhow::Result;
use chrono::{Utc, TimeZone};
use byteorder::{BigEndian, ReadBytesExt, ByteOrder};
use std::io::Cursor;

use super::common::BlockDevice;
use crate::{DeletedFile, FileType, FileMetadata, BlockRange};

/// XFS magic number in superblock - "XFSB"
const XFS_MAGIC: u32 = 0x58465342;

/// File mode constants for inode analysis
const S_IFMT: u16 = 0o170000;   // File type mask
const S_IFREG: u16 = 0o100000;  // Regular file
const S_IFDIR: u16 = 0o040000;  // Directory
const S_IFLNK: u16 = 0o120000;  // Symbolic link

/// XFS superblock structure
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
    pub inodes_per_block_log: u8,
    pub ag_blocks_log: u8,
    pub realtime_extents_log: u8,
    pub in_progress: u8,
    pub inode_max_pct: u8,
    pub inode_count: u64,
    pub free_inode_count: u64,
    pub free_data_block_count: u64,
    pub free_realtime_extent_count: u64,
}

/// XFS dinode core structure for inode analysis
#[derive(Debug, Clone)]
pub struct XfsDinodeCore {
    pub magic: u16,
    pub mode: u16,
    pub version: u8,
    pub format: u8,
    pub nlink: u16,
    pub uid: u32,
    pub gid: u32,
    pub size: u64,
    pub next_unlinked: u32,
}

/// XFS recovery engine with enhanced capabilities
#[derive(Debug)]
pub struct XfsRecoveryEngine {
    superblock: XfsSuperblock,
    ag_count: u32,
    ag_blocks: u32,
    block_size: u32,
    inode_size: u16,
}

impl XfsSuperblock {
    /// Parse XFS superblock from raw data
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < 264 {
            return Err(anyhow::anyhow!("Insufficient data for XFS superblock"));
        }

        // Parse magic number first
        let magic = BigEndian::read_u32(&data[0..4]);
        
        if magic != XFS_MAGIC {
            return Err(anyhow::anyhow!("Invalid XFS magic: 0x{:08x}", magic));
        }
        
        // Parse UUID
        let mut uuid = [0u8; 16];
        uuid.copy_from_slice(&data[32..48]);
        
        // Parse filesystem name - 12 bytes starting at offset 108
        let mut fs_name = [0u8; 12];
        if data.len() >= 120 {
            fs_name.copy_from_slice(&data[108..120]);
        }

        Ok(XfsSuperblock {
            magic,
            block_size: BigEndian::read_u32(&data[4..8]),
            data_blocks: BigEndian::read_u64(&data[8..16]),
            realtime_blocks: BigEndian::read_u64(&data[16..24]),
            realtime_extents: BigEndian::read_u64(&data[24..32]),
            uuid,
            log_start: BigEndian::read_u64(&data[48..56]),
            root_inode: BigEndian::read_u64(&data[56..64]),
            realtime_bitmap_inode: BigEndian::read_u64(&data[64..72]),
            realtime_summary_inode: BigEndian::read_u64(&data[72..80]),
            realtime_extent_size: BigEndian::read_u32(&data[80..84]),
            ag_blocks: BigEndian::read_u32(&data[84..88]),
            ag_count: BigEndian::read_u32(&data[88..92]),
            realtime_bitmap_blocks: BigEndian::read_u32(&data[92..96]),
            log_blocks: BigEndian::read_u32(&data[96..100]),
            version_num: BigEndian::read_u16(&data[100..102]),
            sector_size: BigEndian::read_u16(&data[102..104]),
            inode_size: BigEndian::read_u16(&data[104..106]),
            inodes_per_block: BigEndian::read_u16(&data[106..108]),
            filesystem_name: fs_name,
            block_log: data[108],
            sector_log: data[109],
            inode_log: data[110],
            inodes_per_block_log: data[111],
            ag_blocks_log: data[112],
            realtime_extents_log: data[113],
            in_progress: data[114],
            inode_max_pct: data[115],
            inode_count: BigEndian::read_u64(&data[128..136]),
            free_inode_count: BigEndian::read_u64(&data[136..144]),
            free_data_block_count: BigEndian::read_u64(&data[144..152]),
            free_realtime_extent_count: BigEndian::read_u64(&data[152..160]),
        })
    }
}

impl XfsRecoveryEngine {
    /// Create new XFS recovery engine
    pub fn new(device: &BlockDevice) -> Result<Self> {
        // Read the superblock
        let sb_data = device.read_block(0, 512)?;
        let superblock = XfsSuperblock::parse(&sb_data)?;
        
        Ok(XfsRecoveryEngine {
            ag_count: superblock.ag_count,
            ag_blocks: superblock.ag_blocks,
            block_size: superblock.block_size,
            inode_size: superblock.inode_size,
            superblock,
        })
    }
    
    /// Scan for deleted files
    pub fn scan_deleted_files(&self, device: &BlockDevice, confidence_threshold: f32) -> Result<Vec<DeletedFile>> {
        let mut deleted_files = Vec::new();
        let mut file_id = 1u64;
        
        tracing::info!("Starting XFS filesystem analysis...");
        tracing::info!("Block size: {}, AG count: {}, AG blocks: {}", 
                       self.block_size, self.ag_count, self.ag_blocks);
        
        tracing::info!("Scanning {} allocation groups for deleted files...", self.ag_count);
        
        // Scan allocation groups for deleted inodes
        for ag_no in 0..self.ag_count.min(10) { // Limit for safety
            tracing::debug!("Scanning allocation group {}/{}", ag_no + 1, self.ag_count);
            
            let progress = (ag_no as f32 / self.ag_count as f32) * 100.0;
            tracing::info!("Progress: {:.1}%", progress);
            
            if let Ok(mut found_files) = self.scan_allocation_group(device, ag_no, &mut file_id) {
                let ag_file_count = found_files.len();
                if ag_file_count > 0 {
                    tracing::info!("Found {} potential files in AG {}", ag_file_count, ag_no);
                    deleted_files.append(&mut found_files);
                }
            }
        }
        
        // Filter by confidence threshold
        let high_confidence_files: Vec<DeletedFile> = deleted_files.into_iter()
            .filter(|file| file.metadata.confidence > confidence_threshold)
            .collect();
            
        tracing::info!("Total files found: {} (confidence > {:.2})", 
                       high_confidence_files.len(), confidence_threshold);
        
        Ok(high_confidence_files)
    }
    
    /// Scan a single allocation group for deleted inodes
    fn scan_allocation_group(&self, device: &BlockDevice, ag_no: u32, file_id: &mut u64) -> Result<Vec<DeletedFile>> {
        let mut files = Vec::new();
        
        // Calculate allocation group boundaries
        let ag_start_block = ag_no as u64 * self.ag_blocks as u64;
        tracing::debug!("Scanning AG {} starting at block {}", ag_no, ag_start_block);
        
        let scan_blocks = self.ag_blocks.min(100); // Limit scan for demo
        
        // Scan blocks in this allocation group
        for block_offset in 0..scan_blocks {
            let block_num = ag_start_block + block_offset as u64;
            
            // Read and analyze block for inodes
            if let Ok(block_data) = device.read_block(block_num, self.block_size) {
                if let Ok(mut block_files) = self.analyze_block_for_inodes(&block_data, block_num, file_id) {
                    files.append(&mut block_files);
                }
            }
        }
        
        Ok(files)
    }
    
    /// Analyze a block for potential inode structures
    fn analyze_block_for_inodes(&self, block_data: &[u8], block_num: u64, file_id: &mut u64) -> Result<Vec<DeletedFile>> {
        let mut files = Vec::new();
        
        // Scan for inode signatures at inode boundaries
        for offset in (0..block_data.len()).step_by(self.inode_size as usize) {
            if offset + self.inode_size as usize <= block_data.len() {
                let inode_data = &block_data[offset..offset + self.inode_size as usize];
                
                if let Some(deleted_file) = self.analyze_potential_inode(inode_data, block_num, offset, file_id) {
                    files.push(deleted_file);
                    *file_id += 1;
                }
            }
        }
        
        Ok(files)
    }
    
    /// Analyze potential inode data for deleted file indicators
    fn analyze_potential_inode(&self, data: &[u8], block_num: u64, offset: usize, file_id: &u64) -> Option<DeletedFile> {
        if data.len() < 96 {
            return None;
        }
        
        // Check for inode magic ("IN")
        if data.len() >= 4 {
            let potential_magic = BigEndian::read_u16(&data[0..2]);
            if potential_magic == 0x494E { // "IN" magic
                let mode = BigEndian::read_u16(&data[2..4]);
                
                // Check if this looks like a regular file
                if mode & S_IFMT == S_IFREG && mode & 0o777 != 0 {
                    
                    // Look for potential modification time
                    let mut best_timestamp = None;
                    for i in (16..64).step_by(4) {
                        if i + 4 <= data.len() {
                            let timestamp = BigEndian::read_u32(&data[i..i + 4]);
                            if timestamp > 946684800 && timestamp < 2147483647 { // 2000-2038 range
                                best_timestamp = Some(timestamp as i64);
                                break;
                            }
                        }
                    }
                    
                    // Look for file size
                    let size = if data.len() >= 64 {
                        BigEndian::read_u64(&data[56..64])
                    } else {
                        0
                    };
                    
                    if size > 0 && size < 1_000_000_000 { // Reasonable file size
                        let confidence = self.calculate_inode_confidence(data, mode);
                        
                        if confidence > 0.3 {
                            return Some(DeletedFile {
                                id: *file_id,
                                name: format!("xfs_recovered_{}.dat", file_id),
                                file_type: self.determine_file_type(mode),
                                metadata: FileMetadata {
                                    size,
                                    created: best_timestamp.and_then(|ts| Utc.timestamp_opt(ts, 0).single()),
                                    modified: best_timestamp.and_then(|ts| Utc.timestamp_opt(ts, 0).single()),
                                    accessed: None,
                                    permissions: Some(mode as u32 & 0o7777),
                                    uid: if data.len() >= 12 {
                                        Some(BigEndian::read_u32(&data[8..12]))
                                    } else {
                                        None
                                    },
                                    gid: if data.len() >= 16 {
                                        Some(BigEndian::read_u32(&data[12..16]))
                                    } else {
                                        None
                                    },
                                    confidence,
                                },
                                data_blocks: vec![BlockRange {
                                    start: block_num,
                                    count: 1,
                                }],
                            });
                        }
                    }
                }
            }
        }
        
        None
    }
    
    /// Calculate confidence score for potential inode
    fn calculate_inode_confidence(&self, data: &[u8], mode: u16) -> f32 {
        let mut confidence = 0.0;
        
        // Base confidence for valid mode
        if mode & S_IFMT == S_IFREG {
            confidence += 0.3;
            
            if data.len() >= 4 {
                // Check for reasonable permissions
                if let Some(perms) = self.extract_permissions(data) {
                    if perms > 0 && perms <= 0o777 {
                        confidence += 0.2;
                    }
                }
            }
            
            // Check for reasonable file size
            if data.len() >= 64 {
                let size = BigEndian::read_u64(&data[56..64]);
                if size > 0 && size < 1_000_000_000 { // 1GB limit
                    confidence += 0.3;
                }
            }
            
            // Look for timestamp patterns
            if self.has_reasonable_timestamp(data) {
                confidence += 0.2;
            }
        }
        
        confidence.min(1.0)
    }
    
    /// Extract file permissions from inode data
    fn extract_permissions(&self, data: &[u8]) -> Option<u32> {
        if data.len() >= 4 {
            let mode = BigEndian::read_u16(&data[2..4]);
            if mode & S_IFMT == S_IFREG {
                return Some(mode as u32 & 0o7777);
            }
        }
        None
    }
    
    /// Check if inode contains reasonable timestamps
    fn has_reasonable_timestamp(&self, data: &[u8]) -> bool {
        // Look for timestamps in common inode locations
        for offset in &[16, 20, 24, 28, 32, 36, 40, 44] {
            if *offset + 4 <= data.len() {
                let timestamp = BigEndian::read_u32(&data[*offset..*offset + 4]);
                // Check for reasonable Unix timestamp (2000-2038)
                if timestamp > 946684800 && timestamp < 2147483647 {
                    return true;
                }
            }
        }
        false
    }
    
    /// Determine file type from inode mode
    fn determine_file_type(&self, mode: u16) -> FileType {
        match mode & S_IFMT {
            S_IFREG => FileType::RegularFile,
            S_IFDIR => FileType::Directory,
            S_IFLNK => FileType::SymbolicLink,
            _ => FileType::Unknown,
        }
    }
}
