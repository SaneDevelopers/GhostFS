//! exFAT directory entry parsing
//!
//! exFAT uses 32-byte directory entries with multiple types:
//! - 0x85: File entry (deleted: 0x05)
//! - 0xC0: Stream extension (deleted: 0x40)  
//! - 0xC1: File name entry (deleted: 0x41)
//! - 0x81: Allocation bitmap
//! - 0x82: Up-case table
//! - 0x83: Volume label

use anyhow::Result;
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Cursor;

/// Directory entry size in bytes
pub const ENTRY_SIZE: usize = 32;

/// Entry type codes (high bit set = in-use)
pub const ENTRY_TYPE_FILE: u8 = 0x85;
pub const ENTRY_TYPE_STREAM: u8 = 0xC0;
pub const ENTRY_TYPE_FILENAME: u8 = 0xC1;
pub const ENTRY_TYPE_BITMAP: u8 = 0x81;
pub const ENTRY_TYPE_UPCASE: u8 = 0x82;
pub const ENTRY_TYPE_LABEL: u8 = 0x83;

/// Deleted entry type codes (high bit cleared)
pub const ENTRY_TYPE_FILE_DELETED: u8 = 0x05;
pub const ENTRY_TYPE_STREAM_DELETED: u8 = 0x40;
pub const ENTRY_TYPE_FILENAME_DELETED: u8 = 0x41;

/// File attributes
pub const ATTR_READ_ONLY: u16 = 0x01;
pub const ATTR_HIDDEN: u16 = 0x02;
pub const ATTR_SYSTEM: u16 = 0x04;
pub const ATTR_DIRECTORY: u16 = 0x10;
pub const ATTR_ARCHIVE: u16 = 0x20;

/// Parsed directory entry
#[derive(Debug, Clone)]
pub enum DirectoryEntry {
    /// File entry (primary entry for a file/directory)
    File(FileEntry),
    /// Stream extension (contains file size and first cluster)
    StreamExtension(StreamExtensionEntry),
    /// File name (15 UTF-16 characters per entry)
    FileName(FileNameEntry),
    /// Allocation bitmap
    Bitmap(BitmapEntry),
    /// Deleted entry (can be recovered)
    Deleted(DeletedEntry),
    /// Unknown or unused entry
    Unknown(u8),
}

/// File entry (primary entry for a file or directory)
#[derive(Debug, Clone)]
pub struct FileEntry {
    /// Entry type (0x85 or 0x05 if deleted)
    pub entry_type: u8,
    /// Number of secondary entries (StreamExtension + FileNames)
    pub secondary_count: u8,
    /// Checksum of all entries in set
    pub set_checksum: u16,
    /// File attributes (directory, hidden, etc.)
    pub file_attributes: u16,
    /// Creation timestamp
    pub create_timestamp: u32,
    /// Last modified timestamp
    pub modify_timestamp: u32,
    /// Last accessed timestamp
    pub access_timestamp: u32,
    /// Is this entry deleted?
    pub is_deleted: bool,
}

/// Stream extension entry (contains size and cluster info)
#[derive(Debug, Clone)]
pub struct StreamExtensionEntry {
    /// Entry type
    pub entry_type: u8,
    /// General flags (NoFatChain bit)
    pub general_flags: u8,
    /// File name length in characters
    pub name_length: u8,
    /// File name hash
    pub name_hash: u16,
    /// Valid data length
    pub valid_data_length: u64,
    /// First cluster of data
    pub first_cluster: u32,
    /// Data length (file size)
    pub data_length: u64,
    /// Is this entry deleted?
    pub is_deleted: bool,
}

/// File name entry (up to 15 UTF-16 characters)
#[derive(Debug, Clone)]
pub struct FileNameEntry {
    /// Entry type
    pub entry_type: u8,
    /// Flags
    pub flags: u8,
    /// Filename fragment (UTF-16)
    pub file_name: String,
    /// Is this entry deleted?
    pub is_deleted: bool,
}

/// Allocation bitmap entry
#[derive(Debug, Clone)]
pub struct BitmapEntry {
    /// Bitmap flags
    pub flags: u8,
    /// First cluster of bitmap
    pub first_cluster: u32,
    /// Bitmap size in bytes
    pub data_length: u64,
}

/// Deleted entry (for recovery)
#[derive(Debug, Clone)]
pub struct DeletedEntry {
    /// Original entry type (with high bit cleared)
    pub original_type: u8,
    /// Raw data for potential recovery
    pub raw_data: Vec<u8>,
}

impl DirectoryEntry {
    /// Parse a single directory entry from 32 bytes
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < ENTRY_SIZE {
            anyhow::bail!("Insufficient data for directory entry");
        }
        
        let entry_type = data[0];
        
        // Check if entry is in-use (high bit set) or deleted
        let in_use = entry_type & 0x80 != 0;
        let _type_code = entry_type & 0x7F;
        
