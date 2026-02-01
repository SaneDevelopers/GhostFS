# Testing the Interactive Scan Prompt Feature

## Current Status ✅
- Adaptive scanning works perfectly
- Small filesystem (10MB test-xfs.img) scans 100% of blocks
- Found and can recover files successfully

## To Test Interactive Prompt (>100GB filesystems)

### Option 1: Use Linux/WSL (Recommended)
```bash
# On Linux or WSL
cd /path/to/ghostfs
chmod +x scripts/create-test-xfs-linux.sh
./scripts/create-test-xfs-linux.sh

# Transfer the created .img file to Mac
# Then test on Mac:
cargo run -p ghostfs-cli -- scan large-xfs-test.img --fs xfs
```

### Option 2: Manually Test With Existing Small Image
For development testing, you can temporarily lower the threshold:

In `crates/ghostfs-cli/src/main.rs`, change line ~148:
```rust
// FROM:
if interactive && total_size_gb > 100.0 {

// TO (for testing):
if interactive && total_size_gb > 0.001 {  // Triggers on >1MB
```

Then run:
```bash
cargo run -p ghostfs-cli -- scan test-data/test-xfs.img --fs xfs
```

You'll see the interactive prompt even on the small test image.

### Option 3: Create 150GB Sparse Image (macOS can read XFS)
If you have an XFS image created on Linux, macOS can still READ it for testing:

1. On Linux: Create a 150GB image with `create-test-xfs-linux.sh`
2. Transfer to Mac via USB/network
3. Test scanning - the prompt will appear!

## What the Prompt Looks Like

```
⚠️  Large filesystem detected: 150.25 GB (39424000 blocks)
   Scanning all blocks may take considerable time.

📊 Scan options:
   • Type 'all' or '100%' to scan entire filesystem (thorough but slow)
   • Type a percentage: e.g., '10%' to scan 10% of blocks
   • Type storage size: e.g., '50GB', '500MB', '1TB'
   • Press Enter for smart adaptive scan (recommended)

🔍 How much do you want to scan? [adaptive]: _
```

**Test Inputs:**
- `10%` → Scans 10% (15GB)
- `50GB` → Scans first 50GB
- `all` → Scans all 150GB
- `[Enter]` → Smart adaptive (1% = ~1.5GB for 150GB FS)

## Current Test Results

With the 10MB test image:
- ✅ Adaptive algorithm works (scanned 100%)
- ✅ Found 2 recoverable files
- ✅ Scanning completes successfully
- 🔄 No prompt shown (< 100GB threshold)
