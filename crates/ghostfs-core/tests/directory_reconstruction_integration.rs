//! Integration test for directory path reconstruction across all three filesystems

use ghostfs_core::recovery::{
    BtrfsDirEntry, BtrfsDirReconstructor, DirectoryReconstructor, ExFatDirReconstructor,
    XfsDirEntry, XfsDirReconstructor,
};
use std::path::PathBuf;

#[test]
fn test_xfs_path_reconstruction_integration() {
    let mut reconstructor = XfsDirReconstructor::new(4096);

    // Simulate a directory tree:
    // / (root, inode 64)
    //   └── home/ (inode 100)
    //       └── user/ (inode 200)
    //           ├── documents/ (inode 300)
    //           │   └── report.pdf (inode 400)
    //           └── file.txt (inode 250)

    reconstructor.add_entries(vec![
        XfsDirEntry {
            inode: 100,
            name: "home".to_string(),
            file_type: 2, // DIR
            parent_inode: 64,
            source_block: 0,
        },
        XfsDirEntry {
            inode: 200,
            name: "user".to_string(),
            file_type: 2, // DIR
            parent_inode: 100,
            source_block: 1,
        },
        XfsDirEntry {
            inode: 300,
            name: "documents".to_string(),
            file_type: 2, // DIR
            parent_inode: 200,
            source_block: 2,
        },
        XfsDirEntry {
            inode: 400,
            name: "report.pdf".to_string(),
            file_type: 1, // REG_FILE
            parent_inode: 300,
            source_block: 3,
        },
        XfsDirEntry {
            inode: 250,
            name: "file.txt".to_string(),
            file_type: 1, // REG_FILE
            parent_inode: 200,
            source_block: 4,
        },
    ]);

    // Test path reconstruction
    let path1 = reconstructor.reconstruct_path(400);
    assert_eq!(
        path1,
        Some(PathBuf::from("/home/user/documents/report.pdf"))
    );

    let path2 = reconstructor.reconstruct_path(250);
    assert_eq!(path2, Some(PathBuf::from("/home/user/file.txt")));

    let path3 = reconstructor.reconstruct_path(300);
    assert_eq!(path3, Some(PathBuf::from("/home/user/documents")));

    // Test filename extraction
    assert_eq!(
        reconstructor.get_filename(400),
        Some("report.pdf".to_string())
    );
    assert_eq!(
        reconstructor.get_filename(250),
        Some("file.txt".to_string())
    );

    // Test stats
    let stats = reconstructor.stats();
    assert_eq!(stats.total_entries, 5);
    assert!(stats.paths_reconstructed >= 3); // At least the ones we queried
    assert_eq!(stats.root_id, Some(64));
}

#[test]
fn test_btrfs_path_reconstruction_integration() {
    let mut reconstructor = BtrfsDirReconstructor::new();

    // Simulate a Btrfs directory tree:
    // / (root, inode 256)
    //   └── var/ (inode 257)
    //       └── log/ (inode 258)
    //           └── system.log (inode 259)

    reconstructor.add_entry(BtrfsDirEntry {
        inode: 257,
        name: "var".to_string(),
        file_type: 2, // DIR
        parent_inode: 256,
    });

    reconstructor.add_entry(BtrfsDirEntry {
        inode: 258,
        name: "log".to_string(),
        file_type: 2, // DIR
        parent_inode: 257,
    });

    reconstructor.add_entry(BtrfsDirEntry {
        inode: 259,
        name: "system.log".to_string(),
        file_type: 1, // REG_FILE
        parent_inode: 258,
    });

    // Test path reconstruction
    let path = reconstructor.reconstruct_path(259);
    assert_eq!(path, Some(PathBuf::from("/var/log/system.log")));

    // Test filename
    assert_eq!(
        reconstructor.get_filename(259),
        Some("system.log".to_string())
    );

    // Test stats
    let stats = reconstructor.stats();
    assert_eq!(stats.total_entries, 3);
    assert_eq!(stats.root_id, Some(256));
}

#[test]
fn test_exfat_path_reconstruction_integration() {
    // exFAT uses clusters instead of inodes
    // Root cluster is typically 5, cluster heap starts at offset 128KB
    let reconstructor = ExFatDirReconstructor::new(4096, 5, 131072);

    // For this test, we just verify the reconstructor can be created
    // and has the correct configuration
    assert_eq!(reconstructor.stats().root_id, Some(5));

    // The actual scanning would require a real exFAT filesystem image
    // which is beyond the scope of this unit test
}

#[test]
fn test_directory_reconstructor_trait() {
    // Test that all three reconstructors implement the DirectoryReconstructor trait

    let xfs_reconstructor = XfsDirReconstructor::new(4096);
    assert_eq!(xfs_reconstructor.stats().total_entries, 0);

    let btrfs_reconstructor = BtrfsDirReconstructor::new();
    assert_eq!(btrfs_reconstructor.stats().total_entries, 0);
    assert_eq!(btrfs_reconstructor.stats().root_id, Some(256));

    let exfat_reconstructor = ExFatDirReconstructor::new(4096, 5, 131072);
    assert_eq!(exfat_reconstructor.stats().total_entries, 0);
    assert_eq!(exfat_reconstructor.stats().root_id, Some(5));
}
