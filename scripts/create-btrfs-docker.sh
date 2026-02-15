#!/bin/bash
# Create a Btrfs test image using Docker
# This script creates a Btrfs-formatted disk image with test files,
# then deletes some files to test recovery

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
TEST_DATA_DIR="$PROJECT_DIR/test-data"
IMAGE_FILE="$TEST_DATA_DIR/test-btrfs.img"
IMAGE_SIZE="100M"

echo "🔧 Creating Btrfs test image using Docker..."
echo "   Output: $IMAGE_FILE"

# Create test-data directory if needed
mkdir -p "$TEST_DATA_DIR"

# Create empty image file (Btrfs needs at least 114MB)
echo "📦 Creating empty image file (120MB - Btrfs minimum)..."
dd if=/dev/zero of="$IMAGE_FILE" bs=1M count=120 2>/dev/null

# Use Docker to format and populate the image
echo "🐳 Running Docker container to create Btrfs filesystem..."

docker run --rm --privileged \
    -v "$TEST_DATA_DIR:/data" \
    ubuntu:22.04 /bin/bash -c '
    set -e
    
    # Install btrfs-progs
    apt-get update -qq && apt-get install -y -qq btrfs-progs > /dev/null 2>&1
    
    IMAGE="/data/test-btrfs.img"
    MOUNT_POINT="/mnt/btrfs"
    
    echo "📂 Formatting image as Btrfs..."
    mkfs.btrfs -f "$IMAGE" > /dev/null
    
    echo "📁 Mounting filesystem..."
    mkdir -p "$MOUNT_POINT"
    mount -o loop "$IMAGE" "$MOUNT_POINT"
    
    echo "✍️  Creating test files..."
    
    # Create various test files
    echo "Hello, this is a test file for GhostFS recovery testing." > "$MOUNT_POINT/hello.txt"
    echo "This is another test file with some content." > "$MOUNT_POINT/readme.txt"
    echo "Important document content here." > "$MOUNT_POINT/important.doc"
    
    # Create a directory with files
    mkdir -p "$MOUNT_POINT/documents"
    echo "Document 1 content" > "$MOUNT_POINT/documents/doc1.txt"
    echo "Document 2 content" > "$MOUNT_POINT/documents/doc2.txt"
    
    # Create binary-ish files (JPEG signature for testing)
    printf "\xFF\xD8\xFF\xE0\x00\x10JFIF" > "$MOUNT_POINT/photo.jpg"
    dd if=/dev/urandom bs=1024 count=10 >> "$MOUNT_POINT/photo.jpg" 2>/dev/null
    
    # PNG signature
    printf "\x89PNG\r\n\x1A\n" > "$MOUNT_POINT/image.png"
    dd if=/dev/urandom bs=1024 count=5 >> "$MOUNT_POINT/image.png" 2>/dev/null
    
    # Sync to disk
    sync
    
    echo "📊 Files before deletion:"
    ls -la "$MOUNT_POINT/"
    ls -la "$MOUNT_POINT/documents/"
    
    echo "🗑️  Deleting some files (for recovery testing)..."
    rm "$MOUNT_POINT/hello.txt"
    rm "$MOUNT_POINT/important.doc"
    rm "$MOUNT_POINT/documents/doc1.txt"
    rm "$MOUNT_POINT/photo.jpg"
    
    # Sync again
    sync
    
    echo "📊 Files after deletion:"
    ls -la "$MOUNT_POINT/" || true
    ls -la "$MOUNT_POINT/documents/" || true
    
    echo "📤 Unmounting..."
    umount "$MOUNT_POINT"
    
    echo "✅ Btrfs test image created successfully!"
'

echo ""
echo "✅ Done! Test image created at: $IMAGE_FILE"
echo ""
echo "To test recovery, run:"
echo "  cargo run -p ghostfs-cli -- scan $IMAGE_FILE --fs btrfs"
echo "  cargo run -p ghostfs-cli -- recover $IMAGE_FILE --fs btrfs --out ./recovered"
