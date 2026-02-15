/// Fragment detection and catalog management for advanced file recovery
///
/// This module handles detecting, storing, and organizing file fragments
/// for later reassembly into complete files.

use chrono::{DateTime, Utc};
use std::collections::{BTreeMap, HashMap};

use crate::recovery::signatures::SignatureMatch;

/// Unique identifier for a fragment
pub type FragmentId = u64;

/// Represents a detected file fragment
#[derive(Debug, Clone)]
pub struct Fragment {
    /// Unique fragment identifier
    pub id: FragmentId,
    
    /// Starting byte offset on the device
    pub start_offset: u64,
    
    /// Size of the fragment in bytes
    pub size: u64,
    
    /// Detected file signature (if any at fragment start)
    pub signature: Option<SignatureMatch>,
    
    /// Fast content hash for similarity matching
    pub content_hash: u64,
    
    /// Suspected parent file inode/cluster (if detectable)
    pub parent_file_hint: Option<u64>,
    
    /// Temporal hint from nearby metadata
    pub temporal_hint: Option<DateTime<Utc>>,
    
    /// Confidence that this is a valid fragment (0.0-1.0)
    pub confidence: f32,
    
    /// Filesystem block number
    pub block_number: u64,
    
    /// Fragment data (optional, can be loaded on demand)
    pub data: Option<Vec<u8>>,
}

impl Fragment {
    /// Create a new fragment
    pub fn new(
        id: FragmentId,
        start_offset: u64,
        size: u64,
        block_number: u64,
    ) -> Self {
        Self {
            id,
            start_offset,
            size,
            signature: None,
            content_hash: 0,
            parent_file_hint: None,
            temporal_hint: None,
            confidence: 0.5, // Default medium confidence
            block_number,
            data: None,
        }
    }
    
    /// Calculate a simple content hash from fragment data
    pub fn calculate_content_hash(data: &[u8]) -> u64 {
        // Simple FNV-1a hash for fast similarity matching
        const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
        const FNV_PRIME: u64 = 0x100000001b3;
        
        let mut hash = FNV_OFFSET_BASIS;
        
        // Hash first 1KB for performance (representative sample)
        let sample_size = std::cmp::min(data.len(), 1024);
        for &byte in &data[..sample_size] {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        
        hash
    }
    
    /// Update fragment with data and calculate hash
    pub fn set_data(&mut self, data: Vec<u8>) {
        self.content_hash = Self::calculate_content_hash(&data);
        self.data = Some(data);
    }
    
    /// Get similarity score with another fragment (0.0-1.0)
    pub fn similarity_to(&self, other: &Fragment) -> f32 {
        // Compare content hashes
        let hash_similarity = if self.content_hash == other.content_hash {
            1.0
        } else {
            // Count matching bits in hash
            let xor = self.content_hash ^ other.content_hash;
            let matching_bits = 64 - xor.count_ones();
            matching_bits as f32 / 64.0
        };
        
        // Consider size similarity
        let size_ratio = if self.size > other.size {
            other.size as f32 / self.size as f32
        } else {
            self.size as f32 / other.size as f32
        };
        
        // Weighted combination
        0.7 * hash_similarity + 0.3 * size_ratio
    }
}

/// Catalog of all detected fragments
#[derive(Debug)]
pub struct FragmentCatalog {
    /// All fragments indexed by ID
    fragments: HashMap<FragmentId, Fragment>,
    
    /// Fragments organized by file signature type
    by_signature: HashMap<String, Vec<FragmentId>>,
    
    /// Fragments organized by size (for quick range queries)
    by_size: BTreeMap<u64, Vec<FragmentId>>,
    
    /// Fragments organized by disk location
    by_location: BTreeMap<u64, FragmentId>,
    
    /// Next fragment ID to assign
    next_id: FragmentId,
}

impl FragmentCatalog {
    /// Create a new empty fragment catalog
    pub fn new() -> Self {
        Self {
            fragments: HashMap::new(),
            by_signature: HashMap::new(),
            by_size: BTreeMap::new(),
            by_location: BTreeMap::new(),
            next_id: 1,
        }
    }
    
    /// Add a fragment to the catalog
    pub fn add_fragment(&mut self, mut fragment: Fragment) -> FragmentId {
        let id = self.next_id;
        self.next_id += 1;
        
        fragment.id = id;
        
        // Index by signature if present
        if let Some(ref sig) = fragment.signature {
            self.by_signature
                .entry(sig.signature.mime_type.clone())
                .or_insert_with(Vec::new)
                .push(id);
        }
        
        // Index by size
        self.by_size
            .entry(fragment.size)
            .or_insert_with(Vec::new)
            .push(id);
        
        // Index by location
        self.by_location.insert(fragment.start_offset, id);
        
        // Store fragment
        self.fragments.insert(id, fragment);
        
        id
    }
    
    /// Get a fragment by ID
    pub fn get(&self, id: FragmentId) -> Option<&Fragment> {
        self.fragments.get(&id)
    }
    
    /// Get all fragments
    pub fn all_fragments(&self) -> impl Iterator<Item = &Fragment> {
        self.fragments.values()
    }
    
