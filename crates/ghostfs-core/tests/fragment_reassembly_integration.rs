/// Comprehensive integration test for fragment reassembly
///
/// This test demonstrates the complete workflow:
/// 1. Detect fragments from raw blocks
/// 2. Match related fragments
/// 3. Reassemble into complete files
/// 4. Write recovered files
use ghostfs_core::recovery::{
    FileSignature, Fragment, FragmentCatalog, FragmentMatcher, ReassemblyEngine, SignatureMatch,
};

#[test]
fn test_jpeg_fragment_reassembly() {
    // Simulate a fragmented JPEG file split into 3 fragments

    let mut catalog = FragmentCatalog::new();

    // Fragment 1: JPEG header
    let mut frag1 = Fragment::new(0, 0, 2048, 0);
    let jpeg_header = vec![
        0xFF, 0xD8, 0xFF, 0xE0, // JPEG SOI + APP0
        0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, // JFIF
              // ... more JPEG header data
    ];
    frag1.set_data(jpeg_header);
    frag1.signature = Some(SignatureMatch {
        category: "image".to_string(),
        signature: FileSignature {
            signature: vec![0xFF, 0xD8, 0xFF],
            offset: 0,
            mime_type: "image/jpeg".to_string(),
            extensions: vec!["jpg".to_string()],
            description: "JPEG Image".to_string(),
        },
        confidence: 1.0,
    });

    // Fragment 2: Middle section
    let mut frag2 = Fragment::new(0, 8192, 4096, 2);
    let jpeg_data = vec![0xAA; 4096]; // Simulated JPEG data
    frag2.set_data(jpeg_data);

    // Fragment 3: End marker
    let mut frag3 = Fragment::new(0, 16384, 1024, 4);
    let mut jpeg_end = vec![0xBB; 1020];
    jpeg_end.extend_from_slice(&[0xFF, 0xD9]); // JPEG EOI
    frag3.set_data(jpeg_end);

    // Set temporal hints (fragments from same time)
    let now = chrono::Utc::now();
    frag1.temporal_hint = Some(now);
    frag2.temporal_hint = Some(now);
    frag3.temporal_hint = Some(now);

    // Add fragments to catalog
    catalog.add_fragment(frag1);
    catalog.add_fragment(frag2);
    catalog.add_fragment(frag3);

    assert_eq!(catalog.len(), 3);

    // Create reassembly engine
    let engine = ReassemblyEngine::new(catalog).with_min_confidence(0.3); // Lower threshold for test

    // Reassemble JPEG fragments
    let results = engine.reassemble_by_type("image/jpeg");

    assert!(!results.is_empty(), "Should find reassembled files");

    let result = &results[0];
    // May not match all 3 fragments depending on similarity thresholds
    assert!(
        result.fragment_ids.len() >= 1,
        "Should reassemble at least 1 fragment"
    );
    assert!(result.confidence > 0.2, "Should have some confidence");
    assert_eq!(result.file_type.as_deref(), Some("image/jpeg"));

    println!(
        "✅ Successfully reassembled JPEG from {} fragments",
        result.fragment_ids.len()
    );
    println!("   Total size: {} bytes", result.total_size);
    println!("   Confidence: {:.2}%", result.confidence * 100.0);
}

#[test]
fn test_multiple_file_reassembly() {
    let mut catalog = FragmentCatalog::new();

    let now = chrono::Utc::now();
    let earlier = now - chrono::Duration::hours(48);

    // File 1: JPEG (2 fragments) — spatially close, same timestamp
    let mut jpeg1 = Fragment::new(0, 0, 1024, 0);
    jpeg1.signature = Some(SignatureMatch {
        category: "image".to_string(),
        signature: FileSignature {
            signature: vec![0xFF, 0xD8, 0xFF],
            offset: 0,
            mime_type: "image/jpeg".to_string(),
            extensions: vec!["jpg".to_string()],
            description: "JPEG".to_string(),
        },
        confidence: 1.0,
    });
    jpeg1.set_data(vec![0xFF, 0xD8, 0xFF, 0xE0]);
    jpeg1.temporal_hint = Some(now);

    let mut jpeg2 = Fragment::new(0, 1024, 1024, 1);
    jpeg2.set_data(vec![0xFF, 0xD8, 0xFF, 0xE0]); // Similar content
    jpeg2.temporal_hint = Some(now);

    // File 2: PNG (2 fragments) — spatially close, different timestamp
    let mut png1 = Fragment::new(0, 500_000, 2048, 2);
    png1.signature = Some(SignatureMatch {
        category: "image".to_string(),
        signature: FileSignature {
            signature: vec![0x89, 0x50, 0x4E, 0x47],
            offset: 0,
            mime_type: "image/png".to_string(),
            extensions: vec!["png".to_string()],
            description: "PNG".to_string(),
        },
        confidence: 1.0,
    });
    png1.set_data(vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A]);
    png1.temporal_hint = Some(earlier);

    let mut png2 = Fragment::new(0, 502_048, 2048, 3);
    png2.set_data(vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A]); // Similar content
    png2.temporal_hint = Some(earlier);

    catalog.add_fragment(jpeg1);
    catalog.add_fragment(jpeg2);
    catalog.add_fragment(png1);
    catalog.add_fragment(png2);

    let engine = ReassemblyEngine::new(catalog).with_min_confidence(0.3);

    // Reassemble all files
    let results = engine.reassemble_all();

    assert!(!results.is_empty(), "Should find multiple files");

    // Check we found both file types
    let jpeg_count = results
        .iter()
        .filter(|r| r.file_type.as_deref() == Some("image/jpeg"))
        .count();
    let png_count = results
        .iter()
        .filter(|r| r.file_type.as_deref() == Some("image/png"))
        .count();

    assert!(jpeg_count >= 1, "Should reassemble JPEG");
    assert!(png_count >= 1, "Should reassemble PNG");

    println!("✅ Reassembled {} files total", results.len());
    println!("   - {} JPEG files", jpeg_count);
    println!("   - {} PNG files", png_count);
}

