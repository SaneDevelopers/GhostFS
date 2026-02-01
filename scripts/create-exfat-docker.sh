#!/bin/bash
# Create an exFAT test image using Docker (with privileged mode for mounting)
# This script uses Docker because macOS doesn't have native exFAT mkfs tools

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
TEST_DATA_DIR="$PROJECT_ROOT/test-data"
IMAGE_FILE="$TEST_DATA_DIR/test-exfat.img"
IMAGE_SIZE="50M"

echo "🔧 Creating exFAT test image using Docker..."

# Create test-data directory if it doesn't exist
mkdir -p "$TEST_DATA_DIR"

# Create the test image using Docker with privileged mode for mounting
docker run --rm --privileged -v "$TEST_DATA_DIR:/data" ubuntu:22.04 bash -c "
    set -e
    
    echo '📦 Installing exfat tools...'
    apt-get update -qq > /dev/null
    apt-get install -y -qq exfatprogs > /dev/null
    
    echo '💾 Creating $IMAGE_SIZE image file...'
    dd if=/dev/zero of=/data/test-exfat.img bs=1M count=50 status=none
    
    echo '📝 Formatting as exFAT...'
    mkfs.exfat -n GHOSTFS /data/test-exfat.img
    
    echo '📂 Mounting and populating with test files...'
    mkdir -p /mnt/exfat
    mount /data/test-exfat.img /mnt/exfat
    
    # Create test files with recognizable content
    echo 'Creating test files...'
    
    # Text file
    echo 'This is a test file for GhostFS exFAT recovery.' > /mnt/exfat/readme.txt
    
    # Create a file with JPEG signature
    printf '\xFF\xD8\xFF\xE0\x00\x10JFIF\x00\x01\x01\x00\x00\x01\x00\x01\x00\x00' > /mnt/exfat/photo.jpg
    # Add some random data to make it realistic
    dd if=/dev/urandom bs=1K count=10 >> /mnt/exfat/photo.jpg 2>/dev/null
    # Add JPEG end marker
    printf '\xFF\xD9' >> /mnt/exfat/photo.jpg
    
    # Create a file with PNG signature  
    printf '\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR' > /mnt/exfat/image.png
    dd if=/dev/urandom bs=1K count=10 >> /mnt/exfat/image.png 2>/dev/null
    printf 'IEND\xAE\x42\x60\x82' >> /mnt/exfat/image.png
    
    # Create a subdirectory with files
    mkdir -p /mnt/exfat/documents
    echo 'Document 1 content' > /mnt/exfat/documents/doc1.txt
    echo 'Document 2 content' > /mnt/exfat/documents/doc2.txt
    
    # Sync to ensure data is written
    sync
    
    echo 'Listing files before deletion...'
    ls -la /mnt/exfat/
    ls -la /mnt/exfat/documents/
    
    # Delete some files to create recovery targets
    echo '🗑️  Deleting files to simulate recovery scenario...'
    rm /mnt/exfat/photo.jpg
    rm /mnt/exfat/documents/doc1.txt
    
    sync
    
    echo 'Listing files after deletion...'
    ls -la /mnt/exfat/
    ls -la /mnt/exfat/documents/
    
    # Unmount
    umount /mnt/exfat
    
    echo '✅ exFAT test image created successfully!'
"

echo ""
echo "📁 Test image created at: $IMAGE_FILE"
echo "📊 Size: $(ls -lh "$IMAGE_FILE" | awk '{print $5}')"
echo ""
echo "🔍 To test exFAT recovery:"
echo "   cargo run -p ghostfs-cli -- scan $IMAGE_FILE --fs exfat"
echo "   cargo run -p ghostfs-cli -- recover $IMAGE_FILE --fs exfat --out ./recovered"
