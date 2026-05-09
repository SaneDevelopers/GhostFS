use super::fragment_matcher::FragmentMatcher;
/// Partial file recovery - Phase 5B
///
/// This module handles recovery of partially overwritten or fragmented files
/// by intelligently reconstructing what can be recovered and marking gaps.
use super::fragments::{Fragment, FragmentCatalog, FragmentId};
use crate::DeletedFile;

/// Result of partial file recovery
#[derive(Debug, Clone)]
pub struct PartialRecoveryResult {
    /// Total file size expected
    pub expected_size: u64,

    /// Bytes successfully recovered
    pub recovered_bytes: u64,

    /// Recovery completeness ratio (0.0-1.0)
    pub completeness: f32,

    /// Gaps in the recovered file
    pub gaps: Vec<GapInfo>,

    /// Fragments that were assembled
    pub assembled_fragments: Vec<FragmentId>,

    /// Whether the file is usable (enough data recovered)
    pub is_usable: bool,
}

/// Information about a gap in the recovered file
#[derive(Debug, Clone)]
pub struct GapInfo {
    /// Byte offset where gap starts
    pub offset: u64,

    /// Size of the gap in bytes
    pub size: u64,

    /// Whether this gap is critical for file integrity
    pub is_critical: bool,
}

/// Partial file recovery engine
pub struct PartialRecovery {
    /// Fragment matcher for finding related fragments
    _matcher: FragmentMatcher,

    /// Minimum completeness threshold
    min_completeness: f32,
}

impl PartialRecovery {
    /// Create a new partial recovery engine
    pub fn new() -> Self {
        Self {
            _matcher: FragmentMatcher::new().with_min_confidence(0.4),
            min_completeness: 0.3, // Recover if at least 30% of file is available
        }
    }

    /// Set minimum completeness threshold
    pub fn with_min_completeness(mut self, min_completeness: f32) -> Self {
        self.min_completeness = min_completeness;
        self
    }

    /// Attempt partial recovery of a file
    pub fn recover_partial(
        &self,
        file: &DeletedFile,
        catalog: &FragmentCatalog,
    ) -> Option<PartialRecoveryResult> {
        // Find all fragments that could belong to this file
        let candidates = self.find_candidate_fragments(file, catalog);

        if candidates.is_empty() {
            return None;
        }

        // Build file map from fragments
        let file_map = self.build_file_map(file.size, &candidates, catalog);

        // Calculate recovery metrics
        let recovered_bytes = file_map.covered_bytes();
        let completeness = recovered_bytes as f32 / file.size as f32;

        if completeness < self.min_completeness {
            return None; // Not enough data recovered
        }

        // Identify gaps
        let gaps = self.identify_gaps(&file_map, file.size);

        // Determine if file is usable
        let is_usable = self.is_file_usable(completeness, &gaps, file);

        Some(PartialRecoveryResult {
            expected_size: file.size,
            recovered_bytes,
            completeness,
            gaps,
            assembled_fragments: file_map.fragment_ids(),
            is_usable,
        })
    }

    /// Find candidate fragments for a file
    fn find_candidate_fragments(
        &self,
        file: &DeletedFile,
        catalog: &FragmentCatalog,
    ) -> Vec<FragmentId> {
        let mut candidates = Vec::new();

        // Search by signature if file type is known
        if let Some(ref mime) = file.metadata.mime_type {
            let sig_fragments = catalog.fragments_by_signature(mime);
            candidates.extend(sig_fragments.iter().map(|f| f.id));
        }

        // Search by size range (fragments should be smaller than file)
        let size_fragments = catalog.fragments_by_size_range(0, file.size);
        candidates.extend(size_fragments.iter().map(|f| f.id));

        // Search by location (near file's data blocks)
        for block_range in &file.data_blocks {
            let offset = block_range.start_block * 4096; // Assume 4KB blocks
            let nearby = catalog.fragments_near_location(offset, 1024 * 1024); // 1MB range
            candidates.extend(nearby.iter().map(|f| f.id));
        }

        // Deduplicate
        candidates.sort_unstable();
        candidates.dedup();

        candidates
    }

    /// Build a file map from fragments
    fn build_file_map(
        &self,
        file_size: u64,
        fragment_ids: &[FragmentId],
        catalog: &FragmentCatalog,
    ) -> FileMap {
        let mut file_map = FileMap::new(file_size);

        for &frag_id in fragment_ids {
            if let Some(fragment) = catalog.get(frag_id) {
                // Try to place fragment in the file
                // For now, use simple heuristic: order by disk location
                file_map.add_fragment(frag_id, fragment);
            }
        }

        file_map
    }

    /// Identify gaps in the recovered file
    fn identify_gaps(&self, file_map: &FileMap, total_size: u64) -> Vec<GapInfo> {
        let mut gaps = Vec::new();
        let mut current_offset = 0u64;

        for segment in &file_map.segments {
            if segment.offset > current_offset {
                // Found a gap
                gaps.push(GapInfo {
                    offset: current_offset,
                    size: segment.offset - current_offset,
                    is_critical: current_offset < 4096, // Gaps at file start are critical
                });
            }
            current_offset = segment.offset + segment.size;
        }

        // Final gap at end of file
        if current_offset < total_size {
            gaps.push(GapInfo {
                offset: current_offset,
                size: total_size - current_offset,
                is_critical: false, // End gaps usually less critical
            });
        }

        gaps
    }

