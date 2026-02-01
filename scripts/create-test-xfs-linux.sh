#!/bin/bash
# Create realistic XFS test image (Linux/WSL only)
# Run this on a Linux machine or WSL

set -e

IMAGE_FILE="large-xfs-test.img"
IMAGE_SIZE="3G"
MOUNT_POINT="/tmp/ghostfs-test-mount"
DOWNLOADS_DIR="$HOME/Downloads"

echo "🔧 Creating realistic 3GB XFS test image with real data..."
echo "⚠️  This script requires Linux with XFS support"

# Check for XFS tools
if ! command -v mkfs.xfs &> /dev/null; then
    echo "❌ mkfs.xfs not found. Install with:"
    echo "   Ubuntu/Debian: sudo apt-get install xfsprogs"
    echo "   Fedora/RHEL: sudo dnf install xfsprogs"
    exit 1
fi

# Create image
echo "📝 Creating $IMAGE_SIZE image..."
dd if=/dev/zero of="$IMAGE_FILE" bs=1M count=3072 status=progress

# Format
echo "💾 Formatting as XFS..."
mkfs.xfs -f "$IMAGE_FILE"

# Mount
echo "📁 Mounting..."
mkdir -p "$MOUNT_POINT"
sudo mount -o loop "$IMAGE_FILE" "$MOUNT_POINT"

# Copy Downloads
if [ -d "$DOWNLOADS_DIR" ]; then
    echo "📦 Copying Downloads folder..."
    sudo cp -r "$DOWNLOADS_DIR"/* "$MOUNT_POINT/" 2>/dev/null || true
fi

# Add test videos
echo "🎬 Adding test video files..."
for i in 1 2; do
    sudo dd if=/dev/urandom of="$MOUNT_POINT/test-video-$i.mp4" bs=1M count=50 2>/dev/null
done

sync

echo "📸 Filesystem contents:"
sudo du -sh "$MOUNT_POINT"

# Delete files for recovery testing
echo "🗑️  Deleting files for recovery testing..."
sudo find "$MOUNT_POINT" -type f | head -10 | while read f; do
    echo "  Deleting: $(basename "$f")"
    sudo rm -f "$f"
done

sync

# Unmount
echo "💿 Unmounting..."
sudo umount "$MOUNT_POINT"
rmdir "$MOUNT_POINT"

echo ""
echo "✅ XFS test image created: $IMAGE_FILE"
echo "📊 Size: $(ls -lh $IMAGE_FILE | awk '{print $5}')"
echo ""
echo "Transfer this file to your Mac and test with:"
echo "   cargo run -p ghostfs-cli -- scan $IMAGE_FILE --fs xfs"
