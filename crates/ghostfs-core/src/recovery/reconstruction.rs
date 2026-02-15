/// Smart extent reconstruction - Phase 5C
///
/// This module implements intelligent extent reconstruction for files with
/// complex or damaged extent maps, using pattern analysis and data validation.
use anyhow::Result;

use super::fragment_matcher::FragmentMatcher;
use super::fragments::FragmentCatalog;
use crate::{BlockRange, DeletedFile};

/// Result of extent reconstruction
#[derive(Debug, Clone)]
pub struct ReconstructionResult {
    /// Original extent count
    pub original_extents: usize,

    /// Reconstructed extent count
    pub reconstructed_extents: usize,

    /// Confidence in reconstruction (0.0-1.0)
    pub confidence: f32,

    /// New block ranges for the file
    pub block_ranges: Vec<BlockRange>,

    /// Whether reconstruction was successful
    pub success: bool,

    /// Reconstruction strategy used
    pub strategy: ReconstructionStrategy,
}

/// Strategies for extent reconstruction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReconstructionStrategy {
    /// Sequential scan - find contiguous data
    Sequential,

    /// Signature-based - use file signatures to find boundaries
    SignatureBased,

    /// Pattern-based - analyze data patterns
    PatternBased,

    /// Fragment assembly - piece together from fragments
    FragmentAssembly,

    /// Hybrid - combination of strategies
    Hybrid,
}

/// Smart extent reconstructor
pub struct ExtentReconstructor {
    /// Fragment matcher for finding related data
    _matcher: FragmentMatcher,

    /// Minimum confidence threshold
    min_confidence: f32,
}

impl ExtentReconstructor {
    /// Create a new extent reconstructor
    pub fn new() -> Self {
        Self {
            _matcher: FragmentMatcher::new().with_min_confidence(0.5),
            min_confidence: 0.6,
        }
    }

    /// Set minimum confidence threshold
    pub fn with_min_confidence(mut self, confidence: f32) -> Self {
        self.min_confidence = confidence;
        self
    }

    /// Reconstruct extents for a file
    pub fn reconstruct(
        &self,
        file: &DeletedFile,
        device_data: &[u8],
        catalog: &FragmentCatalog,
    ) -> Result<ReconstructionResult> {
        // Choose best reconstruction strategy based on file characteristics
        let strategy = self.choose_strategy(file, catalog);

        let result = match strategy {
            ReconstructionStrategy::Sequential => self.reconstruct_sequential(file, device_data)?,
            ReconstructionStrategy::SignatureBased => {
                self.reconstruct_signature_based(file, device_data)?
            }
            ReconstructionStrategy::PatternBased => {
                self.reconstruct_pattern_based(file, device_data)?
            }
            ReconstructionStrategy::FragmentAssembly => {
                self.reconstruct_from_fragments(file, catalog)?
            }
            ReconstructionStrategy::Hybrid => {
                self.reconstruct_hybrid(file, device_data, catalog)?
            }
        };

        Ok(result)
    }

    /// Choose the best reconstruction strategy
    fn choose_strategy(
        &self,
        file: &DeletedFile,
        catalog: &FragmentCatalog,
    ) -> ReconstructionStrategy {
        // If we have fragments in catalog, try fragment assembly
        if !catalog.is_empty() {
            return ReconstructionStrategy::FragmentAssembly;
        }

        // If file has known signature, use signature-based
        if file.metadata.mime_type.is_some() {
            return ReconstructionStrategy::SignatureBased;
        }

        // For large files, use pattern-based
        if file.size > 1024 * 1024 {
            // > 1MB
            return ReconstructionStrategy::PatternBased;
        }

        // Default to sequential
        ReconstructionStrategy::Sequential
    }

