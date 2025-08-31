#!/bin/bash
# Create simple test file system images with embedded real data

set -e

echo "💾 Creating simplified test data with embedded files..."

TEST_DIR="test-data"
mkdir -p "$TEST_DIR"

# Create simple test files
create_test_content() {
    echo "📝 Creating test file content..."
    
    # Create a simple text document
    cat > /tmp/test_document.txt << 'EOF'
This is a recovered test document.
It contains real data for XFS recovery testing.
Line 3: Testing file recovery functionality.
Line 4: Unicode test ñáéíóú αβγδε 日本語
Line 5: Numbers and symbols: 1234567890 !@#$%^&*()
EOF

    # Create a config file
    cat > /tmp/test_config.ini << 'EOF'
[Settings]
version=2.0
app_name=GhostFS
debug=true

[Database]
host=localhost
port=5432
name=recovery_test
EOF

    # Create JSON data
    cat > /tmp/test_data.json << 'EOF'
{
  "users": [
    {"id": 1, "name": "John Doe", "active": true},
    {"id": 2, "name": "Jane Smith", "active": false}
  ],
  "config": {
    "theme": "dark",
    "auto_save": true
  }
}
EOF
}

# Create a simple XFS test image with embedded data
create_simple_xfs_image() {
    local filename="$1"
    local size_mb="$2"
    
    echo "🔨 Creating simple XFS test image: $filename (${size_mb}MB)"
    
    # Create base image
    dd if=/dev/zero of="$TEST_DIR/$filename" bs=1M count="$size_mb" 2>/dev/null
    
    # Write XFS superblock signature at the beginning
    printf "XFSB" | dd of="$TEST_DIR/$filename" bs=1 seek=0 conv=notrunc 2>/dev/null
    
    # Write block size (4096) at offset 4
    printf "\x00\x00\x10\x00" | dd of="$TEST_DIR/$filename" bs=1 seek=4 conv=notrunc 2>/dev/null
    
    # Embed test files at predictable block locations (4KB blocks)
    # Block 1 (offset 0x1000 = 4096): document.txt
    dd if=/tmp/test_document.txt of="$TEST_DIR/$filename" bs=1 seek=4096 conv=notrunc 2>/dev/null
    
    # Block 2 (offset 0x2000 = 8192): config.ini  
    dd if=/tmp/test_config.ini of="$TEST_DIR/$filename" bs=1 seek=8192 conv=notrunc 2>/dev/null
    
    # Block 3 (offset 0x3000 = 12288): data.json
    dd if=/tmp/test_data.json of="$TEST_DIR/$filename" bs=1 seek=12288 conv=notrunc 2>/dev/null
    
    echo "✅ Created simplified XFS image with real data at blocks 1, 2, and 3"
}

# Create test content
create_test_content

# Create simplified XFS test image
create_simple_xfs_image "test-xfs.img" 10

# Create simple additional test files
echo "Creating additional test files..."
echo "Binary test data: $(date)" > "$TEST_DIR/sample-text.txt"
printf "\x89PNG\x0d\x0a\x1a\x0a\x00\x00\x00\x0d" > "$TEST_DIR/sample-binary.bin"

# Clean up temporary files
rm -f /tmp/test_document.txt /tmp/test_config.ini /tmp/test_data.json

echo "📁 Simplified test data created in $TEST_DIR/"
echo "Contents:"
ls -la "$TEST_DIR/"

echo ""
echo "📋 Test data locations:"
echo "  Block 1 (0x1000): document.txt content"
echo "  Block 2 (0x2000): config.ini content" 
echo "  Block 3 (0x3000): data.json content"
