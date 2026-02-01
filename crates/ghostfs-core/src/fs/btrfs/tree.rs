/// Btrfs B-tree structures and traversal
/// 
/// Btrfs uses copy-on-write B-trees for all metadata storage.
/// Trees are identified by their root and have nodes at multiple levels.

use anyhow::{Result, bail};
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Cursor;

use super::BlockDevice;

// ============================================================================
// Constants
// ============================================================================

/// Btrfs item types
pub const BTRFS_INODE_ITEM_KEY: u8 = 1;
pub const BTRFS_INODE_REF_KEY: u8 = 12;
pub const BTRFS_INODE_EXTREF_KEY: u8 = 13;
pub const BTRFS_DIR_ITEM_KEY: u8 = 84;
pub const BTRFS_DIR_INDEX_KEY: u8 = 96;
pub const BTRFS_EXTENT_DATA_KEY: u8 = 108;
pub const BTRFS_ORPHAN_ITEM_KEY: u8 = 48;
pub const BTRFS_ROOT_ITEM_KEY: u8 = 132;
pub const BTRFS_ROOT_REF_KEY: u8 = 156;
pub const BTRFS_CHUNK_ITEM_KEY: u8 = 228;

/// Well-known object IDs
pub const BTRFS_ROOT_TREE_OBJECTID: u64 = 1;
pub const BTRFS_EXTENT_TREE_OBJECTID: u64 = 2;
pub const BTRFS_CHUNK_TREE_OBJECTID: u64 = 3;
pub const BTRFS_DEV_TREE_OBJECTID: u64 = 4;
pub const BTRFS_FS_TREE_OBJECTID: u64 = 5;
pub const BTRFS_ORPHAN_OBJECTID: u64 = u64::MAX - 5; // -6
pub const BTRFS_FIRST_FREE_OBJECTID: u64 = 256;

// ============================================================================
// Structures
// ============================================================================

/// Btrfs tree key - identifies an item in the tree
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct BtrfsKey {
    pub objectid: u64,
    pub item_type: u8,
    pub offset: u64,
}

impl BtrfsKey {
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < 17 {
            bail!("Insufficient data for BtrfsKey");
        }
        let mut cursor = Cursor::new(data);
        Ok(Self {
            objectid: cursor.read_u64::<LittleEndian>()?,
            item_type: cursor.read_u8()?,
            offset: cursor.read_u64::<LittleEndian>()?,
        })
    }
    
    /// Size of a key in bytes
    pub const SIZE: usize = 17;
}

/// Btrfs node header - present at the start of every tree node
#[derive(Debug, Clone)]
pub struct BtrfsHeader {
    pub checksum: [u8; 32],
    pub fsid: [u8; 16],
    pub bytenr: u64,      // Logical address of this node
    pub flags: u64,
    pub chunk_tree_uuid: [u8; 16],
    pub generation: u64,
    pub owner: u64,       // Tree ID that owns this node
    pub nritems: u32,     // Number of items in this node
    pub level: u8,        // 0 for leaf, >0 for internal nodes
}

impl BtrfsHeader {
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < 101 {
            bail!("Insufficient data for BtrfsHeader");
        }
        
        let mut checksum = [0u8; 32];
        checksum.copy_from_slice(&data[0..32]);
        
        let mut fsid = [0u8; 16];
        fsid.copy_from_slice(&data[32..48]);
        
        let mut cursor = Cursor::new(&data[48..]);
        let bytenr = cursor.read_u64::<LittleEndian>()?;
        let flags = cursor.read_u64::<LittleEndian>()?;
        
        let mut chunk_tree_uuid = [0u8; 16];
        chunk_tree_uuid.copy_from_slice(&data[72..88]);
        
        let mut cursor = Cursor::new(&data[88..]);
        let generation = cursor.read_u64::<LittleEndian>()?;
        let owner = cursor.read_u64::<LittleEndian>()?;
        let nritems = cursor.read_u32::<LittleEndian>()?;
        let level = cursor.read_u8()?;
        