    /// Sequential extent reconstruction
    fn reconstruct_sequential(
        &self,
        file: &DeletedFile,
        device_data: &[u8],
    ) -> Result<ReconstructionResult> {
        let mut block_ranges = Vec::new();
        let block_size = 4096u64;

        if file.data_blocks.is_empty() {
            return Ok(ReconstructionResult {
                original_extents: 0,
                reconstructed_extents: 0,
                confidence: 0.0,
                block_ranges: Vec::new(),
                success: false,
                strategy: ReconstructionStrategy::Sequential,
            });
        }

        // Try to extend existing extents by scanning adjacent blocks
        for block_range in &file.data_blocks {
            let start_offset = (block_range.start_block * block_size) as usize;

            // Check if data exists at this location
            if start_offset < device_data.len() {
                // Try to find actual data boundaries
                let extended_range = self.extend_range(
                    block_range.start_block,
                    block_range.block_count,
                    device_data,
                    block_size,
                );

                block_ranges.push(extended_range);
            } else {
                // Keep original if we can't access data
                block_ranges.push(block_range.clone());
            }
        }

        let confidence = if block_ranges.is_empty() {
            0.0
        } else {
            0.7 // Medium confidence for sequential reconstruction
        };

        let is_empty = block_ranges.is_empty();

        Ok(ReconstructionResult {
            original_extents: file.data_blocks.len(),
            reconstructed_extents: block_ranges.len(),
            confidence,
            block_ranges,
            success: !is_empty,
            strategy: ReconstructionStrategy::Sequential,
        })
    }

    /// Extend a block range by analyzing data
    fn extend_range(
        &self,
        start_block: u64,
        block_count: u64,
        device_data: &[u8],
        block_size: u64,
    ) -> BlockRange {
        let start_offset = (start_block * block_size) as usize;
        let mut current_blocks = block_count;

        // Try extending forward
        loop {
            let next_block_offset = start_offset + (current_blocks as usize * block_size as usize);

            if next_block_offset + block_size as usize > device_data.len() {
                break; // Reached end of device
            }

            // Check if next block contains data (not all zeros)
            let next_block =
                &device_data[next_block_offset..next_block_offset + block_size as usize];
            if self.is_data_block(next_block) {
                current_blocks += 1;
            } else {
                break; // Hit empty block
            }

            // Limit extension to avoid runaway
            if current_blocks > block_count * 2 {
                break;
            }
        }

        BlockRange {
            start_block,
            block_count: current_blocks,
            is_allocated: true,
        }
    }

    /// Check if a block contains data (not all zeros or patterns)
    fn is_data_block(&self, block: &[u8]) -> bool {
        // Check for all zeros
        if block.iter().all(|&b| b == 0) {
            return false;
        }

        // Check for repeating patterns (0xFF, etc.)
        let first_byte = block[0];
        if block.iter().all(|&b| b == first_byte) {
            return false;
        }

        // Has varied data
        true
    }

    /// Signature-based reconstruction
    fn reconstruct_signature_based(
        &self,
        file: &DeletedFile,
        device_data: &[u8],
    ) -> Result<ReconstructionResult> {
        // For now, fall back to sequential
        // TODO: Implement signature scanning
        self.reconstruct_sequential(file, device_data)
    }

    /// Pattern-based reconstruction
    fn reconstruct_pattern_based(
        &self,
        file: &DeletedFile,
        device_data: &[u8],
    ) -> Result<ReconstructionResult> {
        // For now, fall back to sequential
        // TODO: Implement pattern analysis
        self.reconstruct_sequential(file, device_data)
    }

    /// Reconstruct from fragments in catalog
    fn reconstruct_from_fragments(
        &self,
        file: &DeletedFile,
        catalog: &FragmentCatalog,
    ) -> Result<ReconstructionResult> {
        let mut block_ranges = Vec::new();
        let block_size = 4096u64;

        // Find fragments that could belong to this file
        let mut candidates = Vec::new();

        if let Some(ref mime) = file.metadata.mime_type {
            let frags = catalog.fragments_by_signature(mime);
            candidates.extend(frags);
        }

        // Convert fragments to block ranges
        for frag in &candidates {
            let start_block = frag.start_offset / block_size;
            let block_count = (frag.size + block_size - 1) / block_size; // Round up

            block_ranges.push(BlockRange {
                start_block,
                block_count,
                is_allocated: true,
            });
        }

        // Sort by block number
        block_ranges.sort_by_key(|r| r.start_block);

        // Merge adjacent ranges
        block_ranges = self.merge_adjacent_ranges(block_ranges);

        let confidence = if block_ranges.is_empty() {
            0.0
        } else if candidates.len() >= 3 {
            0.8 // High confidence if multiple fragments found
        } else {
            0.6 // Medium confidence
        };

        let is_empty = block_ranges.is_empty();

        Ok(ReconstructionResult {
            original_extents: file.data_blocks.len(),
            reconstructed_extents: block_ranges.len(),
            confidence,
            block_ranges,
            success: !is_empty,
            strategy: ReconstructionStrategy::FragmentAssembly,
        })
    }

