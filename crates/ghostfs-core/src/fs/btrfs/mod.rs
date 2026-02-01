/// Btrfs file system support
use anyhow::Result;
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Cursor;

pub mod tree;
pub mod recovery;

use super::common::BlockDevice;

/// Btrfs magic number
const BTRFS_MAGIC: &[u8; 8] = b"_BHRfS_M";

/// Btrfs superblock structure (simplified)
#[derive(Debug)]
pub struct BtrfsSuperblock {
    pub magic: [u8; 8],
    pub uuid: [u8; 16],
    pub physical_address: u64,
    pub flags: u64,
    pub magic2: [u8; 8],
    pub generation: u64,
    pub root: u64,
    pub chunk_root: u64,
    pub log_root: u64,
    pub total_bytes: u64,
    pub bytes_used: u64,
    pub root_dir_objectid: u64,
    pub num_devices: u64,
    pub sectorsize: u32,
    pub nodesize: u32,
    pub stripesize: u32,
    pub chunk_root_generation: u64,
}

impl BtrfsSuperblock {
    /// Parse Btrfs superblock from raw bytes
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < 1024 {
            anyhow::bail!("Insufficient data for Btrfs superblock");
        }

        let mut cursor = Cursor::new(data);
        
        // Skip checksum (32 bytes)
        cursor.set_position(32);
        
        let mut uuid = [0u8; 16];
        std::io::Read::read_exact(&mut cursor, &mut uuid)?;
        
        let physical_address = cursor.read_u64::<LittleEndian>()?;
        let flags = cursor.read_u64::<LittleEndian>()?;
        
        let mut magic = [0u8; 8];
        std::io::Read::read_exact(&mut cursor, &mut magic)?;
        
        if &magic != BTRFS_MAGIC {
            anyhow::bail!("Invalid Btrfs magic");
        }
        
        let generation = cursor.read_u64::<LittleEndian>()?;
        let root = cursor.read_u64::<LittleEndian>()?;
        let chunk_root = cursor.read_u64::<LittleEndian>()?;
        let log_root = cursor.read_u64::<LittleEndian>()?;
        
        // Skip log_root_transid
        cursor.set_position(cursor.position() + 8);
        
        let total_bytes = cursor.read_u64::<LittleEndian>()?;
        let bytes_used = cursor.read_u64::<LittleEndian>()?;
        let root_dir_objectid = cursor.read_u64::<LittleEndian>()?;
        let num_devices = cursor.read_u64::<LittleEndian>()?;
        let sectorsize = cursor.read_u32::<LittleEndian>()?;
        let nodesize = cursor.read_u32::<LittleEndian>()?;
        
        // Skip leafsize (deprecated)
        cursor.set_position(cursor.position() + 4);
        
        let stripesize = cursor.read_u32::<LittleEndian>()?;
        
        // Skip some fields to get to chunk_root_generation
        cursor.set_position(176);
        let chunk_root_generation = cursor.read_u64::<LittleEndian>()?;

        let magic2 = magic;

        Ok(BtrfsSuperblock {
            magic,
            uuid,
            physical_address,
            flags,
            magic2,
            generation,
            root,
            chunk_root,
            log_root,
            total_bytes,
            bytes_used,
            root_dir_objectid,
            num_devices,
            sectorsize,
            nodesize,
            stripesize,
            chunk_root_generation,
        })
    }
}

/// Check if data contains Btrfs superblock signature
pub fn is_btrfs_superblock(data: &[u8]) -> bool {
    if data.len() < 72 {
        return false;
    }
    
    // Btrfs magic is at offset 64
    &data[64..72] == BTRFS_MAGIC
}

/// Get Btrfs file system information
pub fn get_filesystem_info(device: &BlockDevice) -> Result<String> {
    // Btrfs superblock is at 64KB
    let sb_data = device.read_bytes(65536, 4096)?;
    let superblock = BtrfsSuperblock::parse(sb_data)?;
    
    let fs_size_mb = superblock.total_bytes / (1024 * 1024);
    let used_mb = superblock.bytes_used / (1024 * 1024);
    let uuid_str = format!("{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        superblock.uuid[0], superblock.uuid[1], superblock.uuid[2], superblock.uuid[3],
        superblock.uuid[4], superblock.uuid[5], superblock.uuid[6], superblock.uuid[7],
        superblock.uuid[8], superblock.uuid[9], superblock.uuid[10], superblock.uuid[11],
        superblock.uuid[12], superblock.uuid[13], superblock.uuid[14], superblock.uuid[15]
    );
    
    Ok(format!(
        "Btrfs File System\n\
         - Sector Size: {} bytes\n\
         - Node Size: {} bytes\n\
         - Total Size: {} MB\n\
         - Used Space: {} MB\n\
         - Generation: {}\n\
         - Number of Devices: {}\n\
         - Root Tree: 0x{:x}\n\
         - Chunk Tree: 0x{:x}\n\
         - UUID: {}",
        superblock.sectorsize,
        superblock.nodesize,
        fs_size_mb,
        used_mb,
        superblock.generation,
        superblock.num_devices,
        superblock.root,
        superblock.chunk_root,
        uuid_str
    ))
}

/// Scan for deleted files in Btrfs
pub fn scan_for_deleted_files(device: &BlockDevice) -> Result<Vec<crate::DeletedFile>> {
    // Parse superblock
    let sb_data = device.read_bytes(65536, 4096)?;
    let superblock = BtrfsSuperblock::parse(sb_data)?;
    
    tracing::info!("Btrfs scan: Starting tree analysis");
    tracing::info!("  Generation: {}", superblock.generation);
    tracing::info!("  Root tree: 0x{:x}", superblock.root);
    tracing::info!("  Node size: {} bytes", superblock.nodesize);
    
    // Create and use the recovery engine
    let recovery_engine = recovery::BtrfsRecoveryEngine::new(device, superblock)?;
    let deleted_files = recovery_engine.scan_deleted_files()?;
    
    tracing::info!("Btrfs scan complete: {} files found", deleted_files.len());
    
    Ok(deleted_files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_btrfs_magic_detection() {
        let mut test_data = vec![0u8; 72];
        test_data[64..72].copy_from_slice(BTRFS_MAGIC);
        assert!(is_btrfs_superblock(&test_data));
        
        let wrong_magic = vec![0u8; 72];
        assert!(!is_btrfs_superblock(&wrong_magic));
    }
}
