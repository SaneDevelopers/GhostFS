#!/bin/bash
# Create a realistic XFS test image with Downloads folder data

set -e

# Configuration
IMAGE_FILE="test-data/large-xfs-test.img"
IMAGE_SIZE="3G"  # 3GB image
MOUNT_POINT="/tmp/ghostfs-test-mount"
DOWNLOADS_DIR="$HOME/Downloads"

echo "🔧 Creating realistic XFS test image with Downloads data..."

# Create directory if doesn't exist
mkdir -p test-data

# Check if image already exists
if [ -f "$IMAGE_FILE" ]; then
    echo "⚠️  Image already exists at $IMAGE_FILE"
    read -p "Delete and recreate? (y/n): " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        rm -f "$IMAGE_FILE"
    else
        echo "❌ Aborted"
        exit 1
    fi
fi

# Create image file (not sparse - actual 3GB)
echo "📝 Creating $IMAGE_SIZE image file..."
dd if=/dev/zero of="$IMAGE_FILE" bs=1M count=3072 status=progress 2>&1 | grep -v records

# Format as XFS
echo "💾 Formatting as XFS..."
if ! command -v mkfs.xfs &> /dev/null; then
    echo "❌ mkfs.xfs not found. On macOS, XFS is not natively supported."
    echo "   You can:"
    echo "   1. Use Linux/WSL to create the image"
    echo "   2. Use a pre-made XFS image"
    echo "   3. Install xfsprogs via brew (may not work on newer macOS)"
    rm -f "$IMAGE_FILE"
    exit 1
fi
mkfs.xfs -f "$IMAGE_FILE" 2>&1 || { echo "❌ Failed to format XFS"; rm -f "$IMAGE_FILE"; exit 1; }

# Mount the image
echo "📁 Mounting filesystem..."
mkdir -p "$MOUNT_POINT"
sudo mount -o loop "$IMAGE_FILE" "$MOUNT_POINT"

# Copy Downloads folder
if [ -d "$DOWNLOADS_DIR" ]; then
    echo "📦 Copying Downloads folder contents..."
    sudo cp -r "$DOWNLOADS_DIR"/* "$MOUNT_POINT/" 2>/dev/null || echo "  ⚠️  Some files may have been skipped"
    echo "✅ Downloads copied"
else
    echo "⚠️  Downloads folder not found, creating sample data instead..."
    sudo mkdir -p "$MOUNT_POINT/documents"
    echo "Sample document" | sudo tee "$MOUNT_POINT/documents/sample.txt" > /dev/null
fi

# Add extra video files if they exist in common locations
echo "🎬 Looking for video files to add..."
VIDEO_COUNT=0
for video_path in "$HOME/Movies"/*.mp4 "$HOME/Movies"/*.mov "$HOME/Desktop"/*.mp4 "$DOWNLOADS_DIR"/*.mp4; do
    if [ -f "$video_path" ] && [ $VIDEO_COUNT -lt 2 ]; then
        echo "  Adding: $(basename "$video_path")"
        sudo cp "$video_path" "$MOUNT_POINT/" 2>/dev/null && VIDEO_COUNT=$((VIDEO_COUNT + 1))
    fi
done

if [ $VIDEO_COUNT -eq 0 ]; then
    echo "  ℹ️  No video files found, creating test video placeholders..."
    sudo dd if=/dev/urandom of="$MOUNT_POINT/test-video-1.mp4" bs=1M count=50 2>/dev/null
    # Show what we have
fi
echo ""
echo "📸 Current filesystem contents:"
sudo du -sh "$MOUNT_POINT"
sudo find "$MOUNT_POINT" -type f | wc -l | xargs echo "  Total files:"

# Delete some files for recovery testing
echo ""
echo "🗑️  Deleting random files for recovery testing..."
# Delete first 5 PDF files found
sudo find "$MOUNT_POINT" -name "*.pdf" -type f | head -5 | while read file; do
    echo "  Deleting: $(basename "$file")"
    sudo rm -f "$file"
done

# Delete first 3 images
sudo f"
echo "📋 Remaining files after deletion:"
sudo find "$MOUNT_POINT" -type f | wc -l | xargs echo "  Files remaining:"

# Unmount
echo ""
echo "💿 Unmounting..."
sudo umount "$MOUNT_POINT"
rmdir "$MOUNT_POINT"

# Show file info
echo ""
echo "✅ Realistic XFS test image created!"
echo "📍 Location: $IMAGE_FILE"
echo "📊 Image size: $(ls -lh $IMAGE_FILE | awk '{print $5}')"
echo "💽 Disk usage: $(du -h $IMAGE_FILE | cut -f1)"
echo ""
echo "🧪 To test scanning:"
echo "   cargo run -p ghostfs-cli -- scan $IMAGE_FILE --fs xfs --info"
echo ""
echo "🔍 To test recovery:"
echo "   mkdir recovered-test"
echo "   cargo run -p ghostfs-cli -- recover $IMAGE_FILE --fs xfs --out recovered-test
# Delete some files
echo ""
echo "🗑️  Deleting test files..."
sudo rm -f "$MOUNT_POINT/test-text.txt"
sudo rm -f "$MOUNT_POINT/test-data.json"
sudo rm -f "$MOUNT_POINT/documents/important.txt"
sudo rm -f "$MOUNT_POINT/random-1mb.bin"

sync

echo "📋 Files after deletion:"
sudo ls -lh "$MOUNT_POINT" 2>/dev/null || echo "  (some directories may be empty)"

# Unmount
echo ""
echo "💿 Unmounting..."
sudo umount "$MOUNT_POINT"
rmdir "$MOUNT_POINT"

# Show file info
echo ""
echo "✅ Large XFS test image created!"
echo "📍 Location: $IMAGE_FILE"
echo "📊 Size: $(ls -lh $IMAGE_FILE | awk '{print $5}')"
echo "💽 Actual disk usage: $(du -h $IMAGE_FILE | cut -f1)"
echo ""
echo "🧪 To test with ghostfs:"
echo "   cargo run -p ghostfs-cli -- scan $IMAGE_FILE --fs xfs"
echo "   (Should trigger interactive prompt for large filesystem)"
