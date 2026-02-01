#!/bin/bash
# Create a large XFS test image using Docker
# Works on macOS, Windows, Linux - anywhere Docker is available

set -e

IMAGE_FILE="test-data/large-xfs-test.img"
IMAGE_SIZE_MB=3072  # 3GB
DOWNLOADS_DIR="$HOME/Downloads"

echo "🐋 Creating 3GB XFS test image using Docker..."

# Create test-data directory if it doesn't exist
mkdir -p test-data

# Check if Docker is running
if ! docker info > /dev/null 2>&1; then
    echo "❌ Docker is not running. Please start Docker Desktop and try again."
    exit 1
fi

echo "📝 Creating ${IMAGE_SIZE_MB}MB image file..."
dd if=/dev/zero of="$IMAGE_FILE" bs=1M count=$IMAGE_SIZE_MB status=progress

echo "🐳 Running Docker container with XFS support..."
docker run --rm -it \
    -v "$(pwd)/$IMAGE_FILE:/work/disk.img" \
    -v "$DOWNLOADS_DIR:/downloads:ro" \
    --privileged \
    ubuntu:22.04 bash -c '
    set -e
    
    echo "📦 Installing XFS tools..."
    apt-get update -qq
    apt-get install -y -qq xfsprogs > /dev/null 2>&1
    
    echo "💾 Formatting as XFS..."
    mkfs.xfs -f /work/disk.img
    
    echo "📁 Mounting filesystem..."
    mkdir -p /mnt/xfs
    mount -o loop /work/disk.img /mnt/xfs
    
    echo "📦 Copying Downloads folder..."
    if [ -d /downloads ] && [ "$(ls -A /downloads 2>/dev/null)" ]; then
        cp -r /downloads/* /mnt/xfs/ 2>/dev/null || echo "  Some files skipped"
        echo "✅ Downloads copied"
    else
        echo "⚠️  No Downloads folder, creating sample data..."
        mkdir -p /mnt/xfs/documents /mnt/xfs/images
        echo "Sample document for testing" > /mnt/xfs/documents/test.txt
        echo "Another test file" > /mnt/xfs/documents/important.doc
    fi
    
    echo "🎬 Creating test video files..."
    dd if=/dev/urandom of=/mnt/xfs/test-video-1.mp4 bs=1M count=50 2>/dev/null
    dd if=/dev/urandom of=/mnt/xfs/test-video-2.mov bs=1M count=75 2>/dev/null
    
    echo "📄 Creating additional test files..."
    dd if=/dev/urandom of=/mnt/xfs/large-file.bin bs=1M count=100 2>/dev/null
    echo "{\"test\": \"data\", \"numbers\": [1,2,3,4,5]}" > /mnt/xfs/data.json
    
    # Create PDF-like file (with PDF header)
    echo -n "%PDF-1.4" > /mnt/xfs/document.pdf
    dd if=/dev/urandom bs=1K count=500 2>/dev/null >> /mnt/xfs/document.pdf
    
    sync
    
    echo "📊 Filesystem contents:"
    df -h /mnt/xfs
    echo "Files created: $(find /mnt/xfs -type f | wc -l)"
    
    echo "🗑️  Deleting random files for recovery testing..."
    # Delete various file types
    find /mnt/xfs -name "*.pdf" -type f | head -2 | while read f; do
        echo "  Deleting: $(basename "$f")"
        rm -f "$f"
    done
    
    find /mnt/xfs -name "*.txt" -type f | head -3 | while read f; do
        echo "  Deleting: $(basename "$f")"
        rm -f "$f"
    done
    
    find /mnt/xfs -name "*.mp4" -o -name "*.mov" | head -1 | while read f; do
        echo "  Deleting: $(basename "$f")"
        rm -f "$f"
    done
    
    find /mnt/xfs -name "*.json" | head -1 | while read f; do
        echo "  Deleting: $(basename "$f")"
        rm -f "$f"
    done
    
    sync
    
    echo "📋 Files remaining: $(find /mnt/xfs -type f | wc -l)"
    
    echo "💿 Unmounting..."
    umount /mnt/xfs
    
    echo "✅ XFS image created successfully inside container"
'

echo ""
echo "✅ Large XFS test image created!"
echo "📍 Location: $IMAGE_FILE"
echo "📊 Size: $(ls -lh $IMAGE_FILE | awk '{print $5}')"
echo ""
echo "🧪 To test scanning:"
echo "   cargo run -p ghostfs-cli -- scan $IMAGE_FILE --fs xfs --info"
echo ""
echo "🔍 To test recovery:"
echo "   mkdir -p recovered-test"
echo "   cargo run -p ghostfs-cli -- recover $IMAGE_FILE --fs xfs --out recovered-test"
