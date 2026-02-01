/// exFAT file system support
use anyhow::Result;
use byteorder::{LittleEndian, ReadBytesExt};
use encoding_rs::UTF_16LE;
use std::io::Cursor;

use super::common::BlockDevice;

// Sub-modules
pub mod directory;
pub mod fat;
pub mod recovery;

/// exFAT file system signature
const EXFAT_SIGNATURE: &[u8; 8] = b"EXFAT   ";

/// exFAT boot sector structure (simplified)
#[derive(Debug)]
pub struct ExFatBootSector {
    pub jump_boot: [u8; 3],
    pub file_system_name: [u8; 8],
    pub partition_offset: u64,
    pub volume_length: u64,
    pub fat_offset: u32,
    pub fat_length: u32,
    pub cluster_heap_offset: u32,
    pub cluster_count: u32,
    pub first_cluster_of_root_directory: u32,
    pub volume_serial_number: u32,
    pub file_system_revision: u16,
    pub volume_flags: u16,
    pub bytes_per_sector_shift: u8,
    pub sectors_per_cluster_shift: u8,
    pub number_of_fats: u8,
    pub drive_select: u8,
    pub percent_in_use: u8,
}

impl ExFatBootSector {
    /// Parse exFAT boot sector from raw bytes
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < 512 {
            anyhow::bail!("Insufficient data for exFAT boot sector");
        }

        let mut cursor = Cursor::new(data);

        let mut jump_boot = [0u8; 3];
        std::io::Read::read_exact(&mut cursor, &mut jump_boot)?;

        let mut file_system_name = [0u8; 8];
        std::io::Read::read_exact(&mut cursor, &mut file_system_name)?;

        if &file_system_name != EXFAT_SIGNATURE {
            anyhow::bail!("Invalid exFAT signature");
        }

        // Skip reserved area (53 bytes)
        cursor.set_position(64);

        let partition_offset = cursor.read_u64::<LittleEndian>()?;
        let volume_length = cursor.read_u64::<LittleEndian>()?;
        let fat_offset = cursor.read_u32::<LittleEndian>()?;
        let fat_length = cursor.read_u32::<LittleEndian>()?;
        let cluster_heap_offset = cursor.read_u32::<LittleEndian>()?;
        let cluster_count = cursor.read_u32::<LittleEndian>()?;
        let first_cluster_of_root_directory = cursor.read_u32::<LittleEndian>()?;
        let volume_serial_number = cursor.read_u32::<LittleEndian>()?;
        let file_system_revision = cursor.read_u16::<LittleEndian>()?;
        let volume_flags = cursor.read_u16::<LittleEndian>()?;
        let bytes_per_sector_shift = cursor.read_u8()?;
        let sectors_per_cluster_shift = cursor.read_u8()?;
        let number_of_fats = cursor.read_u8()?;
        let drive_select = cursor.read_u8()?;
        let percent_in_use = cursor.read_u8()?;

        Ok(ExFatBootSector {
            jump_boot,
            file_system_name,
            partition_offset,
            volume_length,
            fat_offset,
            fat_length,
            cluster_heap_offset,
            cluster_count,
            first_cluster_of_root_directory,
            volume_serial_number,
            file_system_revision,
            volume_flags,
            bytes_per_sector_shift,
            sectors_per_cluster_shift,
            number_of_fats,
            drive_select,
            percent_in_use,
        })
    }

    /// Get bytes per sector
    pub fn bytes_per_sector(&self) -> u32 {
        1 << self.bytes_per_sector_shift
    }

    /// Get sectors per cluster
    pub fn sectors_per_cluster(&self) -> u32 {
        1 << self.sectors_per_cluster_shift
    }

    /// Get bytes per cluster
    pub fn bytes_per_cluster(&self) -> u32 {
        self.bytes_per_sector() * self.sectors_per_cluster()
    }
}

/// Check if data contains exFAT boot sector signature
pub fn is_exfat_boot_sector(data: &[u8]) -> bool {
    if data.len() < 11 {
        return false;
    }

    // exFAT signature is at offset 3
    &data[3..11] == EXFAT_SIGNATURE
}

