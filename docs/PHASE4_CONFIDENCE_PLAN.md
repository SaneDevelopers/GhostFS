# Phase 4: Filesystem-Specific Confidence Scoring - Detailed Plan

**Goal**: Implement robust, filesystem-specific confidence scoring to accurately assess file recoverability

**Date**: February 2, 2026

---

## 📊 Current State Analysis

### Existing Confidence Framework (GOOD ✅)
- **Base algorithm**: Weighted factor system (working well)
- **Weight distribution**:
  - Time recency: 25%
  - Metadata completeness: 15%
  - Data block integrity: 20%
  - File signature match: 15%
  - Size consistency: 10%
  - **FS-specific: 15%** ← Currently placeholder (0.5 for all)

### Current Limitations (NEEDS WORK ❌)
1. All filesystems get same FS-specific score (0.5)
2. No validation of filesystem-specific structures
3. Missing filesystem metadata in confidence calculations
4. Can't distinguish between "truly deleted" vs "corrupted metadata"

---

## 🎯 Phase 4 Goals

### Primary Objectives
1. **XFS Confidence Scoring**: Leverage AG structure, inode validity, extent alignment
2. **Btrfs Confidence Scoring**: Use generation counters, checksums, COW tracking
3. **exFAT Confidence Scoring**: Validate FAT chains, cluster bounds, UTF-16 encoding

### Success Criteria
- ✅ Confidence scores reflect true recoverability (not arbitrary)
- ✅ Each filesystem uses its unique structures for validation
- ✅ Clear distinction between high/medium/low confidence files
- ✅ Test data shows meaningful score differences (not all 0.7)

---

## 🏗️ Architecture Design

### Extended Data Structures

We need filesystem-specific metadata to pass into confidence calculations:

```rust
// New: Filesystem-specific metadata for confidence scoring
#[derive(Debug, Clone)]
pub enum FsSpecificMetadata {
    Xfs(XfsFileMetadata),
    Btrfs(BtrfsFileMetadata),
    ExFat(ExFatFileMetadata),
}

#[derive(Debug, Clone)]
pub struct XfsFileMetadata {
    pub ag_number: u32,           // Which AG contains the inode
    pub ag_inode_number: u32,     // Inode number within AG
    pub extent_count: u32,        // Number of data extents
    pub extent_format: ExtentFormat, // Inline, extent list, or btree
    pub is_aligned: bool,         // Are extents properly aligned?
    pub last_link_count: u32,     // Link count before deletion
    pub inode_generation: u32,    // XFS generation counter
}

#[derive(Debug, Clone, Copy)]
pub enum ExtentFormat {
    Local,      // Data stored in inode (small files)
    Extents,    // Direct extent list
    Btree,      // B+tree format (large files)
}

#[derive(Debug, Clone)]
pub struct BtrfsFileMetadata {
    pub generation: u64,          // Btrfs generation number
    pub transid: u64,             // Transaction ID
    pub checksum_valid: bool,     // Checksum verification result
    pub in_snapshot: bool,        // File exists in a snapshot
    pub cow_extent_count: u32,    // Number of COW extents
    pub extent_refs: Vec<u64>,    // Extent reference counts
    pub tree_level: u8,           // Level in B-tree (0 = leaf)
}

#[derive(Debug, Clone)]
pub struct ExFatFileMetadata {
    pub first_cluster: u32,       // Starting cluster
    pub cluster_chain: Vec<u32>,  // Full FAT chain
    pub chain_valid: bool,        // FAT chain integrity
    pub utf16_valid: bool,        // Filename UTF-16 validity
    pub entry_count: u8,          // Number of directory entries
    pub checksum: u16,            // Entry set checksum
    pub attributes: u16,          // File attributes
}
```