    /// Determine if partially recovered file is usable
    fn is_file_usable(&self, completeness: f32, gaps: &[GapInfo], file: &DeletedFile) -> bool {
        // High completeness = likely usable
        if completeness >= 0.9 {
            return true;
        }

        // Check for critical gaps
        let has_critical_gaps = gaps.iter().any(|g| g.is_critical);
        if has_critical_gaps {
            return false;
        }

        // File-type specific rules
        if let Some(ref mime) = file.metadata.mime_type {
            match mime.as_str() {
                // Text files are usable even with gaps
                "text/plain" | "text/html" | "text/csv" => completeness >= 0.5,

                // Images need header + reasonable amount of data
                "image/jpeg" | "image/png" | "image/gif" => {
                    completeness >= 0.6 && !has_critical_gaps
                }

                // Archives/executables need high completeness
                "application/zip" | "application/x-executable" => completeness >= 0.95,

                // Default: medium threshold
                _ => completeness >= 0.7,
            }
        } else {
            // Unknown type: conservative threshold
            completeness >= 0.8
        }
    }
}

impl Default for PartialRecovery {
    fn default() -> Self {
        Self::new()
    }
}

/// Map of file segments recovered from fragments
#[derive(Debug)]
struct FileMap {
    /// Total file size
    total_size: u64,

    /// Segments of the file that have been recovered
    segments: Vec<FileSegment>,
}

#[derive(Debug, Clone)]
struct FileSegment {
    /// Byte offset in the file
    offset: u64,

    /// Size of this segment
    size: u64,

    /// Fragment ID that provides this data
    fragment_id: FragmentId,
}

impl FileMap {
    fn new(total_size: u64) -> Self {
        Self {
            total_size,
            segments: Vec::new(),
        }
    }

    fn add_fragment(&mut self, frag_id: FragmentId, fragment: &Fragment) {
        // Simple placement: use fragment offset as file offset
        // In a more sophisticated system, we'd analyze content to find the right position
        let segment = FileSegment {
            offset: fragment.start_offset % self.total_size, // Wrap to file size
            size: fragment.size.min(self.total_size),        // Don't exceed file size
            fragment_id: frag_id,
        };

        self.segments.push(segment);

        // Sort segments by offset
        self.segments.sort_by_key(|s| s.offset);

        // Merge overlapping segments (keep the first one)
        self.merge_overlaps();
    }

    fn merge_overlaps(&mut self) {
        if self.segments.len() <= 1 {
            return;
        }

        let mut merged = Vec::new();
        let mut current = self.segments[0].clone();

        for segment in self.segments.iter().skip(1) {
            if segment.offset <= current.offset + current.size {
                // Overlap detected - extend current if this one goes further
                let current_end = current.offset + current.size;
                let segment_end = segment.offset + segment.size;

                if segment_end > current_end {
                    current.size = segment_end - current.offset;
                }
            } else {
                // No overlap - save current and start new
                merged.push(current);
                current = segment.clone();
            }
        }

        merged.push(current);
        self.segments = merged;
    }

    fn covered_bytes(&self) -> u64 {
        self.segments.iter().map(|s| s.size).sum()
    }

    fn fragment_ids(&self) -> Vec<FragmentId> {
        self.segments.iter().map(|s| s.fragment_id).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gap_identification() {
        let mut file_map = FileMap::new(10000);

        // Add some segments with gaps
        file_map.segments = vec![
            FileSegment {
                offset: 0,
                size: 1000,
                fragment_id: 1,
            },
            FileSegment {
                offset: 2000,
                size: 1000,
                fragment_id: 2,
            },
            FileSegment {
                offset: 5000,
                size: 2000,
                fragment_id: 3,
            },
        ];

        let recovery = PartialRecovery::new();
        let gaps = recovery.identify_gaps(&file_map, 10000);

        assert_eq!(gaps.len(), 3); // Gap at 1000-2000, 3000-5000, 7000-10000
        assert_eq!(gaps[0].offset, 1000);
        assert_eq!(gaps[0].size, 1000);
    }

    #[test]
    fn test_completeness_calculation() {
        let mut file_map = FileMap::new(10000);
        file_map.segments = vec![FileSegment {
            offset: 0,
            size: 5000,
            fragment_id: 1,
        }];

        let covered = file_map.covered_bytes();
        assert_eq!(covered, 5000);

        let completeness = covered as f32 / 10000.0;
        assert_eq!(completeness, 0.5);
    }

    #[test]
    fn test_segment_merging() {
        let mut file_map = FileMap::new(10000);

        // Add overlapping segments
        file_map.segments = vec![
            FileSegment {
                offset: 0,
                size: 1000,
                fragment_id: 1,
            },
            FileSegment {
                offset: 500,
                size: 1000,
                fragment_id: 2,
            },
        ];

        file_map.merge_overlaps();

        assert_eq!(file_map.segments.len(), 1);
        assert_eq!(file_map.segments[0].size, 1500);
    }
}