        match entry_type {
            // In-use entries
            ENTRY_TYPE_FILE => Ok(DirectoryEntry::File(FileEntry::parse(data, false)?)),
            ENTRY_TYPE_STREAM => Ok(DirectoryEntry::StreamExtension(StreamExtensionEntry::parse(data, false)?)),
            ENTRY_TYPE_FILENAME => Ok(DirectoryEntry::FileName(FileNameEntry::parse(data, false)?)),
            ENTRY_TYPE_BITMAP => Ok(DirectoryEntry::Bitmap(BitmapEntry::parse(data)?)),
            
            // Deleted entries (recoverable!)
            ENTRY_TYPE_FILE_DELETED => Ok(DirectoryEntry::Deleted(DeletedEntry {
                original_type: entry_type,
                raw_data: data[..ENTRY_SIZE].to_vec(),
            })),
            ENTRY_TYPE_STREAM_DELETED => Ok(DirectoryEntry::Deleted(DeletedEntry {
                original_type: entry_type,
                raw_data: data[..ENTRY_SIZE].to_vec(),
            })),
            ENTRY_TYPE_FILENAME_DELETED => Ok(DirectoryEntry::Deleted(DeletedEntry {
                original_type: entry_type,
                raw_data: data[..ENTRY_SIZE].to_vec(),
            })),
            
            // Unused entry (0x00) or unknown
            0x00 => Ok(DirectoryEntry::Unknown(0x00)),
            _ => {
                if in_use {
                    // Unknown in-use type
                    Ok(DirectoryEntry::Unknown(entry_type))
                } else {
                    // Potentially deleted entry of unknown type
                    Ok(DirectoryEntry::Deleted(DeletedEntry {
                        original_type: entry_type,
                        raw_data: data[..ENTRY_SIZE].to_vec(),
                    }))
                }
            }
        }
    }
    
    /// Check if this is a deleted entry
    pub fn is_deleted(&self) -> bool {
        match self {
            DirectoryEntry::File(f) => f.is_deleted,
            DirectoryEntry::StreamExtension(s) => s.is_deleted,
            DirectoryEntry::FileName(n) => n.is_deleted,
            DirectoryEntry::Deleted(_) => true,
            _ => false,
        }
    }
}

impl FileEntry {
    /// Parse file entry from raw bytes
    pub fn parse(data: &[u8], is_deleted: bool) -> Result<Self> {
        let mut cursor = Cursor::new(data);
        
        let entry_type = cursor.read_u8()?;
        let secondary_count = cursor.read_u8()?;
        let set_checksum = cursor.read_u16::<LittleEndian>()?;
        let file_attributes = cursor.read_u16::<LittleEndian>()?;
        let _reserved1 = cursor.read_u16::<LittleEndian>()?;
        let create_timestamp = cursor.read_u32::<LittleEndian>()?;
        let modify_timestamp = cursor.read_u32::<LittleEndian>()?;
        let access_timestamp = cursor.read_u32::<LittleEndian>()?;
        
        Ok(FileEntry {
            entry_type,
            secondary_count,
            set_checksum,
            file_attributes,
            create_timestamp,
            modify_timestamp,
            access_timestamp,
            is_deleted,
        })
    }
    
    /// Check if this is a directory
    pub fn is_directory(&self) -> bool {
        self.file_attributes & ATTR_DIRECTORY != 0
    }
}

impl StreamExtensionEntry {
    /// Parse stream extension entry from raw bytes
    pub fn parse(data: &[u8], is_deleted: bool) -> Result<Self> {
        let mut cursor = Cursor::new(data);
        
        let entry_type = cursor.read_u8()?;
        let general_flags = cursor.read_u8()?;
        let _reserved = cursor.read_u8()?;
        let name_length = cursor.read_u8()?;
        let name_hash = cursor.read_u16::<LittleEndian>()?;
        let _reserved2 = cursor.read_u16::<LittleEndian>()?;
        let valid_data_length = cursor.read_u64::<LittleEndian>()?;
        let _reserved3 = cursor.read_u32::<LittleEndian>()?;
        let first_cluster = cursor.read_u32::<LittleEndian>()?;
        let data_length = cursor.read_u64::<LittleEndian>()?;
        
        Ok(StreamExtensionEntry {
            entry_type,
            general_flags,
            name_length,
            name_hash,
            valid_data_length,
            first_cluster,
            data_length,
            is_deleted,
        })
    }
    
    /// Check if file uses contiguous allocation (NoFatChain)
    pub fn is_contiguous(&self) -> bool {
        self.general_flags & 0x02 != 0
    }
}

impl FileNameEntry {
    /// Parse file name entry from raw bytes
    pub fn parse(data: &[u8], is_deleted: bool) -> Result<Self> {
        let entry_type = data[0];
        let flags = data[1];
        
        // File name is 15 UTF-16 characters starting at offset 2
        let utf16_data = &data[2..32];
        let file_name = decode_utf16_name(utf16_data)?;
        
        Ok(FileNameEntry {
            entry_type,
            flags,
            file_name,
            is_deleted,
        })
    }
}

impl BitmapEntry {
    /// Parse allocation bitmap entry
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut cursor = Cursor::new(data);
        