        Ok(Self {
            checksum,
            fsid,
            bytenr,
            flags,
            chunk_tree_uuid,
            generation,
            owner,
            nritems,
            level,
        })
    }
    
    /// Size of header in bytes
    pub const SIZE: usize = 101;
}

/// Key pointer for internal (non-leaf) nodes
#[derive(Debug, Clone)]
pub struct BtrfsKeyPtr {
    pub key: BtrfsKey,
    pub blockptr: u64,    // Logical address of child node
    pub generation: u64,
}

impl BtrfsKeyPtr {
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < 33 {
            bail!("Insufficient data for BtrfsKeyPtr");
        }
        let key = BtrfsKey::parse(&data[0..17])?;
        let mut cursor = Cursor::new(&data[17..]);
        Ok(Self {
            key,
            blockptr: cursor.read_u64::<LittleEndian>()?,
            generation: cursor.read_u64::<LittleEndian>()?,
        })
    }
    
    pub const SIZE: usize = 33;
}

/// Item header for leaf nodes
#[derive(Debug, Clone)]
pub struct BtrfsItem {
    pub key: BtrfsKey,
    pub offset: u32,      // Offset from end of header to item data
    pub size: u32,        // Size of item data
}

impl BtrfsItem {
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < 25 {
            bail!("Insufficient data for BtrfsItem");
        }
        let key = BtrfsKey::parse(&data[0..17])?;
        let mut cursor = Cursor::new(&data[17..]);
        Ok(Self {
            key,
            offset: cursor.read_u32::<LittleEndian>()?,
            size: cursor.read_u32::<LittleEndian>()?,
        })
    }
    
    pub const SIZE: usize = 25;
}

/// A parsed tree node (internal or leaf)
#[derive(Debug)]
pub struct BtrfsNode {
    pub header: BtrfsHeader,
    pub is_leaf: bool,
    pub key_ptrs: Vec<BtrfsKeyPtr>,  // For internal nodes
    pub items: Vec<BtrfsItem>,        // For leaf nodes  
    pub raw_data: Vec<u8>,            // Raw node data for extracting item content
}

impl BtrfsNode {
    /// Get the data for a specific item in a leaf node
    pub fn get_item_data(&self, item: &BtrfsItem) -> Option<&[u8]> {
        if !self.is_leaf {
            return None;
        }
        
        // Item data is stored at the end of the node, growing backwards
        // offset is relative to start of node data (after header)
        let data_start = BtrfsHeader::SIZE + item.offset as usize;
        let data_end = data_start + item.size as usize;
        
        if data_end <= self.raw_data.len() {
            Some(&self.raw_data[data_start..data_end])
        } else {
            None
        }
    }
}

// ============================================================================
// Tree Reader
// ============================================================================

/// Reads and traverses Btrfs B-trees
pub struct BtrfsTreeReader<'a> {
    device: &'a BlockDevice,
    nodesize: u32,
}

impl<'a> BtrfsTreeReader<'a> {
    pub fn new(device: &'a BlockDevice, nodesize: u32) -> Self {
        Self { device, nodesize }
    }
    
    /// Read and parse a tree node at the given logical address
    pub fn read_node(&self, bytenr: u64) -> Result<BtrfsNode> {
        let data = self.device.read_bytes(bytenr, self.nodesize as usize)?;
        self.parse_node(&data)
    }
    
    /// Parse a node from raw bytes
    fn parse_node(&self, data: &[u8]) -> Result<BtrfsNode> {
        if data.len() < BtrfsHeader::SIZE {
            bail!("Node data too small");
        }
        
        let header = BtrfsHeader::parse(data)?;
        let is_leaf = header.level == 0;
        
        let mut key_ptrs = Vec::new();
        let mut items = Vec::new();
        
        if is_leaf {
            // Leaf node: parse item headers
            let mut offset = BtrfsHeader::SIZE;
            for _ in 0..header.nritems {
                if offset + BtrfsItem::SIZE > data.len() {
                    break;
                }
                items.push(BtrfsItem::parse(&data[offset..])?);
                offset += BtrfsItem::SIZE;
            }
        } else {
            // Internal node: parse key pointers
            let mut offset = BtrfsHeader::SIZE;
            for _ in 0..header.nritems {
                if offset + BtrfsKeyPtr::SIZE > data.len() {
                    break;
                }
                key_ptrs.push(BtrfsKeyPtr::parse(&data[offset..])?);
                offset += BtrfsKeyPtr::SIZE;
            }
        }
        
        Ok(BtrfsNode {
            header,
            is_leaf,
            key_ptrs,
            items,
            raw_data: data.to_vec(),
        })
    }
    