/// Get exFAT file system information
pub fn get_filesystem_info(device: &BlockDevice) -> Result<String> {
    let sector0 = device.read_sector(0)?;
    let boot_sector = ExFatBootSector::parse(sector0)?;

    let bytes_per_sector = boot_sector.bytes_per_sector();
    let bytes_per_cluster = boot_sector.bytes_per_cluster();
    let volume_size_mb = (boot_sector.volume_length * bytes_per_sector as u64) / (1024 * 1024);
    let cluster_heap_size_mb =
        (boot_sector.cluster_count as u64 * bytes_per_cluster as u64) / (1024 * 1024);

    Ok(format!(
        "exFAT File System\n\
         - Bytes per Sector: {}\n\
         - Sectors per Cluster: {}\n\
         - Bytes per Cluster: {}\n\
         - Volume Size: {} MB\n\
         - Cluster Count: {}\n\
         - Cluster Heap Size: {} MB\n\
         - FAT Offset: {} sectors\n\
         - FAT Length: {} sectors\n\
         - Root Directory Cluster: {}\n\
         - Volume Serial: 0x{:08X}\n\
         - File System Revision: {}.{}\n\
         - Percent In Use: {}%",
        bytes_per_sector,
        boot_sector.sectors_per_cluster(),
        bytes_per_cluster,
        volume_size_mb,
        boot_sector.cluster_count,
        cluster_heap_size_mb,
        boot_sector.fat_offset,
        boot_sector.fat_length,
        boot_sector.first_cluster_of_root_directory,
        boot_sector.volume_serial_number,
        boot_sector.file_system_revision >> 8,
        boot_sector.file_system_revision & 0xFF,
        boot_sector.percent_in_use
    ))
}

/// Decode UTF-16LE filename from exFAT directory entry
pub fn decode_utf16_filename(utf16_data: &[u8]) -> Result<String> {
    // Remove null terminator and trailing nulls
    let mut end = utf16_data.len();
    while end >= 2 && utf16_data[end - 2] == 0 && utf16_data[end - 1] == 0 {
        end -= 2;
    }

    let (decoded, _encoding, had_errors) = UTF_16LE.decode(&utf16_data[..end]);
    if had_errors {
        anyhow::bail!("Invalid UTF-16 filename");
    }

    Ok(decoded.into_owned())
}

/// Scan for deleted files in exFAT
pub fn scan_for_deleted_files(device: &BlockDevice) -> Result<Vec<crate::DeletedFile>> {
    // Parse boot sector
    let sector0 = device.read_sector(0)?;
    let boot_sector = ExFatBootSector::parse(sector0)?;

    tracing::info!("exFAT scan: Starting recovery analysis");
    tracing::info!(
        "  Volume size: {} MB",
        (boot_sector.volume_length * boot_sector.bytes_per_sector() as u64) / (1024 * 1024)
    );
    tracing::info!("  Cluster size: {} bytes", boot_sector.bytes_per_cluster());
    tracing::info!(
        "  Root directory cluster: {}",
        boot_sector.first_cluster_of_root_directory
    );

    // Create and use the recovery engine
    let recovery_engine = recovery::ExFatRecoveryEngine::new(device, boot_sector)?;
    let deleted_files = recovery_engine.scan_deleted_files()?;

    tracing::info!("exFAT scan complete: {} files found", deleted_files.len());

    Ok(deleted_files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exfat_signature_detection() {
        let mut test_data = vec![0u8; 11];
        test_data[3..11].copy_from_slice(EXFAT_SIGNATURE);
        assert!(is_exfat_boot_sector(&test_data));

        let wrong_signature = vec![0u8; 11];
        assert!(!is_exfat_boot_sector(&wrong_signature));
    }

    #[test]
    fn test_utf16_decoding() {
        // "test.txt" in UTF-16LE with null terminator
        let utf16_data = [
            0x74, 0x00, 0x65, 0x00, 0x73, 0x00, 0x74, 0x00, 0x2E, 0x00, 0x74, 0x00, 0x78, 0x00,
            0x74, 0x00, 0x00, 0x00,
        ];

        let decoded = decode_utf16_filename(&utf16_data).unwrap();
        assert_eq!(decoded, "test.txt");
    }
}