    /// Hybrid reconstruction combining multiple strategies
    fn reconstruct_hybrid(
        &self,
        file: &DeletedFile,
        device_data: &[u8],
        catalog: &FragmentCatalog,
    ) -> Result<ReconstructionResult> {
        // Try sequential first
        let mut best_result = self.reconstruct_sequential(file, device_data)?;

        // Try fragment assembly if catalog has data
        if !catalog.is_empty() {
            let frag_result = self.reconstruct_from_fragments(file, catalog)?;

            if frag_result.confidence > best_result.confidence {
                best_result = frag_result;
                best_result.strategy = ReconstructionStrategy::Hybrid;
            }
        }

        Ok(best_result)
    }

    /// Merge adjacent block ranges
    fn merge_adjacent_ranges(&self, ranges: Vec<BlockRange>) -> Vec<BlockRange> {
        if ranges.len() <= 1 {
            return ranges;
        }

        let mut merged = Vec::new();
        let mut current = ranges[0].clone();

        for range in ranges.iter().skip(1) {
            let current_end = current.start_block + current.block_count;

            if range.start_block <= current_end + 1 {
                // Adjacent or overlapping - merge
                let range_end = range.start_block + range.block_count;
                let new_end = current_end.max(range_end);
                current.block_count = new_end - current.start_block;
            } else {
                // Not adjacent - save current and start new
                merged.push(current);
                current = range.clone();
            }
        }

        merged.push(current);
        merged
    }
}

impl Default for ExtentReconstructor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_block_detection() {
        let reconstructor = ExtentReconstructor::new();

        let zeros = vec![0u8; 4096];
        assert!(!reconstructor.is_data_block(&zeros));

        let pattern = vec![0xFFu8; 4096];
        assert!(!reconstructor.is_data_block(&pattern));

        let mut data = vec![0u8; 4096];
        data[100] = 1;
        data[200] = 2;
        assert!(reconstructor.is_data_block(&data));
    }

    #[test]
    fn test_range_merging() {
        let reconstructor = ExtentReconstructor::new();

        let ranges = vec![
            BlockRange {
                start_block: 0,
                block_count: 10,
                is_allocated: true,
            },
            BlockRange {
                start_block: 10,
                block_count: 5,
                is_allocated: true,
            },
            BlockRange {
                start_block: 20,
                block_count: 3,
                is_allocated: true,
            },
        ];

        let merged = reconstructor.merge_adjacent_ranges(ranges);

        assert_eq!(merged.len(), 2); // First two should merge
        assert_eq!(merged[0].block_count, 15);
    }

    #[test]
    fn test_strategy_selection() {
        let reconstructor = ExtentReconstructor::new();
        let catalog = FragmentCatalog::new();

        use crate::{FileMetadata, FileType};
        use std::collections::HashMap;

        let small_file = DeletedFile {
            id: 1,
            inode_or_cluster: 100,
            original_path: None,
            size: 1024,
            deletion_time: None,
            confidence_score: 0.8,
            file_type: FileType::RegularFile,
            data_blocks: vec![],
            is_recoverable: true,
            metadata: FileMetadata {
                mime_type: None,
                file_extension: None,
                permissions: None,
                owner_uid: None,
                owner_gid: None,
                created_time: None,
                modified_time: None,
                accessed_time: None,
                extended_attributes: HashMap::new(),
            },
            fs_metadata: None,
        };

        let strategy = reconstructor.choose_strategy(&small_file, &catalog);
        assert_eq!(strategy, ReconstructionStrategy::Sequential);
    }
}
