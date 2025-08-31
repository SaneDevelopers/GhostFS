#!/bin/bash
# Create test file system images with real data for development

set -e

echo "💾 Creating test file system images with real data..."

TEST_DIR="test-data"
mkdir -p "$TEST_DIR"

# Create temporary files for testing
create_test_files() {
    local temp_dir="$1"
    
    echo "📝 Creating test files..."
    
    # Create a text document
    cat > "$temp_dir/document.txt" << 'EOF'
This is a test document for file recovery testing.
It contains multiple lines of text to simulate real file content.
Created by GhostFS test data generator.
Contains special characters: !@#$%^&*()
Unicode test: ñáéíóú αβγδε 日本語 русский
Numbers: 1234567890
EOF

    # Create a configuration file
    cat > "$temp_dir/config.ini" << 'EOF'
[Settings]
version=1.0
debug=true
max_files=1000

[Database]
host=localhost
port=5432
name=testdb

[Logging]
level=INFO
output=/var/log/app.log
EOF

    # Create a JSON file
    cat > "$temp_dir/data.json" << 'EOF'
{
  "users": [
    {
      "id": 1,
      "name": "John Doe",
      "email": "john@example.com",
      "active": true
    },
    {
      "id": 2, 
      "name": "Jane Smith",
      "email": "jane@example.com",
      "active": false
    }
  ],
  "settings": {
    "theme": "dark",
    "notifications": true,
    "auto_save": true
  }
}
EOF

    # Create a shell script
    cat > "$temp_dir/backup.sh" << 'EOF'
#!/bin/bash
# Backup script for testing
echo "Starting backup process..."
tar -czf backup-$(date +%Y%m%d).tar.gz /important/data
echo "Backup completed successfully"
EOF

    # Create a binary-like file with some structured data
    printf "\x7fELF\x01\x01\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00Binary test data for recovery\x00" > "$temp_dir/binary_test.bin"
    
    # Create a larger text file
    for i in {1..100}; do
        echo "Line $i: This is test data for large file recovery testing. $(date)" >> "$temp_dir/large_file.txt"
    done
    
    chmod +x "$temp_dir/backup.sh"
    echo "✅ Created test files in $temp_dir"
}

# Function to create and populate a test image
create_test_image() {
    local fs_type="$1"
    local image_name="$2"
    local size="$3"
    local mount_point="/tmp/ghostfs_mount_$$"
    local temp_files="/tmp/ghostfs_files_$$"
    
    echo "🔨 Creating $fs_type test image: $image_name (${size}MB)"
    
    # Create empty image
    dd if=/dev/zero of="$TEST_DIR/$image_name" bs=1M count="$size" 2>/dev/null
    
    # Create file system based on available tools
    case "$fs_type" in
        "xfs")
            if command -v mkfs.xfs &> /dev/null; then
                mkfs.xfs -f "$TEST_DIR/$image_name" >/dev/null 2>&1
                echo "✅ Created XFS filesystem"
            else
                echo "⚠️  mkfs.xfs not found, creating simulated XFS image"
                create_simulated_xfs_image "$TEST_DIR/$image_name"
                return
            fi
            ;;
        "btrfs")
            if command -v mkfs.btrfs &> /dev/null; then
                mkfs.btrfs -f "$TEST_DIR/$image_name" >/dev/null 2>&1
                echo "✅ Created Btrfs filesystem"
            else
                echo "⚠️  mkfs.btrfs not found, creating simulated image"
                create_simulated_image "$TEST_DIR/$image_name" "$fs_type"
                return
            fi
            ;;
        "exfat")
            if command -v mkfs.exfat &> /dev/null; then
                mkfs.exfat "$TEST_DIR/$image_name" >/dev/null 2>&1
                echo "✅ Created ExFAT filesystem"
            else
                echo "⚠️  mkfs.exfat not found, creating simulated image"
                create_simulated_image "$TEST_DIR/$image_name" "$fs_type"
                return
            fi
            ;;
    esac
    
    # Try to mount and populate with real data
    if populate_image_with_data "$TEST_DIR/$image_name" "$mount_point" "$temp_files"; then
        echo "✅ Successfully populated $image_name with real data"
    else
        echo "⚠️  Could not mount image, creating simulated data"
        create_simulated_image "$TEST_DIR/$image_name" "$fs_type"
    fi
}