### Updated DeletedFile Structure

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeletedFile {
    // ... existing fields ...
    
    // NEW: Filesystem-specific metadata for confidence scoring
    #[serde(skip)] // Don't serialize (too complex)
    pub fs_metadata: Option<FsSpecificMetadata>,
}
```

---

## 📐 Confidence Scoring Algorithms

### XFS Confidence Factors (15% weight, broken into sub-factors)

#### Factor 1: AG Structure Validity (5% of total)
```
Score calculation:
- Inode number within valid AG range: +2 points
- AG boundary respected by extents: +2 points
- Inode number sequential/reasonable: +1 point
Total: 0-5 points → normalized to 0.0-1.0
```

**Logic**:
```rust
fn calculate_xfs_ag_validity(meta: &XfsFileMetadata, sb: &XfsSuperblock) -> f32 {
    let mut score = 0.0;
    
    // 1. Inode in valid AG range
    if meta.ag_number < sb.ag_count {
        score += 2.0;
    }
    
    // 2. AG inode number reasonable
    let max_inodes_per_ag = (sb.ag_blocks * sb.inodes_per_block as u32) as u64;
    if (meta.ag_inode_number as u64) < max_inodes_per_ag {
        score += 2.0;
    }
    
    // 3. Generation counter reasonable (not corrupted)
    if meta.inode_generation > 0 && meta.inode_generation < 1_000_000 {
        score += 1.0;
    }
    
    score / 5.0 // Normalize to 0-1
}
```

#### Factor 2: Extent Integrity (5% of total)
```
Score calculation:
- All extents within filesystem bounds: +2 points
- Extents properly aligned: +2 points
- No overlapping extents: +1 point
Total: 0-5 points → normalized to 0.0-1.0
```

**Logic**:
```rust
fn calculate_xfs_extent_integrity(
    file: &DeletedFile,
    meta: &XfsFileMetadata,
    sb: &XfsSuperblock
) -> f32 {
    let mut score = 0.0;
    
    // 1. All extents within bounds
    let total_blocks = sb.data_blocks;
    let all_in_bounds = file.data_blocks.iter()
        .all(|b| b.start_block < total_blocks);
    if all_in_bounds {
        score += 2.0;
    }
    
    // 2. Extent alignment (should align to stripe width if using RAID)
    if meta.is_aligned {
        score += 2.0;
    }
    
    // 3. No overlaps (check each extent against others)
    let has_overlaps = check_extent_overlaps(&file.data_blocks);
    if !has_overlaps {
        score += 1.0;
    }
    
    score / 5.0
}
```

#### Factor 3: Inode State Consistency (5% of total)
```
Score calculation:
- Link count was > 0 before deletion: +2 points
- Extent format matches file size: +2 points
- Reasonable extent count: +1 point
Total: 0-5 points → normalized to 0.0-1.0
```

**Logic**:
```rust
fn calculate_xfs_inode_consistency(
    file: &DeletedFile,
    meta: &XfsFileMetadata
) -> f32 {
    let mut score = 0.0;
    
    // 1. Had links (wasn't already corrupted)
    if meta.last_link_count > 0 {
        score += 2.0;
    }
    
    // 2. Format matches size
    let format_ok = match meta.extent_format {
        ExtentFormat::Local => file.size < 100,  // Small files
        ExtentFormat::Extents => meta.extent_count <= 10,  // Reasonable
        ExtentFormat::Btree => meta.extent_count > 10,  // Large files
    };
    if format_ok {
        score += 2.0;
    }
    
    // 3. Extent count reasonable for file size
    let avg_extent_size = if meta.extent_count > 0 {
        file.size / meta.extent_count as u64
    } else {
        0
    };
    // Typical extent is 1-100 blocks (4KB-400KB with 4KB blocks)
    if avg_extent_size > 4096 && avg_extent_size < 409600 {
        score += 1.0;
    }
    
    score / 5.0
}
```

**Final XFS Score**:
```rust
fn calculate_xfs_specific_factor(
    file: &DeletedFile,
    meta: &XfsFileMetadata,
    sb: &XfsSuperblock
) -> f32 {
    let ag_score = calculate_xfs_ag_validity(meta, sb);
    let extent_score = calculate_xfs_extent_integrity(file, meta, sb);
    let inode_score = calculate_xfs_inode_consistency(file, meta);
    
    // Average of three factors
    (ag_score + extent_score + inode_score) / 3.0
}
```

---

### Btrfs Confidence Factors (15% weight)

#### Factor 1: Generation Counter Validity (6% of total)
```
Score calculation:
- Generation <= current FS generation: +3 points
- Generation > 0: +2 points
- Transaction ID consistent: +1 point
Total: 0-6 points → normalized to 0.0-1.0
```

**Logic**:
```rust
fn calculate_btrfs_generation_validity(
    meta: &BtrfsFileMetadata,
    current_gen: u64
) -> f32 {
    let mut score = 0.0;
    
    // 1. Generation is reasonable
    if meta.generation <= current_gen {
        score += 3.0;
    }
    
    // 2. Not corrupted (non-zero)
    if meta.generation > 0 {
        score += 2.0;
    }
    
    // 3. Transaction ID makes sense
    if meta.transid > 0 && meta.transid <= current_gen {
        score += 1.0;
    }
    
    score / 6.0
}
```

#### Factor 2: Checksum Validation (6% of total)
```
Score calculation:
- Checksum valid: +6 points (critical for Btrfs)
- Checksum invalid: 0 points (major red flag)
```

**Logic**:
```rust
fn calculate_btrfs_checksum_score(meta: &BtrfsFileMetadata) -> f32 {
    if meta.checksum_valid {
        1.0  // Perfect score
    } else {
        0.0  // Major issue
    }
}
```

#### Factor 3: COW Structure Integrity (3% of total)
```
Score calculation:
- Extent reference counts valid: +2 points
- File in snapshot (easier recovery): +1 point
Total: 0-3 points → normalized to 0.0-1.0
```

**Logic**:
```rust
fn calculate_btrfs_cow_integrity(meta: &BtrfsFileMetadata) -> f32 {
    let mut score = 0.0;
    
    // 1. Extent refs are non-zero and reasonable
    if meta.extent_refs.iter().all(|&r| r > 0 && r < 1000) {
        score += 2.0;
    }
    
    // 2. In snapshot (better chance of recovery)
    if meta.in_snapshot {
        score += 1.0;
    }
    
    score / 3.0
}
```

**Final Btrfs Score**:
```rust
fn calculate_btrfs_specific_factor(
    meta: &BtrfsFileMetadata,
    current_generation: u64
) -> f32 {
    let gen_score = calculate_btrfs_generation_validity(meta, current_generation);
    let checksum_score = calculate_btrfs_checksum_score(meta);
    let cow_score = calculate_btrfs_cow_integrity(meta);
    
    // Weighted: checksum is most important
    (gen_score * 0.4 + checksum_score * 0.4 + cow_score * 0.2)
}
```

---

### exFAT Confidence Factors (15% weight)

#### Factor 1: FAT Chain Validity (7% of total)
```
Score calculation:
- Chain starts at valid cluster: +2 points
- All clusters in valid range: +3 points
- Chain ends properly (EOF marker): +2 points
Total: 0-7 points → normalized to 0.0-1.0
```

**Logic**:
```rust
fn calculate_exfat_chain_validity(
    meta: &ExFatFileMetadata,
    cluster_count: u32
) -> f32 {
    let mut score = 0.0;
    
    // 1. First cluster valid (>= 2, clusters 0-1 are reserved)
    if meta.first_cluster >= 2 {
        score += 2.0;
    }
    
    // 2. All clusters in bounds
    if meta.cluster_chain.iter().all(|&c| c >= 2 && c < cluster_count) {
        score += 3.0;
    }
    
    // 3. Chain integrity flag
    if meta.chain_valid {
        score += 2.0;
    }
    
    score / 7.0
}
```

#### Factor 2: Directory Entry Consistency (5% of total)
```
Score calculation:
- Checksum valid: +3 points
- Entry count reasonable (1-18): +1 point
- UTF-16 filename valid: +1 point
Total: 0-5 points → normalized to 0.0-1.0
```

**Logic**:
```rust
fn calculate_exfat_entry_consistency(meta: &ExFatFileMetadata) -> f32 {
    let mut score = 0.0;
    
    // 1. Checksum matches
    // (Checksum is calculated over all directory entries in the set)
    // We'll assume this is pre-validated in ExFatFileMetadata
    if meta.checksum != 0 {  // Non-zero means it was verified
        score += 3.0;
    }
    
    // 2. Entry count is reasonable
    // exFAT: 1 File Entry + 1 Stream Extension + 1-17 File Name entries
    if meta.entry_count >= 2 && meta.entry_count <= 18 {
        score += 1.0;
    }
    
    // 3. UTF-16 name is valid
    if meta.utf16_valid {
        score += 1.0;
    }
    
    score / 5.0
}
```

#### Factor 3: Cluster Usage Patterns (3% of total)
```
Score calculation:
- Clusters not marked as bad: +2 points
- Cluster chain length matches file size: +1 point
Total: 0-3 points → normalized to 0.0-1.0
```

**Logic**:
```rust
fn calculate_exfat_cluster_patterns(
    file: &DeletedFile,
    meta: &ExFatFileMetadata,
    cluster_size: u64
) -> f32 {
    let mut score = 0.0;
    
    // 1. No bad cluster markers (0xFFFFFFF7)
    if !meta.cluster_chain.contains(&0xFFFFFFF7) {
        score += 2.0;
    }
    
    // 2. Chain length matches file size
    let expected_clusters = (file.size + cluster_size - 1) / cluster_size;
    let actual_clusters = meta.cluster_chain.len() as u64;
    
    let size_ratio = if expected_clusters > 0 {
        (actual_clusters as f32) / (expected_clusters as f32)
    } else {
        0.0
    };
    
    // Within 10% is good
    if size_ratio >= 0.9 && size_ratio <= 1.1 {
        score += 1.0;
    }
    
    score / 3.0
}
```

**Final exFAT Score**:
```rust
fn calculate_exfat_specific_factor(
    file: &DeletedFile,
    meta: &ExFatFileMetadata,
    cluster_count: u32,
    cluster_size: u64
) -> f32 {
    let chain_score = calculate_exfat_chain_validity(meta, cluster_count);
    let entry_score = calculate_exfat_entry_consistency(meta);
    let pattern_score = calculate_exfat_cluster_patterns(file, meta, cluster_size);
    
    // Weighted: chain validity is most important
    (chain_score * 0.5 + entry_score * 0.3 + pattern_score * 0.2)
}
```

---

## 🔄 Implementation Strategy

### Phase 4.1: Data Structure Updates (Day 1)
**Files to modify**:
1. `crates/ghostfs-core/src/lib.rs`
   - Add `FsSpecificMetadata` enum and sub-structs
   - Add `fs_metadata` field to `DeletedFile`

2. `crates/ghostfs-core/src/recovery/confidence.rs`
   - Update function signatures to accept filesystem metadata
   - Add helper functions for sub-factor calculations

### Phase 4.2: XFS Implementation (Day 2)
**Files to modify**:
1. `crates/ghostfs-core/src/fs/xfs/mod.rs`
   - Extract `XfsFileMetadata` during inode parsing
   - Populate `fs_metadata` field in `DeletedFile` structs

2. `crates/ghostfs-core/src/recovery/confidence.rs`
   - Implement `calculate_xfs_specific_factor` with all sub-factors
   - Add tests for XFS scoring

### Phase 4.3: Btrfs Implementation (Day 3)
**Files to modify**:
1. `crates/ghostfs-core/src/fs/btrfs/recovery.rs`
   - Extract `BtrfsFileMetadata` during file reconstruction
   - Validate checksums
   - Populate `fs_metadata`

2. `crates/ghostfs-core/src/recovery/confidence.rs`
   - Implement `calculate_btrfs_specific_factor`
   - Add tests for Btrfs scoring

### Phase 4.4: exFAT Implementation (Day 4)
**Files to modify**:
1. `crates/ghostfs-core/src/fs/exfat/recovery.rs`
   - Extract `ExFatFileMetadata` during file reconstruction
   - Validate FAT chains and UTF-16 names
   - Populate `fs_metadata`

2. `crates/ghostfs-core/src/recovery/confidence.rs`
   - Implement `calculate_exfat_specific_factor`
   - Add tests for exFAT scoring

### Phase 4.5: Testing & Validation (Day 5)
- Create test cases with known good/bad files
- Verify score distribution (should see 0.3-0.9 range, not all 0.7)
- Test edge cases (corrupted metadata, partial chains, etc.)
- Integration tests with real test images

---

## 📊 Expected Score Distributions

After implementation, we should see:

### XFS Files
- **High confidence (0.8-1.0)**: Recently deleted, all extents valid, good AG structure
- **Medium confidence (0.5-0.8)**: Some extents missing, metadata incomplete
- **Low confidence (0.0-0.5)**: Corrupted inode, extents out of bounds, bad AG

### Btrfs Files
- **High confidence (0.8-1.0)**: Valid checksums, in snapshot, good generation
- **Medium confidence (0.5-0.8)**: Checksum failed but structure intact
- **Low confidence (0.0-0.5)**: Corrupted generation, no checksum, bad COW refs

### exFAT Files
- **High confidence (0.8-1.0)**: Valid FAT chain, good UTF-16, proper checksums
- **Medium confidence (0.5-0.8)**: Partial chain, some metadata missing
- **Low confidence (0.0-0.5)**: Broken chain, bad clusters, corrupted entries

---

## ✅ Success Metrics

1. **Differentiation**: Confidence scores vary meaningfully (not all ~0.7)
2. **Accuracy**: High-confidence files recover successfully (>90% success rate)
3. **Precision**: Low-confidence files have real issues (not false negatives)
4. **Performance**: Confidence calculation adds <10% to scan time

---

## 🧪 Test Plan

### Unit Tests (per filesystem)
```rust
#[test]
fn test_xfs_confidence_high() {
    // Perfect file: all structures valid
    assert!(score > 0.8);
}

#[test]
fn test_xfs_confidence_medium() {
    // Some issues: missing extents
    assert!(score > 0.5 && score < 0.8);
}

#[test]
fn test_xfs_confidence_low() {
    // Major issues: corrupted inode
    assert!(score < 0.5);
}
```

### Integration Tests
- Test with real XFS/Btrfs/exFAT images
- Verify recovered files match confidence scores
- Test edge cases (empty files, huge files, corrupted metadata)

---

## 📝 Documentation Updates

1. Update README with confidence scoring explanation
2. Add docs/CONFIDENCE_SCORING.md with detailed algorithm
3. Update CLI help text to explain score meanings
4. Add examples showing score interpretation

---

## 🚀 Ready to Execute?

This plan provides:
- ✅ Clear data structures
- ✅ Detailed algorithms for each filesystem
- ✅ Step-by-step implementation order
- ✅ Concrete test criteria
- ✅ Expected outcomes

**Estimated time**: 5 days (as planned in IMPLEMENTATION_PLAN.md)

Shall we proceed with implementation? I recommend starting with Phase 4.1 (data structures) today!
