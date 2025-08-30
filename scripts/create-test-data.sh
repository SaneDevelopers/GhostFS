#!/bin/bash
# Create test file system images for development

set -e

echo "💾 Creating test file system images..."

TEST_DIR="test-data"
mkdir -p "$TEST_DIR"

# Function to create and mount a test image
create_test_image() {
    local fs_type="$1"
    local image_name="$2"
    local size="$3"
    local mount_point="/tmp/ghostfs_test_$$"
    
    echo "Creating $fs_type test image: $image_name ($size)"
    
    # Create empty image
    dd if=/dev/zero of="$TEST_DIR/$image_name" bs=1M count="$size" 2>/dev/null
    
    # Create file system
    case "$fs_type" in
        "xfs")
            if command -v mkfs.xfs &> /dev/null; then
                mkfs.xfs -f "$TEST_DIR/$image_name" >/dev/null 2>&1
            else
                echo "⚠️  mkfs.xfs not found, creating empty image"
                return
            fi
            ;;
        "btrfs")
            if command -v mkfs.btrfs &> /dev/null; then
                mkfs.btrfs -f "$TEST_DIR/$image_name" >/dev/null 2>&1
            else
                echo "⚠️  mkfs.btrfs not found, creating empty image"
                return
            fi
            ;;
        "exfat")
            if command -v mkfs.exfat &> /dev/null; then
                mkfs.exfat "$TEST_DIR/$image_name" >/dev/null 2>&1
            else
                echo "⚠️  mkfs.exfat not found, creating empty image"
                return
            fi
            ;;
    esac
    
    # Mount and create test files (requires sudo, so skip for now)
    # We'll add this functionality later when we need real test data
    
    echo "✅ Created $image_name"
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
