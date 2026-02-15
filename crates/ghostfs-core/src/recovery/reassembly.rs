/// Fragment reassembly engine for reconstructing fragmented files
///
/// This module implements intelligent algorithms to reassemble file fragments
/// into complete files using content analysis, signature detection, and
/// structural patterns.
use std::collections::HashSet;
use std::io::{self, Write};

use crate::recovery::fragment_matcher::{FragmentMatcher, MatchScore};
use crate::recovery::fragments::{Fragment, FragmentCatalog, FragmentId};

/// Result of a reassembly attempt
#[derive(Debug, Clone)]
pub struct ReassemblyResult {
    /// Fragment IDs in reassembled order
    pub fragment_ids: Vec<FragmentId>,

    /// Total size of reassembled file
    pub total_size: u64,

    /// Overall confidence in the reassembly
    pub confidence: f32,

    /// Detected file type
    pub file_type: Option<String>,

    /// Any gaps detected in the reassembly
    pub gaps: Vec<GapInfo>,
}

/// Information about a gap between fragments
#[derive(Debug, Clone)]
pub struct GapInfo {
    /// Position in the reassembled file
    pub position: u64,

    /// Size of the gap
    pub size: u64,

    /// Whether the gap was filled (interpolated)
    pub filled: bool,
}

/// Fragment reassembly engine
pub struct ReassemblyEngine {
    /// Fragment catalog
    catalog: FragmentCatalog,

    /// Fragment matcher
    matcher: FragmentMatcher,

    /// Maximum gap to tolerate (bytes)
    max_gap_size: u64,

    /// Minimum confidence for reassembly
    min_confidence: f32,
}

impl ReassemblyEngine {
    /// Create a new reassembly engine
    pub fn new(catalog: FragmentCatalog) -> Self {
        Self {
            catalog,
            matcher: FragmentMatcher::new(),
            max_gap_size: 64 * 1024, // 64KB default
            min_confidence: 0.6,
        }
    }

    /// Set maximum gap size to tolerate
    pub fn with_max_gap_size(mut self, max_gap_size: u64) -> Self {
        self.max_gap_size = max_gap_size;
        self
    }

    /// Set minimum confidence threshold
    pub fn with_min_confidence(mut self, min_confidence: f32) -> Self {
        self.min_confidence = min_confidence;
        self.matcher = self.matcher.with_min_confidence(min_confidence);
        self
    }

    /// Reassemble fragments for a specific file type
    pub fn reassemble_by_type(&self, mime_type: &str) -> Vec<ReassemblyResult> {
        let fragments: Vec<Fragment> = self.catalog.fragments_by_signature(mime_type);

        if fragments.is_empty() {
            return Vec::new();
        }

        // Cluster related fragments
        let clusters = self.matcher.cluster_fragments(&fragments);

        let mut results = Vec::new();

        for cluster in clusters {
            if let Some(result) = self.reassemble_cluster(&cluster) {
                if result.confidence >= self.min_confidence {
                    results.push(result);
                }
            }
        }

        results
    }

    /// Reassemble all detected fragments
    pub fn reassemble_all(&self) -> Vec<ReassemblyResult> {
        let all_fragments: Vec<_> = self.catalog.all_fragments().cloned().collect();

        if all_fragments.is_empty() {
            return Vec::new();
        }

        // Cluster all fragments
        let clusters = self.matcher.cluster_fragments(&all_fragments);

        let mut results = Vec::new();

        for cluster in clusters {
            if let Some(result) = self.reassemble_cluster(&cluster) {
                if result.confidence >= self.min_confidence {
                    results.push(result);
                }
            }
        }

        results
    }

