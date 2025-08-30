#!/bin/bash
# Setup development environment for GhostFS

set -e

echo "🚀 Setting up GhostFS development environment..."

# Check if Rust is installed
if ! command -v cargo &> /dev/null; then
    echo "❌ Rust is not installed. Please install Rust first:"
    echo "   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

echo "✅ Rust is installed: $(rustc --version)"

# Install development tools
echo "📦 Installing Rust development tools..."
cargo install cargo-watch || echo "cargo-watch already installed"
cargo install cargo-audit || echo "cargo-audit already installed"
cargo install cargo-outdated || echo "cargo-outdated already installed"

# Create test data directory
echo "📁 Creating test data directory..."
mkdir -p test-data

# Build the project
echo "🔨 Building project..."
cargo build

# Run tests
echo "🧪 Running tests..."
cargo test

# Create sample test images (small ones for initial development)
echo "💾 Creating sample test images..."

# Create small empty files for testing file system detection
dd if=/dev/zero of=test-data/sample-xfs.img bs=1M count=10 2>/dev/null || true
dd if=/dev/zero of=test-data/sample-btrfs.img bs=1M count=10 2>/dev/null || true
dd if=/dev/zero of=test-data/sample-exfat.img bs=1M count=10 2>/dev/null || true

echo "✨ Development environment setup complete!"
echo ""
echo "🔧 Quick start commands:"
echo "  cargo build                    # Build all crates"
echo "  cargo test                     # Run tests"
echo "  cargo run -p ghostfs-cli -- --help  # Show CLI help"
echo "  cargo watch -x test            # Continuous testing"
echo ""
echo "📖 See docs/DEVELOPMENT.md for detailed development guide"