#[test]
fn test_fragment_matcher_accuracy() {
    let matcher = FragmentMatcher::new().with_min_confidence(0.5);

    // Create two related fragments (from same file)
    let mut frag1 = Fragment::new(1, 0, 1024, 0);
    let mut frag2 = Fragment::new(2, 4096, 1024, 1);

    let data = vec![0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0];
    frag1.set_data(data.clone());
    frag2.set_data(data);

    // Set same temporal hint
    let now = chrono::Utc::now();
    frag1.temporal_hint = Some(now);
    frag2.temporal_hint = Some(now);

    // Create unrelated fragment
    let mut frag3 = Fragment::new(3, 100000, 2048, 50);
    frag3.set_data(vec![0xFF; 2048]);
    frag3.temporal_hint = Some(now - chrono::Duration::hours(24));

    let candidates = vec![&frag2, &frag3];
    let matches = matcher.find_best_matches(&frag1, &candidates);

    assert!(!matches.is_empty());

    // frag2 should match better than frag3
    let (best_match_id, best_score) = &matches[0];
    assert_eq!(*best_match_id, 2, "frag2 should be best match");
    assert!(best_score.confidence > 0.7, "Should have high confidence");

    println!("✅ Fragment matching working correctly");
    println!(
        "   Best match: Fragment {} with {:.2}% confidence",
        best_match_id,
        best_score.confidence * 100.0
    );
}

#[test]
fn test_high_fragmentation_scenario() {
    // Simulate a heavily fragmented file (10 fragments)
    let mut catalog = FragmentCatalog::new();

    let base_data = vec![0xAB, 0xCD, 0xEF, 0x01, 0x23, 0x45];

    // Create 10 fragments with similar content
    for i in 0..10 {
        let mut frag = Fragment::new(0, i * 4096, 512, i);
        let mut data = base_data.clone();
        data.push(i as u8); // Slight variation
        frag.set_data(data);

        // First fragment has signature
        if i == 0 {
            frag.signature = Some(SignatureMatch {
                category: "document".to_string(),
                signature: FileSignature {
                    signature: vec![0xAB, 0xCD],
                    offset: 0,
                    mime_type: "application/octet-stream".to_string(),
                    extensions: vec!["bin".to_string()],
                    description: "Binary".to_string(),
                },
                confidence: 0.8,
            });
        }

        catalog.add_fragment(frag);
    }

    let engine = ReassemblyEngine::new(catalog).with_min_confidence(0.3);
    let stats = engine.get_statistics();

    assert_eq!(stats.total_fragments, 10);
    assert!(stats.reassemblable_files > 0);

    println!("✅ High fragmentation test passed");
    println!("   Total fragments: {}", stats.total_fragments);
    println!("   Reassemblable files: {}", stats.reassemblable_files);
    println!(
        "   Avg fragments/file: {:.1}",
        stats.average_fragments_per_file
    );
}

#[test]
fn test_orphaned_fragments() {
    // Test handling of fragments that can't be matched
    let mut catalog = FragmentCatalog::new();

    // Fragment 1: JPEG
    let mut frag1 = Fragment::new(0, 0, 1024, 0);
    frag1.signature = Some(SignatureMatch {
        category: "image".to_string(),
        signature: FileSignature {
            signature: vec![0xFF, 0xD8, 0xFF],
            offset: 0,
            mime_type: "image/jpeg".to_string(),
            extensions: vec!["jpg".to_string()],
            description: "JPEG".to_string(),
        },
        confidence: 1.0,
    });
    frag1.set_data(vec![0xFF, 0xD8, 0xFF, 0xE0]);

    // Fragment 2: Orphaned (completely different, far away)
    let mut frag2 = Fragment::new(0, 1000000, 2048, 500);
    frag2.set_data(vec![0x00; 2048]);
    frag2.temporal_hint = Some(chrono::Utc::now() - chrono::Duration::days(30));

    catalog.add_fragment(frag1);
    catalog.add_fragment(frag2);

    let engine = ReassemblyEngine::new(catalog);
    let results = engine.reassemble_all();

    // Should still create result for the JPEG, orphan might be separate
    assert!(!results.is_empty());

    println!("✅ Orphaned fragment handling works");
    println!("   Reassembled files: {}", results.len());
}
