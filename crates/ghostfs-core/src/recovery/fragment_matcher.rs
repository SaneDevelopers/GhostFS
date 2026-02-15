/// Content-based fragment matching for intelligent reassembly
///
/// This module provides algorithms to match fragments that likely belong
/// to the same file based on content patterns, byte entropy, and structural analysis.
use std::collections::HashMap;

use crate::recovery::fragments::{Fragment, FragmentId};

/// Result of matching two fragments
#[derive(Debug, Clone)]
pub struct MatchScore {
    /// Overall match confidence (0.0-1.0)
    pub confidence: f32,

    /// Content similarity score
    pub content_similarity: f32,

    /// Structural similarity score
    pub structural_similarity: f32,

    /// Temporal proximity score
    pub temporal_proximity: f32,

    /// Spatial proximity score (disk location)
    pub spatial_proximity: f32,
}

impl MatchScore {
    /// Calculate weighted match score
    pub fn calculate(
        content_sim: f32,
        structural_sim: f32,
        temporal_prox: f32,
        spatial_prox: f32,
    ) -> Self {
        // Weighted combination - content is most important
        let confidence =
            0.5 * content_sim + 0.2 * structural_sim + 0.15 * temporal_prox + 0.15 * spatial_prox;

        Self {
            confidence,
            content_similarity: content_sim,
            structural_similarity: structural_sim,
            temporal_proximity: temporal_prox,
            spatial_proximity: spatial_prox,
        }
    }
}

/// Fragment matching engine
pub struct FragmentMatcher {
    /// Minimum confidence threshold for matches
    min_confidence: f32,
}

impl FragmentMatcher {
    /// Create a new fragment matcher
    pub fn new() -> Self {
        Self {
            min_confidence: 0.4, // Default threshold
        }
    }

    /// Set minimum confidence threshold
    pub fn with_min_confidence(mut self, min_confidence: f32) -> Self {
        self.min_confidence = min_confidence;
        self
    }

    /// Match two fragments
    pub fn match_fragments(&self, frag1: &Fragment, frag2: &Fragment) -> Option<MatchScore> {
        // Calculate individual scores
        let content_sim = self.content_similarity(frag1, frag2);
        let structural_sim = self.structural_similarity(frag1, frag2);
        let temporal_prox = self.temporal_proximity(frag1, frag2);
        let spatial_prox = self.spatial_proximity(frag1, frag2);

        let score = MatchScore::calculate(content_sim, structural_sim, temporal_prox, spatial_prox);

        if score.confidence >= self.min_confidence {
            Some(score)
        } else {
            None
        }
    }

    /// Calculate content similarity between fragments
    fn content_similarity(&self, frag1: &Fragment, frag2: &Fragment) -> f32 {
        // Use pre-calculated hash similarity from Fragment
        frag1.similarity_to(frag2)
    }

    /// Calculate structural similarity
    fn structural_similarity(&self, frag1: &Fragment, frag2: &Fragment) -> f32 {
        let mut score = 0.0;

        // Same file signature type
        if let (Some(ref sig1), Some(ref sig2)) = (&frag1.signature, &frag2.signature) {
            if sig1.signature.mime_type == sig2.signature.mime_type {
                score += 0.5;
            }
        }

        // Similar size (within 20%)
        let size_ratio = if frag1.size > frag2.size {
            frag2.size as f32 / frag1.size as f32
        } else {
            frag1.size as f32 / frag2.size as f32
        };

        if size_ratio > 0.8 {
            score += 0.5;
        }

        score
    }

    /// Calculate temporal proximity
    fn temporal_proximity(&self, frag1: &Fragment, frag2: &Fragment) -> f32 {
        match (&frag1.temporal_hint, &frag2.temporal_hint) {
            (Some(t1), Some(t2)) => {
                let duration = if t1 > t2 {
                    t1.signed_duration_since(*t2)
                } else {
                    t2.signed_duration_since(*t1)
                };

                // Score decreases with time difference
                // Same hour = 1.0, same day = 0.7, same week = 0.4, else = 0.0
                let hours = duration.num_hours().abs();

                if hours < 1 {
                    1.0
                } else if hours < 24 {
                    0.7
                } else if hours < 168 {
                    0.4
                } else {
                    0.0
                }
            }
            _ => 0.5, // No temporal information, neutral score
        }
    }

    /// Calculate spatial proximity (disk location)
    pub fn spatial_proximity(&self, frag1: &Fragment, frag2: &Fragment) -> f32 {
        let distance = if frag1.start_offset > frag2.start_offset {
            frag1.start_offset - frag2.start_offset
        } else {
            frag2.start_offset - frag1.start_offset
        };

        // Score based on distance
        // Adjacent blocks = 1.0, within 1MB = 0.7, within 10MB = 0.4, else = 0.0
        const MB: u64 = 1024 * 1024;

        if distance < 8192 {
            // Within 8KB (adjacent)
            1.0
        } else if distance < MB {
            0.7
        } else if distance < 10 * MB {
            0.4
        } else {
            0.0
        }
    }

    /// Find best matches for a fragment from a collection
    pub fn find_best_matches(
        &self,
        target: &Fragment,
        candidates: &[&Fragment],
    ) -> Vec<(FragmentId, MatchScore)> {
        let mut matches = Vec::new();

        for candidate in candidates {
            if candidate.id == target.id {
                continue; // Skip self
            }

            if let Some(score) = self.match_fragments(target, candidate) {
                matches.push((candidate.id, score));
            }
        }

        // Sort by confidence descending
        matches.sort_by(|a, b| b.1.confidence.partial_cmp(&a.1.confidence).unwrap());

        matches
    }