    /// Search for a specific key in a tree
    /// Returns the leaf node containing the key (or where it would be)
    pub fn search_tree(&self, root_bytenr: u64, key: &BtrfsKey) -> Result<Option<(BtrfsNode, usize)>> {
        let mut current_bytenr = root_bytenr;
        
        loop {
            let node = self.read_node(current_bytenr)?;
            
            if node.is_leaf {
                // Search for the key in the leaf
                for (i, item) in node.items.iter().enumerate() {
                    if item.key == *key {
                        return Ok(Some((node, i)));
                    }
                }
                return Ok(None);
            } else {
                // Binary search in internal node to find next level
                let mut next_idx = 0;
                for (i, kp) in node.key_ptrs.iter().enumerate() {
                    if kp.key > *key {
                        break;
                    }
                    next_idx = i;
                }
                
                if next_idx < node.key_ptrs.len() {
                    current_bytenr = node.key_ptrs[next_idx].blockptr;
                } else {
                    return Ok(None);
                }
            }
        }
    }
    
    /// Iterate all items in a tree (in-order traversal)
    pub fn iterate_tree<F>(&self, root_bytenr: u64, mut callback: F) -> Result<()>
    where
        F: FnMut(&BtrfsNode, &BtrfsItem) -> Result<bool>, // Return false to stop
    {
        self.iterate_node(root_bytenr, &mut callback)
    }
    
    fn iterate_node<F>(&self, bytenr: u64, callback: &mut F) -> Result<()>
    where
        F: FnMut(&BtrfsNode, &BtrfsItem) -> Result<bool>,
    {
        let node = self.read_node(bytenr)?;
        
        if node.is_leaf {
            for item in &node.items {
                if !callback(&node, item)? {
                    return Ok(());
                }
            }
        } else {
            for kp in &node.key_ptrs {
                self.iterate_node(kp.blockptr, callback)?;
            }
        }
        
        Ok(())
    }
    
    /// Find all items matching a specific object ID and type
    pub fn find_items_by_type(&self, root_bytenr: u64, objectid: u64, item_type: u8) -> Result<Vec<(BtrfsKey, Vec<u8>)>> {
        let mut results = Vec::new();
        
        self.iterate_tree(root_bytenr, |node, item| {
            if item.key.objectid == objectid && item.key.item_type == item_type {
                if let Some(data) = node.get_item_data(item) {
                    results.push((item.key.clone(), data.to_vec()));
                }
            }
            Ok(true) // Continue iteration
        })?;
        
        Ok(results)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_btrfs_key_parse() {
        let mut data = vec![0u8; 17];
        // objectid = 256 (first free objectid)
        data[0..8].copy_from_slice(&256u64.to_le_bytes());
        // item_type = 1 (INODE_ITEM)
        data[8] = 1;
        // offset = 0
        data[9..17].copy_from_slice(&0u64.to_le_bytes());
        
        let key = BtrfsKey::parse(&data).unwrap();
        assert_eq!(key.objectid, 256);
        assert_eq!(key.item_type, BTRFS_INODE_ITEM_KEY);
        assert_eq!(key.offset, 0);
    }
    
    #[test]
    fn test_btrfs_key_ordering() {
        let key1 = BtrfsKey { objectid: 100, item_type: 1, offset: 0 };
        let key2 = BtrfsKey { objectid: 100, item_type: 2, offset: 0 };
        let key3 = BtrfsKey { objectid: 200, item_type: 1, offset: 0 };
        
        assert!(key1 < key2);
        assert!(key2 < key3);
        assert!(key1 < key3);
    }
    
    #[test]
    fn test_btrfs_header_size() {
        assert_eq!(BtrfsHeader::SIZE, 101);
    }
}