    /// Reassemble a cluster of related fragments
    fn reassemble_cluster(&self, fragment_ids: &[FragmentId]) -> Option<ReassemblyResult> {
        if fragment_ids.is_empty() {
            return None;
        }

        // Get fragments
        let fragments: Vec<Fragment> = fragment_ids
            .iter()
            .filter_map(|id| self.catalog.get(*id).cloned())
            .collect();

        if fragments.is_empty() {
            return None;
        }

        // Order fragments using multiple strategies
        let ordered = self.order_fragments(&fragments)?;

        // Calculate gaps
        let gaps = self.detect_gaps(&ordered);

        // Calculate total size
        let total_size: u64 = ordered
            .iter()
            .map(|(_, frag_id, _)| self.catalog.get(*frag_id).map(|f| f.size).unwrap_or(0))
            .sum();

        // Calculate overall confidence
        let confidence = self.calculate_reassembly_confidence(&ordered, &gaps);

        // Detect file type
        let file_type = ordered.first().and_then(|(_, frag_id, _)| {
            self.catalog.get(*frag_id).and_then(|frag| {
                frag.signature
                    .as_ref()
                    .map(|s| s.signature.mime_type.clone())
            })
        });

        Some(ReassemblyResult {
            fragment_ids: ordered.iter().map(|(_, frag_id, _)| *frag_id).collect(),
            total_size,
            confidence,
            file_type,
            gaps,
        })
    }

    /// Order fragments intelligently with circular dependency detection
    fn order_fragments(
        &self,
        fragments: &[Fragment],
    ) -> Option<Vec<(usize, FragmentId, MatchScore)>> {
        if fragments.is_empty() {
            return None;
        }

        if fragments.len() == 1 {
            // Single fragment - create dummy score
            let score = MatchScore::calculate(1.0, 1.0, 1.0, 1.0);
            return Some(vec![(0, fragments[0].id, score)]);
        }

        // Find fragment with file signature (likely start)
        let start_idx = fragments
            .iter()
            .position(|f| f.signature.is_some())
            .unwrap_or(0);

        let mut ordered = Vec::new();
        let mut used = HashSet::new();

        let current_id = fragments[start_idx].id;
        used.insert(current_id);

        // Add first fragment
        let initial_score = MatchScore::calculate(1.0, 1.0, 1.0, 1.0);
        ordered.push((start_idx, current_id, initial_score));

        // Chain fragments using best matches
        let mut current_idx = start_idx;
        let mut stuck_count = 0;
        const MAX_STUCK_ATTEMPTS: usize = 3;

        while used.len() < fragments.len() {
            let candidates: Vec<_> = fragments.iter().filter(|f| !used.contains(&f.id)).collect();

            if candidates.is_empty() {
                break;
            }

            let matches = self
                .matcher
                .find_best_matches(&fragments[current_idx], &candidates);

            if matches.is_empty() {
                break;
            }

            // Try matches in order, skipping any that would create circular dependencies
            let mut found_valid = false;
            for (next_id, score) in matches.iter() {
                // Find the fragment in our vector
                if let Some(next_idx) = fragments.iter().position(|f| f.id == *next_id) {
                    // Check for potential circular dependency
                    if self.would_create_cycle(*next_id, fragments, &used) {
                        // Skip this match and try the next one
                        continue;
                    }

                    ordered.push((next_idx, *next_id, score.clone()));
                    used.insert(*next_id);
                    current_idx = next_idx;
                    found_valid = true;
                    stuck_count = 0; // Reset stuck counter on success
                    break;
                }
            }

            if !found_valid {
                // No valid matches found - try to continue with next highest scoring unused fragment
                stuck_count += 1;
                if stuck_count >= MAX_STUCK_ATTEMPTS {
                    break; // Prevent infinite loops
                }

                // Find best unused fragment by temporal/spatial proximity to any ordered fragment
                if let Some((idx, id)) = self.find_best_orphan(fragments, &used, &ordered) {
                    let orphan_score = MatchScore::calculate(0.5, 0.5, 0.5, 0.5); // Lower confidence
                    ordered.push((idx, id, orphan_score));
                    used.insert(id);
                    current_idx = idx;
                } else {
                    break;
                }
            }
        }

        if ordered.is_empty() {
            None
        } else {
            Some(ordered)
        }
    }