        let _entry_type = cursor.read_u8()?;
        let flags = cursor.read_u8()?;
        let _reserved = [0u8; 18];
        cursor.set_position(20);
        let first_cluster = cursor.read_u32::<LittleEndian>()?;
        let data_length = cursor.read_u64::<LittleEndian>()?;
        
        Ok(BitmapEntry {
            flags,
            first_cluster,
            data_length,
        })
    }
}

impl DeletedEntry {
    /// Try to recover file entry from deleted entry
    pub fn recover_as_file(&self) -> Option<FileEntry> {
        if self.original_type == ENTRY_TYPE_FILE_DELETED {
            // Set high bit to make it look like valid entry for parsing
            let mut data = self.raw_data.clone();
            data[0] = ENTRY_TYPE_FILE;
            FileEntry::parse(&data, true).ok()
        } else {
            None
        }
    }
    
    /// Try to recover stream extension from deleted entry
    pub fn recover_as_stream(&self) -> Option<StreamExtensionEntry> {
        if self.original_type == ENTRY_TYPE_STREAM_DELETED {
            let mut data = self.raw_data.clone();
            data[0] = ENTRY_TYPE_STREAM;
            StreamExtensionEntry::parse(&data, true).ok()
        } else {
            None
        }
    }
    
    /// Try to recover filename from deleted entry
    pub fn recover_as_filename(&self) -> Option<FileNameEntry> {
        if self.original_type == ENTRY_TYPE_FILENAME_DELETED {
            let mut data = self.raw_data.clone();
            data[0] = ENTRY_TYPE_FILENAME;
            FileNameEntry::parse(&data, true).ok()
        } else {
            None
        }
    }
}

/// Decode UTF-16LE filename, stopping at null terminator
fn decode_utf16_name(data: &[u8]) -> Result<String> {
    let utf16_chars: Vec<u16> = data
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .take_while(|&c| c != 0)
        .collect();
    
    String::from_utf16(&utf16_chars)
        .map_err(|e| anyhow::anyhow!("Invalid UTF-16 filename: {}", e))
}

/// A complete file entry set (File + StreamExtension + FileNames)
#[derive(Debug, Clone)]
pub struct FileEntrySet {
    /// Primary file entry
    pub file_entry: FileEntry,
    /// Stream extension with size/cluster info
    pub stream_extension: StreamExtensionEntry,
    /// Complete filename (concatenated from FileName entries)
    pub filename: String,
    /// Is this a deleted file entry set?
    pub is_deleted: bool,
}

impl FileEntrySet {
    /// Parse a complete file entry set from directory data
    pub fn parse_from_entries(entries: &[DirectoryEntry]) -> Option<Self> {
        if entries.is_empty() {
            return None;
        }
        
        // First entry must be File entry
        let file_entry = match &entries[0] {
            DirectoryEntry::File(f) => f.clone(),
            DirectoryEntry::Deleted(d) => d.recover_as_file()?,
            _ => return None,
        };
        
        let expected_count = file_entry.secondary_count as usize;
        if entries.len() < expected_count + 1 {
            return None;
        }
        
        // Second entry must be StreamExtension
        let stream_extension = match &entries[1] {
            DirectoryEntry::StreamExtension(s) => s.clone(),
            DirectoryEntry::Deleted(d) => d.recover_as_stream()?,
            _ => return None,
        };
        
        // Remaining entries are FileName entries
        let mut filename = String::new();
        for entry in entries.iter().skip(2).take(expected_count - 1) {
            match entry {
                DirectoryEntry::FileName(n) => filename.push_str(&n.file_name),
                DirectoryEntry::Deleted(d) => {
                    if let Some(n) = d.recover_as_filename() {
                        filename.push_str(&n.file_name);
                    }
                }
                _ => {}
            }
        }
        
        // Trim filename to actual length
        let name_length = stream_extension.name_length as usize;
        if filename.chars().count() > name_length {
            filename = filename.chars().take(name_length).collect();
        }
        
        let is_deleted = file_entry.is_deleted || stream_extension.is_deleted;
        
        Some(FileEntrySet {
            file_entry,
            stream_extension,
            filename,
            is_deleted,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_entry_type_detection() {
        assert_eq!(ENTRY_TYPE_FILE & 0x80, 0x80); // High bit set
        assert_eq!(ENTRY_TYPE_FILE_DELETED & 0x80, 0x00); // High bit clear
    }
    
    #[test]
    fn test_utf16_decode() {
        // "test" in UTF-16LE
        let data = [0x74, 0x00, 0x65, 0x00, 0x73, 0x00, 0x74, 0x00, 0x00, 0x00];
        let name = decode_utf16_name(&data).unwrap();
        assert_eq!(name, "test");
    }
    
    #[test]
    fn test_file_attributes() {
        let entry = FileEntry {
            entry_type: 0x85,
            secondary_count: 2,
            set_checksum: 0,
            file_attributes: ATTR_DIRECTORY,
            create_timestamp: 0,
            modify_timestamp: 0,
            access_timestamp: 0,
            is_deleted: false,
        };
        
        assert!(entry.is_directory());
    }
}