    /// Group fragments into likely file clusters
    pub fn cluster_fragments(&self, fragments: &[Fragment]) -> Vec<Vec<FragmentId>> {
        let mut clusters: Vec<Vec<FragmentId>> = Vec::new();
        let mut assigned: HashMap<FragmentId, usize> = HashMap::new();

        for frag in fragments {
            if assigned.contains_key(&frag.id) {
                continue; // Already in a cluster
            }

            // Start new cluster
            let mut cluster = vec![frag.id];
            assigned.insert(frag.id, clusters.len());

            // Find related fragments
            let candidates: Vec<_> = fragments.iter().collect();
            let matches = self.find_best_matches(frag, &candidates);

            for (matched_id, score) in matches {
                if score.confidence > 0.6 && !assigned.contains_key(&matched_id) {
                    cluster.push(matched_id);
                    assigned.insert(matched_id, clusters.len());
                }
            }

            if !cluster.is_empty() {
                clusters.push(cluster);
            }
        }

        clusters
    }
}

impl Default for FragmentMatcher {
    fn default() -> Self {
        Self::new()
    }
}

/// Calculate byte entropy for a data block (measure of randomness)
pub fn calculate_entropy(data: &[u8]) -> f32 {
    if data.is_empty() {
        return 0.0;
    }

    let mut counts = [0u64; 256];
    for &byte in data {
        counts[byte as usize] += 1;
    }

    let len = data.len() as f64;
    let mut entropy = 0.0;

    for &count in &counts {
        if count > 0 {
            let p = count as f64 / len;
            entropy -= p * p.log2();
        }
    }

    entropy as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_match_score_calculation() {
        let score = MatchScore::calculate(0.8, 0.6, 0.9, 0.7);
        assert!(score.confidence > 0.7);
        assert_eq!(score.content_similarity, 0.8);
    }

    #[test]
    fn test_fragment_matcher_creation() {
        let matcher = FragmentMatcher::new();
        assert_eq!(matcher.min_confidence, 0.4);

        let matcher = FragmentMatcher::new().with_min_confidence(0.6);
        assert_eq!(matcher.min_confidence, 0.6);
    }

    #[test]
    fn test_spatial_proximity() {
        let matcher = FragmentMatcher::new();

        let frag1 = Fragment::new(1, 0, 1024, 0);
        let mut frag2 = Fragment::new(2, 4096, 1024, 1);

        let prox = matcher.spatial_proximity(&frag1, &frag2);
        assert!(prox > 0.6); // Within 8KB

        frag2.start_offset = 10 * 1024 * 1024; // 10MB away
        let prox = matcher.spatial_proximity(&frag1, &frag2);
        assert!(prox < 0.5);
    }

    #[test]
    fn test_temporal_proximity() {
        let matcher = FragmentMatcher::new();

        let now = Utc::now();
        let mut frag1 = Fragment::new(1, 0, 1024, 0);
        let mut frag2 = Fragment::new(2, 4096, 1024, 1);

        frag1.temporal_hint = Some(now);
        frag2.temporal_hint = Some(now);

        let prox = matcher.temporal_proximity(&frag1, &frag2);
        assert_eq!(prox, 1.0); // Same time
    }

    #[test]
    fn test_find_best_matches() {
        let matcher = FragmentMatcher::new();

        let mut frag1 = Fragment::new(1, 0, 1024, 0);
        let mut frag2 = Fragment::new(2, 4096, 1024, 1);
        let mut frag3 = Fragment::new(3, 8192, 2048, 2);

        let data = vec![1, 2, 3, 4, 5];
        frag1.set_data(data.clone());
        frag2.set_data(data);
        frag3.set_data(vec![10, 20, 30]);

        let candidates = vec![&frag2, &frag3];
        let matches = matcher.find_best_matches(&frag1, &candidates);

        assert!(!matches.is_empty());
        assert_eq!(matches[0].0, 2); // frag2 should match best
    }

    #[test]
    fn test_entropy_calculation() {
        // Uniform data (low entropy)
        let uniform = vec![0u8; 100];
        let entropy = calculate_entropy(&uniform);
        assert_eq!(entropy, 0.0);

        // Random-ish data (higher entropy)
        let varied: Vec<u8> = (0..256).map(|i| i as u8).collect();
        let entropy = calculate_entropy(&varied);
        assert!(entropy > 5.0);
    }

    #[test]
    fn test_cluster_fragments() {
        let matcher = FragmentMatcher::new();

        let mut frag1 = Fragment::new(1, 0, 1024, 0);
        let mut frag2 = Fragment::new(2, 4096, 1024, 1);
        let mut frag3 = Fragment::new(3, 100000, 1024, 100);

        let data1 = vec![1, 2, 3, 4, 5];
        let data2 = vec![5, 4, 3, 2, 1];

        frag1.set_data(data1.clone());
        frag2.set_data(data1);
        frag3.set_data(data2);

        let fragments = vec![frag1, frag2, frag3];
        let clusters = matcher.cluster_fragments(&fragments);

        // Should create at least one cluster
        assert!(!clusters.is_empty());
    }
}