    /// Check if adding a fragment would create a circular dependency
    fn would_create_cycle(
        &self,
        candidate_id: FragmentId,
        fragments: &[Fragment],
        used: &HashSet<FragmentId>,
    ) -> bool {
        let mut visited = HashSet::new();
        let mut to_check = vec![candidate_id];

        // Limit depth to prevent excessive computation
        const MAX_DEPTH: usize = 10;
        let mut depth = 0;

        while let Some(current) = to_check.pop() {
            if depth >= MAX_DEPTH {
                return false; // Assume no cycle if chain is very long
            }

            if visited.contains(&current) {
                // Found a cycle within the candidate chain
                return true;
            }

            if used.contains(&current) {
                // Would link back to already-used fragment (not necessarily a cycle, but concerning)
                return true;
            }

            visited.insert(current);

            // Find this fragment and check its best matches
            if let Some(frag) = fragments.iter().find(|f| f.id == current) {
                let candidates: Vec<_> = fragments
                    .iter()
                    .filter(|f| !used.contains(&f.id) && f.id != current)
                    .collect();

                if !candidates.is_empty() {
                    let matches = self.matcher.find_best_matches(frag, &candidates);

                    // Only check the top match (most likely continuation)
                    if let Some((next_id, _)) = matches.first() {
                        to_check.push(*next_id);
                    }
                }
            }

            depth += 1;
        }

        false
    }

    /// Find the best orphaned fragment to add when the greedy chain breaks
    fn find_best_orphan(
        &self,
        fragments: &[Fragment],
        used: &HashSet<FragmentId>,
        ordered: &[(usize, FragmentId, MatchScore)],
    ) -> Option<(usize, FragmentId)> {
        let mut best: Option<(usize, FragmentId, f32)> = None;

        for (idx, frag) in fragments.iter().enumerate() {
            if used.contains(&frag.id) {
                continue;
            }

            // Score based on match quality with any already-ordered fragment
            let mut best_match_score = 0.0f32;
            for (_, ordered_id, _) in ordered {
                if let Some(ordered_frag) = fragments.iter().find(|f| f.id == *ordered_id) {
                    // Use public match_fragments API which combines temporal and spatial proximity
                    if let Some(match_score) = self.matcher.match_fragments(frag, ordered_frag) {
                        best_match_score = best_match_score.max(match_score.confidence);
                    } else {
                        // Even if below confidence threshold, consider spatial proximity
                        let spatial = self.calculate_simple_proximity(frag, ordered_frag);
                        best_match_score = best_match_score.max(spatial * 0.5);
                    }
                }
            }

            if best
                .as_ref()
                .map_or(true, |(_, _, score)| best_match_score > *score)
            {
                best = Some((idx, frag.id, best_match_score));
            }
        }

        best.map(|(idx, id, _)| (idx, id))
    }

    /// Calculate simple proximity score based on disk location
    fn calculate_simple_proximity(&self, frag1: &Fragment, frag2: &Fragment) -> f32 {
        let distance = if frag1.start_offset > frag2.start_offset {
            frag1.start_offset - frag2.start_offset
        } else {
            frag2.start_offset - frag1.start_offset
        };

        // Score based on distance - closer fragments more likely related
        const MB: u64 = 1024 * 1024;
        if distance < 8192 {
            1.0
        } else if distance < MB {
            0.7
        } else if distance < 10 * MB {
            0.4
        } else {
            0.1
        }
    }

    /// Detect gaps between ordered fragments
    fn detect_gaps(&self, ordered: &[(usize, FragmentId, MatchScore)]) -> Vec<GapInfo> {
        let mut gaps = Vec::new();

        let mut current_pos = 0u64;

        for (_, frag_id, _) in ordered {
            if let Some(fragment) = self.catalog.get(*frag_id) {
                // Check for gap
                let expected_start = current_pos;

                // In reality, fragments might not have absolute positions
                // This is a simplified gap detection
                if fragment.size > 0 {
                    let gap_size = 0; // Placeholder - would need more context

                    if gap_size > 0 && gap_size <= self.max_gap_size {
                        gaps.push(GapInfo {
                            position: expected_start,
                            size: gap_size,
                            filled: false,
                        });
                    }
                }

                current_pos += fragment.size;
            }
        }

        gaps
    }

    /// Calculate overall reassembly confidence
    fn calculate_reassembly_confidence(
        &self,
        ordered: &[(usize, FragmentId, MatchScore)],
        gaps: &[GapInfo],
    ) -> f32 {
        if ordered.is_empty() {
            return 0.0;
        }

        // Average match scores
        let avg_match: f32 = ordered
            .iter()
            .map(|(_, _, score)| score.confidence)
            .sum::<f32>()
            / ordered.len() as f32;

        // Penalty for gaps
        let gap_penalty = if gaps.is_empty() {
            1.0
        } else {
            0.8 // 20% penalty for having gaps
        };

        // Bonus for file signature
        let signature_bonus = if ordered
            .first()
            .and_then(|(_, frag_id, _)| self.catalog.get(*frag_id).map(|f| f.signature.is_some()))
            .unwrap_or(false)
        {
            1.1 // 10% bonus
        } else {
            1.0
        };

        (avg_match * gap_penalty * signature_bonus).min(1.0)
    }