# Function to populate image with real data
populate_image_with_data() {
    local image_path="$1"
    local mount_point="$2" 
    local temp_files="$3"
    
    # Create temporary directory for files
    mkdir -p "$temp_files"
    create_test_files "$temp_files"
    
    # Try to mount (this might require sudo or special permissions)
    mkdir -p "$mount_point" 2>/dev/null || return 1
    
    # Attempt to mount (will fail without proper permissions, but that's OK)
    if sudo mount -o loop "$image_path" "$mount_point" 2>/dev/null; then
        echo "📁 Mounted filesystem, copying test files..."
        
        # Copy test files
        cp -r "$temp_files"/* "$mount_point/" 2>/dev/null || true
        
        # Create some additional structure
        mkdir -p "$mount_point/documents" "$mount_point/config" "$mount_point/temp" 2>/dev/null || true
        cp "$temp_files/document.txt" "$mount_point/documents/" 2>/dev/null || true
        cp "$temp_files/config.ini" "$mount_point/config/" 2>/dev/null || true
        
        # Simulate deletion by removing some files after copying
        rm -f "$mount_point/config.ini" 2>/dev/null || true
        rm -f "$mount_point/documents/document.txt" 2>/dev/null || true
        
        # Unmount
        sudo umount "$mount_point" 2>/dev/null || true
        rmdir "$mount_point" 2>/dev/null || true
        rm -rf "$temp_files"
        return 0
    else
        rm -rf "$temp_files" "$mount_point" 2>/dev/null || true
        return 1
    fi
}

# Function to create simulated XFS image with realistic data patterns
create_simulated_xfs_image() {
    local image_path="$1"
    local temp_files="/tmp/ghostfs_sim_$$"
    
    echo "🔧 Creating simulated XFS image with real data patterns..."
    
    # Create temporary files
    mkdir -p "$temp_files"
    create_test_files "$temp_files"
    
    # Create a basic XFS-like structure in the image
    # XFS superblock signature at offset 0
    printf "XFSB" | dd of="$image_path" bs=1 seek=0 conv=notrunc 2>/dev/null
    
    # Add some XFS-specific fields (simplified)
    # Block size (4096 bytes)
    printf "\x00\x00\x10\x00" | dd of="$image_path" bs=1 seek=4 conv=notrunc 2>/dev/null
    # Data blocks count (simulate smaller filesystem)
    printf "\x00\x00\x32\x00" | dd of="$image_path" bs=1 seek=8 conv=notrunc 2>/dev/null
    # AG count (4 allocation groups)
    printf "\x00\x00\x00\x04" | dd of="$image_path" bs=1 seek=88 conv=notrunc 2>/dev/null
    # AG blocks (simulate)
    printf "\x00\x00\x0C\x80" | dd of="$image_path" bs=1 seek=84 conv=notrunc 2>/dev/null
    # Inode size (256 bytes)
    printf "\x01\x00" | dd of="$image_path" bs=1 seek=104 conv=notrunc 2>/dev/null
    
    # Embed actual file content at various offsets to simulate deleted files
    local offset=4096
    
    # Embed document.txt content
    dd if="$temp_files/document.txt" of="$image_path" bs=1 seek=$offset conv=notrunc 2>/dev/null
    offset=$((offset + 2048))
    
    # Embed config.ini content  
    dd if="$temp_files/config.ini" of="$image_path" bs=1 seek=$offset conv=notrunc 2>/dev/null
    offset=$((offset + 1024))
    
    # Embed JSON data
    dd if="$temp_files/data.json" of="$image_path" bs=1 seek=$offset conv=notrunc 2>/dev/null
    offset=$((offset + 1024))
    
    # Embed binary test data
    dd if="$temp_files/binary_test.bin" of="$image_path" bs=1 seek=$offset conv=notrunc 2>/dev/null
    offset=$((offset + 512))
    
    # Add some inode-like structures at predictable locations
    # Simulate directory entries and metadata
    local inode_offset=8192
    for file in "$temp_files"/*; do
        if [ -f "$file" ]; then
            filename=$(basename "$file")
            # Write filename in a directory-like structure
            printf "%-32s" "$filename" | dd of="$image_path" bs=1 seek=$inode_offset conv=notrunc 2>/dev/null
            inode_offset=$((inode_offset + 64))
        fi
    done
    
    # Add file signatures at various locations for better detection
    # PNG signature
    printf "\x89PNG\x0D\x0A\x1A\x0A" | dd of="$image_path" bs=1 seek=16384 conv=notrunc 2>/dev/null
    # JPEG signature  
    printf "\xFF\xD8\xFF\xE0" | dd of="$image_path" bs=1 seek=20480 conv=notrunc 2>/dev/null
    # ZIP signature
    printf "PK\x03\x04" | dd of="$image_path" bs=1 seek=24576 conv=notrunc 2>/dev/null
    
    rm -rf "$temp_files"
    echo "✅ Created simulated XFS image with embedded test data"
}

# Function to create simulated images for other filesystems  
create_simulated_image() {
    local image_path="$1"
    local fs_type="$2"
    local temp_files="/tmp/ghostfs_sim_$$"
    
    echo "🔧 Creating simulated $fs_type image with test data..."
    
    mkdir -p "$temp_files"
    create_test_files "$temp_files"
    
    # Add filesystem signature based on type
    case "$fs_type" in
        "btrfs")
            # Btrfs magic number
            printf "_BHRfS_M" | dd of="$image_path" bs=1 seek=65536 conv=notrunc 2>/dev/null
            ;;
        "exfat")
            # ExFAT signature
            printf "EXFAT   " | dd of="$image_path" bs=1 seek=3 conv=notrunc 2>/dev/null
            ;;
    esac
    
    # Embed test file content
    local offset=8192
    for file in "$temp_files"/*; do
        if [ -f "$file" ]; then
            dd if="$file" of="$image_path" bs=1 seek=$offset conv=notrunc 2>/dev/null
            offset=$((offset + 2048))
        fi
    done
    
    rm -rf "$temp_files"
    echo "✅ Created simulated $fs_type image with test data"
}

# Create test images
create_test_image "xfs" "test-xfs.img" 50
create_test_image "btrfs" "test-btrfs.img" 50
create_test_image "exfat" "test-exfat.img" 50

# Create a simple binary file for testing
echo "Creating test binary file..."
echo -e "\x7fELF\x01\x01\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00" > "$TEST_DIR/sample-binary.bin"

# Create a text file for testing
echo "Creating test text file..."
cat > "$TEST_DIR/sample-text.txt" << 'EOF'
This is a sample text file for testing GhostFS recovery capabilities.
It contains multiple lines and various characters: !@#$%^&*()
Unicode characters: ñáéíóú αβγδε 日本語
Numbers: 1234567890
EOF

echo "📁 Test data created in $TEST_DIR/"
echo "Contents:"
ls -la "$TEST_DIR/"
