/// Common types and utilities for file system access
use anyhow::Result;
use memmap2::MmapOptions;
use std::fs::File;
use std::path::Path;

/// A memory-mapped file for efficient large file access
pub struct BlockDevice {
    _file: File,
    mmap: memmap2::Mmap,
    size: u64,
}

impl BlockDevice {
    /// Open a block device or image file
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::open(path.as_ref())?;
        let size = file.metadata()?.len();
        
        let mmap = unsafe {
            MmapOptions::new()
                .map(&file)?
        };

        Ok(BlockDevice {
            _file: file,
            mmap,
            size,
        })
    }

    /// Get the size of the device in bytes
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Read a slice of bytes from the device
    pub fn read_bytes(&self, offset: u64, length: usize) -> Result<&[u8]> {
        let start = offset as usize;
        let end = start + length;
        
        if end > self.mmap.len() {
            anyhow::bail!("Read beyond end of device: {} > {}", end, self.mmap.len());
        }
        
        Ok(&self.mmap[start..end])
    }

    /// Read a single sector (512 bytes)
    pub fn read_sector(&self, sector: u64) -> Result<&[u8]> {
        self.read_bytes(sector * 512, 512)
    }

    /// Read multiple sectors
    pub fn read_sectors(&self, start_sector: u64, count: u32) -> Result<&[u8]> {
        let offset = start_sector * 512;
        let length = (count as u64 * 512) as usize;
        self.read_bytes(offset, length)
    }

    /// Read data at a specific block offset
    pub fn read_block(&self, block_number: u64, block_size: u32) -> Result<&[u8]> {
        let offset = block_number * block_size as u64;
        self.read_bytes(offset, block_size as usize)
    }
}

/// Common block range representation
#[derive(Debug, Clone)]
pub struct BlockRange {
    pub start: u64,
    pub count: u64,
}

impl BlockRange {
    pub fn new(start: u64, count: u64) -> Self {
        Self { start, count }
    }

    pub fn end(&self) -> u64 {
        self.start + self.count
    }

    pub fn contains(&self, block: u64) -> bool {
        block >= self.start && block < self.end()
    }
}

/// Magic number detection for file types
pub struct MagicDetector;

impl MagicDetector {
    /// Detect file type from first few bytes
    pub fn detect_file_type(data: &[u8]) -> Option<&'static str> {
        if data.len() < 8 {
            return None;
        }

        // Check common file signatures
        match &data[0..4] {
            // Images
            [0xFF, 0xD8, 0xFF, ..] => Some("image/jpeg"),
            [0x89, 0x50, 0x4E, 0x47] => Some("image/png"),
            [0x47, 0x49, 0x46, 0x38] => Some("image/gif"),
            // Documents
            [0x25, 0x50, 0x44, 0x46] => Some("application/pdf"),
            // Archives
            [0x50, 0x4B, 0x03, 0x04] => Some("application/zip"),
            [0x50, 0x4B, 0x05, 0x06] => Some("application/zip"),
            [0x50, 0x4B, 0x07, 0x08] => Some("application/zip"),
            // Executables
            [0x7F, 0x45, 0x4C, 0x46] => Some("application/x-executable"), // ELF
            [0x4D, 0x5A, ..] => Some("application/x-executable"),        // PE/COFF
            // Media
            [0x00, 0x00, 0x00, 0x18] if data.len() >= 8 && &data[4..8] == b"ftyp" => Some("video/mp4"),
            [0x00, 0x00, 0x00, 0x1C] if data.len() >= 8 && &data[4..8] == b"ftyp" => Some("video/mp4"),
            [0x00, 0x00, 0x00, 0x20] if data.len() >= 8 && &data[4..8] == b"ftyp" => Some("video/mp4"),
            _ => None,
        }
    }

    /// Check if data looks like text
    pub fn is_text(data: &[u8]) -> bool {
        if data.is_empty() {
            return false;
        }

        // Simple heuristic: check if most bytes are printable ASCII or common UTF-8
        let printable_count = data.iter()
            .take(1024) // Only check first 1KB
            .filter(|&&b| {
                // Printable ASCII + tab, newline, carriage return
                (b >= 0x20 && b <= 0x7E) || b == 0x09 || b == 0x0A || b == 0x0D
            })
            .count();

        let total_checked = std::cmp::min(data.len(), 1024);
        (printable_count as f32 / total_checked as f32) > 0.8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_magic_detection() {
        // JPEG (needs 8 bytes minimum)
        let jpeg_header = [0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46];
        assert_eq!(MagicDetector::detect_file_type(&jpeg_header), Some("image/jpeg"));

        // PNG
        let png_header = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        assert_eq!(MagicDetector::detect_file_type(&png_header), Some("image/png"));

        // PDF (needs 8 bytes minimum)
        let pdf_header = [0x25, 0x50, 0x44, 0x46, 0x2D, 0x31, 0x2E, 0x34];
        assert_eq!(MagicDetector::detect_file_type(&pdf_header), Some("application/pdf"));
    }

    #[test]
    fn test_text_detection() {
        let text_data = b"Hello, world! This is plain text.";
        assert!(MagicDetector::is_text(text_data));

        let binary_data = [0x00, 0xFF, 0x80, 0x7F, 0x90];
        assert!(!MagicDetector::is_text(&binary_data));
    }

    #[test]
    fn test_block_range() {
        let range = BlockRange::new(10, 5);
        assert_eq!(range.end(), 15);
        assert!(range.contains(12));
        assert!(!range.contains(15));
        assert!(!range.contains(9));
    }
}
