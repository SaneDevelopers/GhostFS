# GhostFS Timeline Feature - Quick Start Guide

## What is the Timeline Feature?

The Timeline Recovery feature analyzes deleted files to create a chronological timeline of file system activity. It helps you understand:
- **When** files were created, modified, and deleted
- **What patterns** exist in deletion behavior
- **Which files** were affected by suspicious activity

Perfect for:
- 🔍 **Forensic Analysis**: Investigate data loss incidents
- 🛡️ **Security Audits**: Detect ransomware or malicious deletion
- 📊 **Data Recovery**: Prioritize which files to recover first
- 📈 **System Monitoring**: Understand file lifecycle patterns

## Installation

Once the Windows build toolchain is configured (see build errors), compile GhostFS:

```bash
cd /path/to/GhostFS
cargo build --release
```

The `ghostfs` binary will be in `target/release/ghostfs.exe`

## Basic Usage

### Simple Timeline Generation

```bash
# Generate timeline from a disk image
ghostfs timeline disk.img --fs xfs
```

This will:
1. Scan the disk image for deleted files
2. Extract all timestamp events
3. Detect suspicious patterns
4. Display a formatted report

### Specify Filesystem Type

```bash
# XFS filesystem (default)
ghostfs timeline /dev/sdb1.img --fs xfs

# Btrfs filesystem
ghostfs timeline backup.img --fs btrfs

# exFAT filesystem
ghostfs timeline usb_drive.img --fs exfat
```

## Export Options

### Export to JSON (Machine-Readable)

```bash
ghostfs timeline disk.img --fs xfs --json timeline.json
```

The JSON file contains complete timeline data for programmatic analysis:
```json
{
  "events": [
    {
      "timestamp": "2026-02-15T14:30:01Z",
      "event_type": "FileDeleted",
      "file_id": 1,
      "description": "Deleted: photo.jpg (2048 bytes, 85% confidence)"
    }
  ],
  "patterns": [...],
  "statistics": {...}
}
```

### Export to CSV (Spreadsheet-Compatible)

```bash
ghostfs timeline disk.img --fs xfs --csv timeline.csv
```

Import the CSV into Excel, Google Sheets, or any data analysis tool.

### Export Both Formats

```bash
ghostfs timeline disk.img --fs xfs --json data.json --csv data.csv
```

## Understanding the Output

### Statistics Section

```
📊 STATISTICS
───────────────────────────────────────────────────────
Total events: 126
Deletion events: 42
Avg deletions/day: 8.4
Peak deletion time: 2026-02-15 14:32:00
```

- **Total events**: All file system events (create, modify, delete)
- **Deletion events**: Only deletion events
- **Avg deletions/day**: Average rate of file deletions
- **Peak deletion time**: When most deletions occurred (helps identify incidents)

### File Types Affected

```
📁 FILE TYPES AFFECTED
───────────────────────────────────────────────────────
  15 x image/jpeg
  12 x application/pdf
  8 x text/plain
```

Shows which file types were deleted. Useful for:
- Identifying targeted attacks (e.g., only documents deleted)
- Understanding what data was lost
- Prioritizing recovery efforts

### Suspicious Patterns

```
⚠️  SUSPICIOUS PATTERNS DETECTED
───────────────────────────────────────────────────────

1. BulkDeletion (Confidence: 90%)
   15 files deleted within 5 minutes starting at 2026-02-15 14:30:00
   Timeframe: 2026-02-15 14:30:00 to 2026-02-15 14:34:52
   Affected files: 15 files
```

**Pattern Types:**

1. **BulkDeletion** (90% confidence)
   - 5+ files deleted within 5 minutes
   - Indicates: Ransomware, accidental mass deletion, or intentional purge
   - Action: High priority for investigation

2. **SelectiveDeletion** (70% confidence)
   - Multiple files of same type deleted
   - Indicates: Targeted cleanup or attack
   - Action: Review if deletion was intentional

### Event Timeline

```
📅 EVENT TIMELINE
───────────────────────────────────────────────────────
2026-02-15 14:30:01 🗑️  - Deleted: photo1.jpg (2048 bytes, 85% confidence)
2026-02-15 14:30:15 🗑️  - Deleted: photo2.jpg (1536 bytes, 90% confidence)
2026-02-15 14:32:10 ✏️  - Modified: document.pdf
2026-02-15 14:35:22 📝 - Created: notes.txt
```

**Event Icons:**
- 📝 = File Created
- ✏️ = File Modified
- 🗑️ = File Deleted
- ✅ = File Recovered (future feature)

## Real-World Workflows

### Scenario 1: Investigating Ransomware

```bash
# 1. Create disk image (on Linux/macOS)
sudo dd if=/dev/sdb of=infected_disk.img bs=4M

# 2. Generate timeline
ghostfs timeline infected_disk.img --fs xfs --json forensic.json

# 3. Review output for bulk deletion patterns
# Look for: High-confidence bulk deletions at specific time

# 4. Recover files
ghostfs recover infected_disk.img --fs xfs --out /recovered
```

**What to look for:**
- Bulk deletion patterns with 90% confidence
- All deletions within a short time window (minutes)
- Specific file types targeted (documents, images)

### Scenario 2: Accidental File Loss Recovery

```bash
# 1. Timeline analysis
ghostfs timeline backup.img --fs btrfs --csv timeline.csv

# 2. Open CSV in Excel
# Filter by: deletion_time around when data was lost

# 3. Identify file IDs from CSV

# 4. Recover specific files
ghostfs recover backup.img --fs btrfs --ids 42,43,44 --out /recovered
```