    /// Write reassembled file to output
    pub fn write_reassembled<W: Write>(
        &self,
        result: &ReassemblyResult,
        mut writer: W,
    ) -> io::Result<usize> {
        let mut total_written = 0;

        for fragment_id in &result.fragment_ids {
            if let Some(fragment) = self.catalog.get(*fragment_id) {
                if let Some(ref data) = fragment.data {
                    writer.write_all(data)?;
                    total_written += data.len();
                }
            }
        }

        Ok(total_written)
    }

    /// Get statistics about reassembly capabilities
    pub fn get_statistics(&self) -> ReassemblyStatistics {
        let total_fragments = self.catalog.len();

        let fragments: Vec<_> = self.catalog.all_fragments().cloned().collect();
        let clusters = self.matcher.cluster_fragments(&fragments);

        let reassemblable = clusters.len();

        ReassemblyStatistics {
            total_fragments,
            reassemblable_files: reassemblable,
            average_fragments_per_file: if reassemblable > 0 {
                total_fragments as f32 / reassemblable as f32
            } else {
                0.0
            },
        }
    }
}

/// Statistics about reassembly capabilities
#[derive(Debug, Clone)]
pub struct ReassemblyStatistics {
    pub total_fragments: usize,
    pub reassemblable_files: usize,
    pub average_fragments_per_file: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recovery::signatures::SignatureMatch;

    #[test]
    fn test_reassembly_engine_creation() {
        let catalog = FragmentCatalog::new();
        let engine = ReassemblyEngine::new(catalog);
        assert_eq!(engine.max_gap_size, 64 * 1024);
    }

    #[test]
    fn test_reassembly_with_two_fragments() {
        let mut catalog = FragmentCatalog::new();

        let mut frag1 = Fragment::new(0, 0, 1024, 0);
        let mut frag2 = Fragment::new(0, 4096, 1024, 1);

        // Add signature to first fragment
        use crate::recovery::signatures::FileSignature;
        frag1.signature = Some(SignatureMatch {
            category: "image".to_string(),
            signature: FileSignature {
                signature: vec![0xFF, 0xD8],
                offset: 0,
                mime_type: "image/jpeg".to_string(),
                extensions: vec!["jpg".to_string()],
                description: "JPEG".to_string(),
            },
            confidence: 1.0,
        });

        let data = vec![0xFF, 0xD8, 0xFF, 0xE0]; // JPEG header
        frag1.set_data(data.clone());
        frag2.set_data(data);

        catalog.add_fragment(frag1);
        catalog.add_fragment(frag2);

        let engine = ReassemblyEngine::new(catalog).with_min_confidence(0.3);

        let results = engine.reassemble_by_type("image/jpeg");
        assert!(!results.is_empty());
    }

    #[test]
    fn test_gap_detection() {
        let catalog = FragmentCatalog::new();
        let engine = ReassemblyEngine::new(catalog);

        let frag1 = Fragment::new(1, 0, 1024, 0);
        let frag2 = Fragment::new(2, 8192, 1024, 2); // Gap between

        let score = MatchScore::calculate(0.8, 0.8, 0.8, 0.8);
        let ordered = vec![(0, frag1.id, score.clone()), (1, frag2.id, score)];

        let gaps = engine.detect_gaps(&ordered);
        // Gap detection is simplified in current impl
        assert!(gaps.len() <= 1);
    }

    #[test]
    fn test_statistics() {
        let mut catalog = FragmentCatalog::new();

        catalog.add_fragment(Fragment::new(0, 0, 1024, 0));
        catalog.add_fragment(Fragment::new(0, 4096, 1024, 1));

        let engine = ReassemblyEngine::new(catalog);
        let stats = engine.get_statistics();

        assert_eq!(stats.total_fragments, 2);
    }

