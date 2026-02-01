# GhostFS 👻

**Professional data recovery tool for XFS, Btrfs, and exFAT file systems**

[![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey.svg)](https://github.com/your-org/ghostfs)

GhostFS is a modern, professional-grade data recovery solution designed for forensics professionals, IT administrators, and advanced users. It specializes in recovering deleted files from XFS, Btrfs, and exFAT file systems with advanced confidence scoring and forensics capabilities.

## ✨ Features

### 🔍 **Advanced File System Support**
- **XFS**: Allocation group scanning, B+tree reconstruction, extended attributes
- **Btrfs**: Snapshot-based recovery, COW exploitation, checksum validation
- **exFAT**: FAT reconstruction, UTF-16 filenames, large file support (>4GB)

### 🧠 **Intelligent Recovery Engine**
- **Confidence Scoring**: AI-powered 0-100% confidence ratings
- **File Signature Analysis**: 50+ file types with validation
- **Timeline Reconstruction**: Deletion pattern analysis
- **Memory-Safe**: Zero-copy operations with Rust

### 🛠️ **Professional Interface**
- **CLI Tool**: Powerful command-line interface for automation
- **Session Management**: SQLite-based recovery sessions
- **Forensics Mode**: Chain of custody and evidence integrity
- **Batch Operations**: Process multiple devices efficiently

### 📊 **Forensics Capabilities**
- **Timeline Analysis**: When and how files were deleted
- **Pattern Detection**: Suspicious deletion patterns
- **Evidence Export**: Tamper-proof evidence packages
- **Audit Trail**: Complete operation logging

## 🚀 Quick Start

### Installation

**Option 1: Pre-built Binaries** (Recommended)
```bash
# Download latest release
curl -L https://github.com/your-org/ghostfs/releases/latest/download/ghostfs-linux.tar.gz | tar xz
sudo mv ghostfs /usr/local/bin/

# Or for macOS
brew install ghostfs
```

**Option 2: Build from Source**
```bash
git clone https://github.com/your-org/ghostfs.git
cd ghostfs
cargo build --release
sudo cp target/release/ghostfs-cli /usr/local/bin/ghostfs
```

### Basic Usage

**1. Detect File System Type**
```bash
ghostfs detect /dev/sdb1
# Output: ✅ Detected: XFS

ghostfs detect /path/to/btrfs-image.img
# Output: ✅ Detected: Btrfs

ghostfs detect /mnt/usb/exfat-drive.img
# Output: ✅ Detected: exFAT
```

**2. Scan for Deleted Files**
```bash
# XFS scan with default settings
ghostfs scan /dev/sdb1 --fs xfs

# Btrfs scan with custom confidence threshold
ghostfs scan /path/to/btrfs.img --fs btrfs --confidence 0.3

# exFAT scan saving to specific session file
ghostfs scan /dev/sdc1 --fs exfat --output exfat_recovery.db
```

**3. List Recovered Files**
```bash
# Show all files with confidence scores
ghostfs list

# Filter by confidence (show only high-confidence files)
ghostfs list --min-confidence 0.8

# Sort by file size
ghostfs list --sort size
```

**4. Recover Files**
```bash
# Recover all files above threshold
ghostfs recover --output-dir ./recovered

# Recover specific files by ID
ghostfs recover --files 1,5,10 --output-dir ./recovered
```

## 📖 Documentation

### Command Reference

#### `ghostfs detect <device>`
Identifies the file system type of a device or image file.

**Examples:**
```bash
ghostfs detect /dev/sdb1                    # Block device
ghostfs detect /path/to/filesystem.img      # Image file  
ghostfs detect /mnt/usb/disk.img            # Mounted image
```

#### `ghostfs scan <device> --fs <type> [options]`
Scans for deleted files and creates a recovery session.

**Required Arguments:**
- `<device>`: Path to device or image file
- `--fs <type>`: File system type (xfs, btrfs, exfat)

**Options:**
- `--confidence <0.0-1.0>`: Minimum confidence threshold (default: 0.5)
- `--output <file>`: Save session to specific file (default: auto-generated)
- `--forensics`: Enable forensics mode with additional analysis
- `--verbose`: Show detailed progress information

**Examples:**
```bash
# Basic XFS scan
ghostfs scan /dev/sdb1 --fs xfs

# Btrfs scan with low confidence threshold for maximum recovery
ghostfs scan /path/to/btrfs.img --fs btrfs --confidence 0.2

# exFAT forensics scan with detailed logging
ghostfs scan /dev/sdc1 --fs exfat --forensics --verbose

# Using cargo run for development
cargo run -p ghostfs-cli -- scan test-data/test-xfs.img
cargo run -p ghostfs-cli -- scan test-data/test-btrfs.img  
cargo run -p ghostfs-cli -- scan test-data/test-exfat.img
```

#### `ghostfs list [options]`
Lists files from the most recent or specified recovery session.

**Options:**
- `--session <file>`: Use specific session file
- `--min-confidence <0.0-1.0>`: Filter by minimum confidence
- `--sort <field>`: Sort by: size, confidence, deletion-time, name
- `--format <type>`: Output format: table (default), json, csv
- `--filter <pattern>`: Filter by filename pattern

**Examples:**
```bash
# List all files from latest session
ghostfs list

# High-confidence files only
ghostfs list --min-confidence 0.8

# Export to JSON for analysis
ghostfs list --format json --output files.json

# Filter specific file types
ghostfs list --filter "*.jpg,*.png,*.gif"
```

#### `ghostfs recover [options]`
Recovers files from a recovery session.

**Options:**
- `--session <file>`: Use specific session file
- `--output-dir <dir>`: Recovery output directory (required)
- `--files <ids>`: Comma-separated file IDs to recover
- `--min-confidence <0.0-1.0>`: Minimum confidence for recovery
- `--preserve-paths`: Recreate original directory structure
- `--verify`: Verify recovered files with checksums

**Examples:**
```bash
# Recover all files above default threshold
ghostfs recover --output-dir ./recovered

# Recover specific files
ghostfs recover --files 1,5,10,15 --output-dir ./important

# High-confidence files with path preservation
ghostfs recover --min-confidence 0.9 --preserve-paths --output-dir ./recovered
```

#### `ghostfs timeline [options]`
Generates deletion timeline analysis.

**Options:**
- `--session <file>`: Use specific session file
- `--format <type>`: Output format: text (default), json, csv
- `--output <file>`: Save timeline to file
- `--date-range <range>`: Filter by date range (e.g., "2024-01-01 to 2024-12-31")

**Examples:**
```bash
# Text timeline
ghostfs timeline

# JSON export for analysis
ghostfs timeline --format json --output timeline.json

# Specific date range
ghostfs timeline --date-range "2024-06-01 to 2024-06-30"
```

## 🛠️ Building from Source

### Prerequisites

**System Requirements:**
- Rust 1.70 or later
- SQLite 3.x development libraries
- Platform-specific build tools

**Platform Setup:**

**macOS:**
```bash
# Install Xcode Command Line Tools
xcode-select --install

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

**Linux (Ubuntu/Debian):**
```bash
# Install dependencies
sudo apt update
sudo apt install build-essential pkg-config libsqlite3-dev

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

**Windows:**
```powershell
# Install Visual Studio Build Tools
# Download from: https://visualstudio.microsoft.com/visual-cpp-build-tools/

# Install Rust
# Download from: https://rustup.rs/
```

### Build Process

**1. Clone Repository:**
```bash
git clone https://github.com/your-org/ghostfs.git
cd ghostfs
```

**2. Development Build:**
```bash
# Build all components
cargo build

# Build only CLI tool
cargo build -p ghostfs-cli

# Build with debug info
cargo build --profile dev
```

**3. Release Build:**
```bash
# Optimized release build
cargo build --release

# Release with debug symbols (for troubleshooting)
cargo build --release --config 'profile.release.debug=true'
```

**4. Run Tests:**
```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Test specific component
cargo test -p ghostfs-core
```

**5. Create Test Data:**
```bash
# Generate test filesystem images
./scripts/create-test-data.sh

# Test detection
cargo run -p ghostfs-cli -- detect test-data/test-xfs.img
```

### Development Tools

**Code Quality:**
```bash
# Format code
cargo fmt

# Lint code
cargo clippy -- -D warnings

# Security audit
cargo audit  # (requires: cargo install cargo-audit)
```

**VS Code Setup:**
The project includes VS Code configuration with:
- Rust-analyzer integration
- Build and test tasks
- Debug configurations
- Recommended extensions

## 📊 Performance & Compatibility

### Scan Performance
| File System | Metadata Scan | Deep Scan | Typical Use Case |
|-------------|---------------|-----------|------------------|
| **XFS** | ~100 MB/s | ~50 MB/s | Linux servers, NAS |
| **Btrfs** | ~80 MB/s | ~40 MB/s | Linux desktops, snapshots |
| **exFAT** | ~150 MB/s | ~75 MB/s | USB drives, SD cards |

### Memory Usage
- **Base CLI**: ~50 MB
- **Per 1000 files**: ~1 MB metadata
- **Memory-mapped**: No additional RAM for file content
- **Database**: ~100 KB per 1000 files

### Platform Support
- ✅ **Linux**: All distributions (x86_64, ARM64)
- ✅ **macOS**: Intel and Apple Silicon
- ✅ **Windows**: Windows 10+ (x86_64)
- ✅ **FreeBSD**: Limited testing

## 🔧 Configuration

### Environment Variables
```bash
# Enable debug logging
export RUST_LOG=debug

# Custom session directory
export GHOSTFS_SESSION_DIR=/path/to/sessions

# Memory limit for large files
export GHOSTFS_MEMORY_LIMIT=2GB
```

### Configuration File
Create `~/.config/ghostfs/config.toml`:
```toml
[recovery]
default_confidence = 0.5
max_file_size = "1GB"
session_directory = "~/.local/share/ghostfs/sessions"

[logging]
level = "info"
file = "~/.local/share/ghostfs/logs/ghostfs.log"

[forensics]
enable_audit_trail = true
hash_algorithm = "SHA256"
```

## 🐛 Troubleshooting

### Common Issues

**Permission Denied:**
```bash
# Linux/macOS: Run with sudo for device access
sudo ghostfs scan /dev/sdb1 --fs xfs

# Or add user to disk group (Linux)
sudo usermod -a -G disk $USER
# Then log out and back in
```

**Out of Memory:**
```bash
# For large files, increase system limits
ulimit -v unlimited

# Or use streaming mode (future feature)
ghostfs scan --streaming /path/to/large.img
```

**File System Not Detected:**
```bash
# Try manual file system specification
ghostfs scan /dev/sdb1 --fs xfs --force

# Check file system integrity first
sudo fsck -n /dev/sdb1
```

### Debug Information

**Enable Verbose Logging:**
```bash
# Temporary debug mode
RUST_LOG=debug ghostfs scan /dev/sdb1 --fs xfs

# Or set environment permanently
export RUST_LOG=ghostfs=trace
```

**Generate Debug Report:**
```bash
# System information
ghostfs --version
uname -a
lsblk  # Linux
diskutil list  # macOS

# Test basic functionality
ghostfs detect test-data/test-xfs.img
```

## 📄 License & Legal

### License
GhostFS is dual-licensed:
- **MIT License** for open-source use
- **Commercial License** for proprietary applications

### Legal Considerations
- **Forensics Use**: Maintains chain of custody standards
- **Data Privacy**: All processing is local, no data transmission
- **Evidence Integrity**: Cryptographic validation of recovered files
- **Compliance**: Meets requirements for legal evidence

### Disclaimer
This software is for legitimate data recovery and forensics purposes only. Users are responsible for compliance with local laws and regulations regarding data access and recovery.

## 🤝 Contributing

We welcome contributions! Please see our [Contributing Guidelines](CONTRIBUTING.md) for details.

### Development Process
1. Fork the repository
2. Create a feature branch
3. Make your changes with tests
4. Submit a pull request

### Areas We Need Help
- Additional file system support (NTFS, ext4, ZFS)
- GUI application development
- Performance optimizations
- Documentation improvements
- Testing on various platforms

## 📞 Support

### Community Support
- **GitHub Issues**: Bug reports and feature requests
- **Discussions**: Questions and community help
- **Wiki**: Additional documentation and guides

### Professional Support
- **Email**: support@ghostfs.com
- **Enterprise**: Custom development and integration
- **Training**: Forensics workshops and certification

---

**GhostFS** - *Bringing deleted files back from the digital afterlife* 👻

For detailed technical information, see [docs/INFO.md](docs/INFO.md)