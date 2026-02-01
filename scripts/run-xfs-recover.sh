#!/usr/bin/env zsh
# Interactive helper script to scan and recover from an image using ghostfs-cli
# Usage: ./scripts/run-xfs-recover.sh
# The script will prompt for an image path and an output directory.

set -euo pipefail

echo "GhostFS interactive recover script"
read -r "IMAGE_PATH?Enter the path to the image file: "
if [[ ! -f "$IMAGE_PATH" ]]; then
  echo "Error: image file not found: $IMAGE_PATH"
  exit 1
fi

read -r "OUT_DIR?Enter the output directory for recovered files (will be created): "
OUT_DIR=${OUT_DIR:-recovered/}

# Ask for optional confidence threshold
read -r "CONFIDENCE?Enter minimum confidence (0.0-1.0) [default: 0.5]: "
CONFIDENCE=${CONFIDENCE:-0.5}

# Optional: ask for verbose logging
if ! read -r "VERBOSE?Enable debug logs? (y/N): "; then
  # If read fails (EOF when piping), default to N
  VERBOSE=${VERBOSE:-N}
fi
if [[ "$VERBOSE" = "y" || "$VERBOSE" = "Y" ]]; then
  export RUST_LOG=debug
else
  export RUST_LOG=info
fi

# Ensure output dir exists
mkdir -p "$OUT_DIR"

# Run scan and capture output
echo "\nScanning image: $IMAGE_PATH"
cargo run -p ghostfs-cli -- scan "$IMAGE_PATH" | tee "$OUT_DIR/scan.log"

# Show found count and ask to proceed
FOUND=$(grep -Eo "Recovery complete: [0-9]+ files found" "$OUT_DIR/scan.log" | awk '{print $3}')
FOUND=${FOUND:-0}

echo "Found files: $FOUND"
if ! read -r "PROCEED?Proceed to recover the found files? (Y/n): "; then
  # If read fails (EOF when piping), assume default YES
  PROCEED=${PROCEED:-Y}
fi
PROCEED=${PROCEED:-Y}

if [[ "$PROCEED" = "n" || "$PROCEED" = "N" ]]; then
  echo "Recovery cancelled. Scan log saved to $OUT_DIR/scan.log"
  exit 0
fi

# Run recover with chosen confidence
echo "Recovering files to: $OUT_DIR (confidence: $CONFIDENCE)"
cargo run -p ghostfs-cli -- recover --confidence "$CONFIDENCE" --out "$OUT_DIR" "$IMAGE_PATH" | tee -a "$OUT_DIR/scan.log"

echo "\nRecovery finished. Recovered files are in: $OUT_DIR"
ls -la "$OUT_DIR"

echo "Done."