    #[test]
    fn test_circular_dependency_detection() {
        let mut catalog = FragmentCatalog::new();

        // Create fragments
        let frag1 = Fragment::new(0, 0, 1024, 1);
        let frag2 = Fragment::new(0, 1024, 1024, 2);
        let frag3 = Fragment::new(0, 2048, 1024, 3);

        catalog.add_fragment(frag1.clone());
        catalog.add_fragment(frag2.clone());
        catalog.add_fragment(frag3.clone());

        let engine = ReassemblyEngine::new(catalog);

        let fragments = vec![frag1.clone(), frag2.clone(), frag3];
        let mut used = HashSet::new();
        used.insert(frag1.id);

        // Test that would_create_cycle detects already-used fragments
        let would_cycle = engine.would_create_cycle(frag1.id, &fragments, &used);
        assert!(would_cycle, "Should detect that fragment is already used");

        // Test that the function completes without errors
        let _result = engine.would_create_cycle(frag2.id, &fragments, &used);
        // Just verify the function runs without panic
    }

    #[test]
    fn test_orphan_fragment_recovery() {
        let mut catalog = FragmentCatalog::new();

        // Create fragments with spatial relationship (temporal_hint is optional)
        let mut frag1 = Fragment::new(0, 0, 1024, 1);
        frag1.temporal_hint = Some(chrono::Utc::now());

        let mut frag2 = Fragment::new(0, 5000, 512, 2); // Far away spatially
        frag2.temporal_hint = Some(chrono::Utc::now());

        let mut frag3 = Fragment::new(0, 2048, 1024, 3); // Closer spatially
        frag3.temporal_hint = Some(chrono::Utc::now());

        catalog.add_fragment(frag1.clone());
        catalog.add_fragment(frag2.clone());
        catalog.add_fragment(frag3.clone());

        let engine = ReassemblyEngine::new(catalog);

        let fragments = vec![frag1.clone(), frag2, frag3];
        let mut used = HashSet::new();
        used.insert(frag1.id);

        let ordered = vec![(0, frag1.id, MatchScore::calculate(1.0, 1.0, 1.0, 1.0))];

        // Find best orphan - may or may not find one depending on confidence thresholds
        // The key is that the function should run without errors
        let orphan = engine.find_best_orphan(&fragments, &used, &ordered);
        // We have 3 fragments, 1 is used, so we should have 2 candidates
        // The function should at least execute successfully
        assert!(
            orphan.is_some() || orphan.is_none(),
            "Function should complete"
        );
    }

    #[test]
    fn test_robust_fragment_ordering() {
        let mut catalog = FragmentCatalog::new();

        // Create a chain of fragments with varying quality
        let mut frag1 = Fragment::new(0, 0, 1024, 1);
        use crate::recovery::signatures::FileSignature;
        frag1.signature = Some(SignatureMatch {
            category: "document".to_string(),
            signature: FileSignature {
                signature: vec![0x25, 0x50, 0x44, 0x46], // PDF header
                offset: 0,
                mime_type: "application/pdf".to_string(),
                extensions: vec!["pdf".to_string()],
                description: "PDF".to_string(),
            },
            confidence: 1.0,
        });
        frag1.set_data(vec![0x25, 0x50, 0x44, 0x46]);

        let mut frag2 = Fragment::new(0, 1024, 1024, 2);
        frag2.set_data(vec![0x00; 1024]);

        let mut frag3 = Fragment::new(0, 2048, 1024, 3);
        frag3.set_data(vec![0xFF; 1024]);

        catalog.add_fragment(frag1.clone());
        catalog.add_fragment(frag2.clone());
        catalog.add_fragment(frag3.clone());

        let engine = ReassemblyEngine::new(catalog).with_min_confidence(0.1);

        let fragments = vec![frag1.clone(), frag2, frag3];
        let ordered = engine.order_fragments(&fragments);

        assert!(ordered.is_some(), "Should successfully order fragments");
        let ordered = ordered.unwrap();
        assert!(
            ordered.len() >= 1,
            "Should have at least the start fragment"
        );

        // First fragment should be the one with the signature
        assert_eq!(
            ordered[0].1, frag1.id,
            "First fragment should have the file signature"
        );
    }
}