    /// Get fragments by signature type
    pub fn fragments_by_signature(&self, mime_type: &str) -> Vec<Fragment> {
        self.by_signature
            .get(mime_type)
            .map(|ids| ids.iter().filter_map(|id| self.fragments.get(id).cloned()).collect())
            .unwrap_or_else(Vec::new)
    }
    
    /// Get fragments within a size range
    pub fn fragments_by_size_range(&self, min_size: u64, max_size: u64) -> Vec<&Fragment> {
        self.by_size
            .range(min_size..=max_size)
            .flat_map(|(_, ids)| ids.iter())
            .filter_map(|id| self.fragments.get(id))
            .collect()
    }
    
    /// Get fragments near a disk location (within range)
    pub fn fragments_near_location(&self, offset: u64, range: u64) -> Vec<&Fragment> {
        let start = offset.saturating_sub(range);
        let end = offset.saturating_add(range);
        
        self.by_location
            .range(start..=end)
            .filter_map(|(_, id)| self.fragments.get(id))
            .collect()
    }
    
    /// Find fragments that could belong to the same file
    pub fn find_related_fragments(&self, fragment_id: FragmentId) -> Vec<(FragmentId, f32)> {
        let Some(fragment) = self.fragments.get(&fragment_id) else {
            return Vec::new();
        };
        
        let mut candidates = Vec::new();
        
        // Check fragments with same signature type
        if let Some(ref sig) = fragment.signature {
            for other_id in self.by_signature
                .get(&sig.signature.mime_type)
                .unwrap_or(&Vec::new())
            {
                if *other_id == fragment_id {
                    continue;
                }
                
                if let Some(other) = self.fragments.get(other_id) {
                    let similarity = fragment.similarity_to(other);
                    if similarity > 0.3 {
                        // Minimum similarity threshold
                        candidates.push((*other_id, similarity));
                    }
                }
            }
        }
        
        // Sort by similarity (descending)
        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        
        candidates
    }
    
    /// Get total number of fragments
    pub fn len(&self) -> usize {
        self.fragments.len()
    }
    
    /// Check if catalog is empty
    pub fn is_empty(&self) -> bool {
        self.fragments.is_empty()
    }
    
    /// Remove a fragment from the catalog
    pub fn remove(&mut self, id: FragmentId) -> Option<Fragment> {
        if let Some(fragment) = self.fragments.remove(&id) {
            // Clean up indices
            if let Some(ref sig) = fragment.signature {
                if let Some(ids) = self.by_signature.get_mut(&sig.signature.mime_type) {
                    ids.retain(|&x| x != id);
                }
            }
            
            if let Some(ids) = self.by_size.get_mut(&fragment.size) {
                ids.retain(|&x| x != id);
            }
            
            self.by_location.remove(&fragment.start_offset);
            
            Some(fragment)
        } else {
            None
        }
    }
}

impl Default for FragmentCatalog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_fragment_creation() {
        let fragment = Fragment::new(1, 1024, 4096, 1);
        assert_eq!(fragment.id, 1);
        assert_eq!(fragment.start_offset, 1024);
        assert_eq!(fragment.size, 4096);
        assert_eq!(fragment.confidence, 0.5);
    }
    
    #[test]
    fn test_content_hash() {
        let data1 = vec![1, 2, 3, 4, 5];
        let data2 = vec![1, 2, 3, 4, 5];
        let data3 = vec![5, 4, 3, 2, 1];
        
        let hash1 = Fragment::calculate_content_hash(&data1);
        let hash2 = Fragment::calculate_content_hash(&data2);
        let hash3 = Fragment::calculate_content_hash(&data3);
        
        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
    }
    
    #[test]
    fn test_catalog_operations() {
        let mut catalog = FragmentCatalog::new();
        
        let frag1 = Fragment::new(0, 0, 1024, 0);
        let frag2 = Fragment::new(0, 4096, 2048, 1);
        
        let id1 = catalog.add_fragment(frag1);
        let id2 = catalog.add_fragment(frag2);
        
        assert_eq!(catalog.len(), 2);
        assert!(catalog.get(id1).is_some());
        assert!(catalog.get(id2).is_some());
    }
    
    #[test]
    fn test_fragment_similarity() {
        let mut frag1 = Fragment::new(1, 0, 1024, 0);
        let mut frag2 = Fragment::new(2, 4096, 1024, 1);
        
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8];
        frag1.set_data(data.clone());
        frag2.set_data(data);
        
        let similarity = frag1.similarity_to(&frag2);
        assert!(similarity > 0.9); // High similarity for identical data
    }
    
    #[test]
    fn test_size_range_query() {
        let mut catalog = FragmentCatalog::new();
        
        catalog.add_fragment(Fragment::new(0, 0, 1024, 0));
        catalog.add_fragment(Fragment::new(0, 4096, 2048, 1));
        catalog.add_fragment(Fragment::new(0, 8192, 4096, 2));
        
        let results = catalog.fragments_by_size_range(1000, 2500);
        assert_eq!(results.len(), 2);
    }
}
