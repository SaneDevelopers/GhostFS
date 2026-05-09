cargo run -p ghostfs-cli -- scan test-data/test-xfs.img --fs xfs
cargo run -p ghostfs-cli -- scan test-data/test-btrfs.img --fs btrfs
cargo run -p ghostfs-cli -- scan test-data/test-exfat.img --fs exfat


# Recover only high-confidence files
cargo run -p ghostfs-cli -- recover --min-confidence 0.8 --output-dir ./recovered

# Timeline as JSON
cargo run -p ghostfs-cli -- timeline --format json --output timeline.json