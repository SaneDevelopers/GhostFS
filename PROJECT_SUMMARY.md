cargo run -p ghostfs-cli -- --help

cargo run -p ghostfs-cli -- detect test-data/test-xfs.img

cargo run -p ghostfs-cli -- scan test-data/test-xfs.img --fs xfs --confidence 0.3 --info

cargo run -p ghostfs-cli -- recover test-data/test-xfs.img --fs xfs --confidence 0.3 --out ./recovered