### Scenario 3: Security Audit

```bash
# 1. Generate timeline with exports
ghostfs timeline /dev/sdb1.img --fs xfs \
  --json audit_$(date +%Y%m%d).json \
  --csv audit_$(date +%Y%m%d).csv

# 2. Archive for compliance
tar -czf audit_package.tar.gz audit_*.json audit_*.csv

# 3. Review patterns monthly for anomalies
```

## Integration with Other GhostFS Commands

### Complete Recovery Workflow

```bash
# Step 1: Scan filesystem
ghostfs scan disk.img --fs xfs

# Step 2: Generate timeline for analysis
ghostfs timeline disk.img --fs xfs --json analysis.json

# Step 3: Review timeline output (in console)
# Identify suspicious patterns and high-confidence files

# Step 4: Recover files
ghostfs recover disk.img --fs xfs --out /recovered

# Step 5: Verify recovered files
ls -lh /recovered
```

### Filesystem Detection

```bash
# If you don't know the filesystem type
ghostfs detect unknown.img

# Use detected type in timeline
ghostfs timeline unknown.img --fs <detected_type>
```

## Advanced Usage

### Analyzing Large Filesystems

For very large filesystems (>100GB), timeline generation may take time. The command provides progress:

```bash
ghostfs timeline large_disk.img --fs xfs --json timeline.json

# Output:
# 📅 Generating Recovery Timeline...
# 🔍 Scanning xfs filesystem...
# ✅ Scan complete: 12,453 files found
# ...
```

### Filtering by Confidence (Future)

Once sessions are persisted (Phase 5), you'll be able to filter:

```bash
# Future feature
ghostfs timeline disk.img --min-confidence 0.8
```

### Combining Multiple Images (Future)

```bash
# Future feature for RAID analysis
ghostfs timeline disk1.img disk2.img disk3.img --raid5
```

## Troubleshooting

### "No events found"

**Cause**: No deleted files with timestamps detected

**Solutions:**
- Verify correct filesystem type (`--fs` parameter)
- Check if filesystem has been overwritten
- Try different filesystem type
- Use `ghostfs scan` first to see raw results

### "Pattern detection seems wrong"

**Cause**: Pattern thresholds may not match your use case

**Explanation:**
- BulkDeletion: Requires 5+ files in 5 minutes
- SelectiveDeletion: Requires 3+ files of same type

These are conservative thresholds to reduce false positives.

### Build Errors

If you see linker errors:

**Windows**:
```
Install Visual Studio Build Tools:
https://visualstudio.microsoft.com/downloads/
Select: "Desktop development with C++"
```

**Linux**:
```bash
sudo apt-get install build-essential
```

**macOS**:
```bash
xcode-select --install
```

## Tips & Best Practices

### 1. Always Work with Images
```bash
# Create image first (Linux/macOS)
sudo dd if=/dev/sdb of=disk.img bs=4M status=progress

# Then analyze
ghostfs timeline disk.img --fs xfs
```

Never run directly on live disks - use images to prevent data corruption.

### 2. Export for Documentation
```bash
# Always export timeline for records
ghostfs timeline disk.img --fs xfs \
  --json "case_${CASE_ID}_timeline.json" \
  --csv "case_${CASE_ID}_timeline.csv"
```

### 3. Combine with Recovery
```bash
# Generate timeline first to understand scope
ghostfs timeline disk.img --fs xfs

# Then recover based on insights
ghostfs recover disk.img --fs xfs --out /recovered
```

### 4. Check Patterns First
Look for high-confidence (90%) bulk deletion patterns - these usually indicate:
- Ransomware attacks
- Accidental `rm -rf` commands
- Script errors
- Malicious activity

## What's Next?

### Planned Features (Phase 5)

1. **Session Persistence**
   - Save scan results to database
   - Generate timeline without rescanning
   - Faster analysis

2. **More Patterns**
   - Periodic deletions (cron jobs)
   - Time-of-day analysis
   - Suspicious activity detection

3. **Visualization**
   - HTML timeline viewer
   - Interactive charts
   - Heat maps

4. **Forensic Mode**
   - Cryptographic hashing
   - Chain-of-custody tracking
   - Legal evidence packages

## Support & Contributing

- **Issues**: GitHub Issues
- **Documentation**: `/docs` directory
- **Examples**: `/examples` directory

---

## Quick Reference Card

```bash
# Basic timeline
ghostfs timeline <image> --fs <xfs|btrfs|exfat>

# With JSON export
ghostfs timeline <image> --fs <fs_type> --json <output.json>

# With CSV export
ghostfs timeline <image> --fs <fs_type> --csv <output.csv>

# Both exports
ghostfs timeline <image> --fs <fs_type> --json out.json --csv out.csv

# Example
ghostfs timeline /dev/sdb1.img --fs xfs --json timeline.json
```

## FAQ

**Q: Do I need to run `scan` before `timeline`?**
A: No, `timeline` automatically scans the filesystem.

**Q: Can I run timeline on a live disk?**
A: Technically yes, but **strongly discouraged**. Always use disk images.

**Q: What's the difference between JSON and CSV export?**
A: JSON is complete and machine-readable. CSV is simplified for spreadsheets.

**Q: How accurate are the patterns?**
A: BulkDeletion is 90% confident, SelectiveDeletion is 70% confident.

**Q: Can I customize pattern detection?**
A: Not yet - this is planned for future releases.

**Q: Does timeline modify my disk?**
A: No, all operations are read-only.

---

**Ready to analyze your timeline? Run:**
```bash
ghostfs timeline --help
```
