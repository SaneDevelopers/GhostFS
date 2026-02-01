#!/usr/bin/env python3
"""
Expand an existing XFS test image to appear larger (for testing adaptive scanning)
This modifies the superblock to make the filesystem appear 150GB while keeping actual file small
"""

import struct
import sys
import os

def expand_xfs_image(input_file, output_file, target_size_gb=150):
    """
    Create a larger XFS image by:
    1. Copying the existing small XFS image
    2. Padding it to appear larger
    3. Updating the superblock to reflect the new size
    """
    
    print(f"📝 Expanding {input_file} to appear as {target_size_gb}GB...")
    
    # Read original image
    with open(input_file, 'rb') as f:
        data = bytearray(f.read())
    
    print(f"📊 Original size: {len(data)} bytes ({len(data) / (1024**2):.2f} MB)")
    
    # XFS Superblock is at offset 0
    # Read current values
    magic = struct.unpack('>I', data[0:4])[0]
    
    if magic != 0x58465342:  # "XFSB"
        print(f"❌ Not a valid XFS image (magic: 0x{magic:08x})")
        return False
    
    block_size = struct.unpack('>I', data[4:8])[0]
    original_blocks = struct.unpack('>Q', data[8:16])[0]
    
    print(f"📦 Block size: {block_size} bytes")
    print(f"📦 Original blocks: {original_blocks}")
    print(f"📦 Original size: {(original_blocks * block_size) / (1024**3):.2f} GB")
    
    # Calculate new block count for target size
    target_bytes = target_size_gb * 1024 * 1024 * 1024
    new_blocks = target_bytes // block_size
    
    print(f"🎯 Target blocks: {new_blocks}")
    print(f"🎯 Target size: {(new_blocks * block_size) / (1024**3):.2f} GB")
    
    # Update the data_blocks field in superblock (offset 8-15, big-endian u64)
    data[8:16] = struct.pack('>Q', new_blocks)
    
    # Also update ag_blocks if needed (offset 84-87, big-endian u32)
    # For large filesystems, typically have more/larger AGs
    ag_count = struct.unpack('>I', data[88:92])[0] if len(data) >= 92 else 4
    new_ag_blocks = new_blocks // ag_count if ag_count > 0 else new_blocks
    
    if len(data) >= 88:
        data[84:88] = struct.pack('>I', new_ag_blocks)
        print(f"📦 AG blocks updated to: {new_ag_blocks}")
    
    # Pad the file to at least a reasonable size (100MB) so it looks somewhat real
    min_physical_size = 100 * 1024 * 1024  # 100MB
    if len(data) < min_physical_size:
        padding = min_physical_size - len(data)
        data.extend(b'\x00' * padding)
        print(f"📏 Padded to {len(data) / (1024**2):.2f} MB physical size")
    
    # Write the modified image
    with open(output_file, 'wb') as f:
        f.write(data)
    
    print(f"✅ Created: {output_file}")
    print(f"💾 Physical size: {len(data) / (1024**2):.2f} MB")
    print(f"📊 Reported size: {(new_blocks * block_size) / (1024**3):.2f} GB")
    
    return True

if __name__ == "__main__":
    input_img = "test-data/test-xfs.img"
    output_img = "test-data/large-xfs-test.img"
    
    if not os.path.exists(input_img):
        print(f"❌ Input file not found: {input_img}")
        sys.exit(1)
    
    # Create a 150GB image (will trigger the >100GB prompt)
    if expand_xfs_image(input_img, output_img, target_size_gb=150):
        print("")
        print("🧪 Test with:")
        print(f"   cargo run -p ghostfs-cli -- scan {output_img} --fs xfs --info")
        print("")
        print("   This should trigger the interactive prompt!")
    else:
        sys.exit(1)
