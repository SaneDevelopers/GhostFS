#!/usr/bin/env python3
"""Create a minimal XFS test image with proper signature and some test data"""

import struct
import os

def create_xfs_test_image(filename, size_mb=50):
    """Create a minimal XFS test image"""
    size_bytes = size_mb * 1024 * 1024
    
    with open(filename, 'wb') as f:
        # Write XFS superblock signature at offset 0
        f.write(b'XFSB')  # XFS magic number
        
        # Add some basic XFS superblock fields (simplified)
        f.write(struct.pack('<I', 4096))      # Block size (4096 bytes)
        f.write(struct.pack('<Q', size_bytes // 4096))  # Data blocks count
        f.write(b'\x00' * 76)  # Padding to reach offset 88
        f.write(struct.pack('<I', 4))         # AG count (4 allocation groups)
        f.write(struct.pack('<I', (size_bytes // 4096) // 4))  # AG blocks
        f.write(b'\x00' * 16)  # More padding
        f.write(struct.pack('<H', 256))       # Inode size (256 bytes)
        
        # Fill rest of superblock with zeros
        current_pos = f.tell()
        f.write(b'\x00' * (4096 - current_pos))
        
        # Add some test file content at various offsets to simulate deleted files
        test_content = [
            (8192, b"This is a test document for XFS recovery.\nIt contains multiple lines.\nCreated by GhostFS test generator.\n"),
            (12288, b'{"users": [{"id": 1, "name": "John Doe"}, {"id": 2, "name": "Jane Smith"}], "config": {"theme": "dark"}}'),
            (16384, b"[Settings]\nversion=1.0\ndebug=true\nmax_files=1000\n\n[Database]\nhost=localhost\nport=5432\n"),
            (20480, b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x10\x00\x00\x00\x10"),  # PNG signature
            (24576, b"\xff\xd8\xff\xe0\x00\x10JFIF"),  # JPEG signature
        ]
        
        for offset, content in test_content:
            f.seek(offset)
            f.write(content)
        
        # Fill the rest of the file
        f.seek(size_bytes - 1)
        f.write(b'\x00')
    
    print(f"Created XFS test image: {filename} ({size_mb}MB)")

if __name__ == "__main__":
    create_xfs_test_image("test-data/test-xfs.img", 50)
