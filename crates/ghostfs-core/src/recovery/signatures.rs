/// Advanced file signature analysis for recovery validation
use std::collections::HashMap;

/// File signature database for validating recovered files
#[derive(Debug, Clone)]
pub struct FileSignature {
    pub signature: Vec<u8>,
    pub offset: usize,
    pub mime_type: String,
    pub extensions: Vec<String>,
    pub description: String,
}

/// Initialize the comprehensive file signature database
pub fn init_signature_database() -> HashMap<String, Vec<FileSignature>> {
    let mut signatures = HashMap::new();

    // Image formats
    let image_sigs = vec![
        FileSignature {
            signature: vec![0xFF, 0xD8, 0xFF],
            offset: 0,
            mime_type: "image/jpeg".to_string(),
            extensions: vec!["jpg".to_string(), "jpeg".to_string()],
            description: "JPEG Image".to_string(),
        },
        FileSignature {
            signature: vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
            offset: 0,
            mime_type: "image/png".to_string(),
            extensions: vec!["png".to_string()],
            description: "PNG Image".to_string(),
        },
        FileSignature {
            signature: vec![0x47, 0x49, 0x46, 0x38, 0x37, 0x61], // GIF87a
            offset: 0,
            mime_type: "image/gif".to_string(),
            extensions: vec!["gif".to_string()],
            description: "GIF Image (87a)".to_string(),
        },
        FileSignature {
            signature: vec![0x47, 0x49, 0x46, 0x38, 0x39, 0x61], // GIF89a
            offset: 0,
            mime_type: "image/gif".to_string(),
            extensions: vec!["gif".to_string()],
            description: "GIF Image (89a)".to_string(),
        },
        FileSignature {
            signature: vec![0x42, 0x4D], // BM
            offset: 0,
            mime_type: "image/bmp".to_string(),
            extensions: vec!["bmp".to_string()],
            description: "Windows Bitmap".to_string(),
        },
    ];
    signatures.insert("image".to_string(), image_sigs);

    // Video formats
    let video_sigs = vec![
        FileSignature {
            signature: vec![0x00, 0x00, 0x00, 0x18, 0x66, 0x74, 0x79, 0x70], // ftyp
            offset: 4,
            mime_type: "video/mp4".to_string(),
            extensions: vec!["mp4".to_string(), "m4v".to_string()],
            description: "MPEG-4 Video".to_string(),
        },
        FileSignature {
            signature: vec![0x46, 0x4C, 0x56, 0x01], // FLV
            offset: 0,
            mime_type: "video/x-flv".to_string(),
            extensions: vec!["flv".to_string()],
            description: "Flash Video".to_string(),
        },
        FileSignature {
            signature: vec![0x1A, 0x45, 0xDF, 0xA3], // EBML
            offset: 0,
            mime_type: "video/webm".to_string(),
            extensions: vec!["webm".to_string(), "mkv".to_string()],
            description: "WebM/Matroska Video".to_string(),
        },
    ];
    signatures.insert("video".to_string(), video_sigs);

    // Audio formats
    let audio_sigs = vec![
        FileSignature {
            signature: vec![0xFF, 0xFB], // MP3 Frame sync
            offset: 0,
            mime_type: "audio/mpeg".to_string(),
            extensions: vec!["mp3".to_string()],
            description: "MP3 Audio".to_string(),
        },
        FileSignature {
            signature: vec![0x49, 0x44, 0x33], // ID3
            offset: 0,
            mime_type: "audio/mpeg".to_string(),
            extensions: vec!["mp3".to_string()],
            description: "MP3 Audio with ID3".to_string(),
        },
        FileSignature {
            signature: vec![0x66, 0x4C, 0x61, 0x43], // fLaC
            offset: 0,
            mime_type: "audio/flac".to_string(),
            extensions: vec!["flac".to_string()],
            description: "FLAC Audio".to_string(),
        },
        FileSignature {
            signature: vec![0x4F, 0x67, 0x67, 0x53], // OggS
            offset: 0,
            mime_type: "audio/ogg".to_string(),
            extensions: vec!["ogg".to_string(), "oga".to_string()],
            description: "Ogg Audio".to_string(),
        },
    ];
    signatures.insert("audio".to_string(), audio_sigs);

    // Document formats
    let document_sigs = vec![
        FileSignature {
            signature: vec![0x25, 0x50, 0x44, 0x46], // %PDF
            offset: 0,
            mime_type: "application/pdf".to_string(),
            extensions: vec!["pdf".to_string()],
            description: "PDF Document".to_string(),
        },
        FileSignature {
            signature: vec![0x50, 0x4B, 0x03, 0x04], // PK (ZIP-based)
            offset: 0,
            mime_type: "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
                .to_string(),
            extensions: vec!["docx".to_string(), "xlsx".to_string(), "pptx".to_string()],
            description: "Microsoft Office Document".to_string(),
        },
        FileSignature {
            signature: vec![0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1], // OLE
            offset: 0,
            mime_type: "application/msword".to_string(),
            extensions: vec!["doc".to_string(), "xls".to_string(), "ppt".to_string()],
            description: "Microsoft Office Legacy Document".to_string(),
        },
    ];
    signatures.insert("document".to_string(), document_sigs);

    // Archive formats
    let archive_sigs = vec![
        FileSignature {
            signature: vec![0x50, 0x4B, 0x03, 0x04], // ZIP
            offset: 0,
            mime_type: "application/zip".to_string(),
            extensions: vec!["zip".to_string()],
            description: "ZIP Archive".to_string(),
        },
        FileSignature {
            signature: vec![0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, 0x00], // Rar!
            offset: 0,
            mime_type: "application/vnd.rar".to_string(),
            extensions: vec!["rar".to_string()],
            description: "RAR Archive".to_string(),
        },
        FileSignature {
            signature: vec![0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C], // 7z
            offset: 0,
            mime_type: "application/x-7z-compressed".to_string(),
            extensions: vec!["7z".to_string()],
            description: "7-Zip Archive".to_string(),
        },
        FileSignature {
            signature: vec![0x1F, 0x8B, 0x08], // GZIP
            offset: 0,
            mime_type: "application/gzip".to_string(),
            extensions: vec!["gz".to_string(), "gzip".to_string()],
            description: "GZIP Archive".to_string(),
        },
    ];
    signatures.insert("archive".to_string(), archive_sigs);

    // Executable formats
    let executable_sigs = vec![
        FileSignature {
            signature: vec![0x4D, 0x5A], // MZ (Windows PE)
            offset: 0,
            mime_type: "application/vnd.microsoft.portable-executable".to_string(),
            extensions: vec!["exe".to_string(), "dll".to_string(), "sys".to_string()],
            description: "Windows Executable".to_string(),
        },
        FileSignature {
            signature: vec![0x7F, 0x45, 0x4C, 0x46], // ELF
            offset: 0,
            mime_type: "application/x-executable".to_string(),
            extensions: vec!["elf".to_string(), "so".to_string()],
            description: "Linux Executable".to_string(),
        },
        FileSignature {
            signature: vec![0xFE, 0xED, 0xFA, 0xCE], // Mach-O 32-bit
            offset: 0,
            mime_type: "application/x-mach-binary".to_string(),
            extensions: vec!["dylib".to_string()],
            description: "macOS Executable (32-bit)".to_string(),
        },
        FileSignature {
            signature: vec![0xFE, 0xED, 0xFA, 0xCF], // Mach-O 64-bit
            offset: 0,
            mime_type: "application/x-mach-binary".to_string(),
            extensions: vec!["dylib".to_string()],
            description: "macOS Executable (64-bit)".to_string(),
        },
    ];
    signatures.insert("executable".to_string(), executable_sigs);

    signatures
}

/// Analyze file content to determine file type and validate signature
pub fn analyze_file_signature(data: &[u8], max_bytes: usize) -> SignatureAnalysisResult {
    let signatures = init_signature_database();
    let analysis_data = &data[..std::cmp::min(data.len(), max_bytes)];

    let mut matches = Vec::new();

    // Check all signature categories
    for (category, category_sigs) in &signatures {
        for signature in category_sigs {
            if signature.offset + signature.signature.len() <= analysis_data.len() {
                let slice =
                    &analysis_data[signature.offset..signature.offset + signature.signature.len()];
                if slice == signature.signature {
                    matches.push(SignatureMatch {
                        category: category.clone(),
                        signature: signature.clone(),
                        confidence: calculate_signature_confidence(signature, analysis_data),
                    });
                }
            }
        }
    }

    // Sort by confidence
    matches.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());

    SignatureAnalysisResult {
        matches,
        analyzed_bytes: analysis_data.len(),
        is_text_file: is_likely_text_file(analysis_data),
        entropy: calculate_entropy(analysis_data),
    }
}

#[derive(Debug, Clone)]
pub struct SignatureMatch {
    pub category: String,
    pub signature: FileSignature,
    pub confidence: f32,
}

#[derive(Debug)]
pub struct SignatureAnalysisResult {
    pub matches: Vec<SignatureMatch>,
    pub analyzed_bytes: usize,
    pub is_text_file: bool,
    pub entropy: f32,
}

/// Calculate confidence for a signature match
fn calculate_signature_confidence(signature: &FileSignature, data: &[u8]) -> f32 {
    let mut confidence = 0.8; // Base confidence for signature match

    // Longer signatures are more reliable
    confidence += (signature.signature.len() as f32 / 20.0).min(0.2);

    // Signatures at offset 0 are more reliable
    if signature.offset == 0 {
        confidence += 0.1;
    }

    // Check for additional validation patterns
    confidence += validate_additional_patterns(signature, data);

    confidence.min(1.0)
}

/// Validate additional file format patterns beyond the main signature
fn validate_additional_patterns(signature: &FileSignature, data: &[u8]) -> f32 {
    match signature.mime_type.as_str() {
        "image/jpeg" => validate_jpeg_structure(data),
        "image/png" => validate_png_structure(data),
        "application/pdf" => validate_pdf_structure(data),
        "video/mp4" => validate_mp4_structure(data),
        _ => 0.0,
    }
}

fn validate_jpeg_structure(data: &[u8]) -> f32 {
    if data.len() < 10 {
        return 0.0;
    }

    let mut confidence = 0.0;

    // Look for JFIF or EXIF markers
    for i in 0..data.len().saturating_sub(4) {
        if &data[i..i + 4] == b"JFIF" {
            confidence += 0.15;
            break;
        }
        if &data[i..i + 4] == b"Exif" {
            confidence += 0.1;
            break;
        }
    }

    // Look for End of Image marker
    for i in 0..data.len().saturating_sub(2) {
        if data[i] == 0xFF && data[i + 1] == 0xD9 {
            confidence += 0.1;
            break;
        }
    }

    confidence
}

fn validate_png_structure(data: &[u8]) -> f32 {
    if data.len() < 33 {
        // PNG header + IHDR chunk minimum
        return 0.0;
    }

    let mut confidence = 0.0;

    // Check for IHDR chunk at position 12
    if data.len() >= 16 && &data[12..16] == b"IHDR" {
        confidence += 0.15;
    }

    // Look for other common PNG chunks
    let chunks = [b"IDAT", b"IEND", b"tEXt", b"gAMA"];
    for chunk in chunks {
        for i in 0..data.len().saturating_sub(4) {
            if &data[i..i + 4] == chunk {
                confidence += 0.05;
                break;
            }
        }
    }

    confidence
}

fn validate_pdf_structure(data: &[u8]) -> f32 {
    if data.len() < 100 {
        return 0.0;
    }

    let mut confidence = 0.0;
    let data_str = String::from_utf8_lossy(&data[..std::cmp::min(data.len(), 1000)]);

    // Look for PDF structure elements
    if data_str.contains("trailer") {
        confidence += 0.1;
    }
    if data_str.contains("startxref") {
        confidence += 0.1;
    }
    if data_str.contains("obj") {
        confidence += 0.05;
    }
    if data_str.contains("endobj") {
        confidence += 0.05;
    }

    confidence
}

fn validate_mp4_structure(data: &[u8]) -> f32 {
    if data.len() < 32 {
        return 0.0;
    }

    let mut confidence = 0.0;

    // Look for common MP4 atoms
    let atoms = [b"moov", b"mdat", b"ftyp", b"mdhd", b"trak"];
    for atom in &atoms {
        for i in 0..data.len().saturating_sub(4) {
            if &data[i..i + 4] == *atom {
                confidence += 0.04;
                break;
            }
        }
    }

    confidence
}

/// Check if file content appears to be text
fn is_likely_text_file(data: &[u8]) -> bool {
    if data.is_empty() {
        return false;
    }

    let sample_size = std::cmp::min(data.len(), 1024);
    let sample = &data[..sample_size];

    // Count printable characters
    let printable_count = sample
        .iter()
        .filter(|&&b| (32..=126).contains(&b) || b == 9 || b == 10 || b == 13)
        .count();

    let printable_ratio = printable_count as f32 / sample.len() as f32;

    // Check for UTF-8 BOM
    let has_utf8_bom = sample.len() >= 3 && sample[..3] == [0xEF, 0xBB, 0xBF];

    printable_ratio > 0.9 || has_utf8_bom
}

/// Calculate Shannon entropy of data
fn calculate_entropy(data: &[u8]) -> f32 {
    if data.is_empty() {
        return 0.0;
    }

    let mut frequencies = [0u32; 256];
    for &byte in data {
        frequencies[byte as usize] += 1;
    }

    let len = data.len() as f32;
    let mut entropy = 0.0;

    for &freq in &frequencies {
        if freq > 0 {
            let p = freq as f32 / len;
            entropy -= p * p.log2();
        }
    }

    entropy
}

/// Extract detailed metadata from file content
pub fn extract_content_metadata(data: &[u8], signature_match: &SignatureMatch) -> ContentMetadata {
    match signature_match.signature.mime_type.as_str() {
        "image/jpeg" => extract_jpeg_metadata(data),
        "image/png" => extract_png_metadata(data),
        "application/pdf" => extract_pdf_metadata(data),
        _ => ContentMetadata::default(),
    }
}

#[derive(Debug, Default)]
pub struct ContentMetadata {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub color_depth: Option<u8>,
    pub compression: Option<String>,
    pub creation_software: Option<String>,
    pub additional_info: HashMap<String, String>,
}

fn extract_jpeg_metadata(data: &[u8]) -> ContentMetadata {
    let mut metadata = ContentMetadata::default();

    // Look for APP0 segment (JFIF)
    for i in 0..data.len().saturating_sub(16) {
        if data[i] == 0xFF && data[i + 1] == 0xE0 && &data[i + 4..i + 8] == b"JFIF" {
            // Extract JFIF version and other info
            if data.len() > i + 14 {
                metadata.additional_info.insert(
                    "jfif_version".to_string(),
                    format!("{}.{}", data[i + 9], data[i + 10]),
                );
            }
            break;
        }
    }

    metadata
}

fn extract_png_metadata(data: &[u8]) -> ContentMetadata {
    let mut metadata = ContentMetadata::default();

    // Parse IHDR chunk for dimensions and color info
    if data.len() >= 25 && &data[12..16] == b"IHDR" {
        let width = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
        let height = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
        let bit_depth = data[24];
        let color_type = data[25];

        metadata.width = Some(width);
        metadata.height = Some(height);
        metadata.color_depth = Some(bit_depth);
        metadata
            .additional_info
            .insert("color_type".to_string(), color_type.to_string());
    }

    metadata
}

fn extract_pdf_metadata(_data: &[u8]) -> ContentMetadata {
    // PDF metadata extraction would require more complex parsing
    ContentMetadata::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jpeg_signature_detection() {
        let jpeg_header = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F'];
        let result = analyze_file_signature(&jpeg_header, 1024);

        assert!(!result.matches.is_empty());
        assert_eq!(result.matches[0].signature.mime_type, "image/jpeg");
        assert!(result.matches[0].confidence > 0.8);
    }

    #[test]
    fn test_png_signature_detection() {
        let png_header = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        let result = analyze_file_signature(&png_header, 1024);

        assert!(!result.matches.is_empty());
        assert_eq!(result.matches[0].signature.mime_type, "image/png");
    }

    #[test]
    fn test_text_file_detection() {
        let text_content = b"Hello, this is a text file with normal characters.";
        assert!(is_likely_text_file(text_content));

        let binary_content = vec![0x00, 0x01, 0x02, 0xFF, 0xFE, 0xFD];
        assert!(!is_likely_text_file(&binary_content));
    }

    #[test]
    fn test_entropy_calculation() {
        // Uniform distribution should have high entropy
        let uniform = (0..=255u8).collect::<Vec<_>>();
        let entropy = calculate_entropy(&uniform);
        assert!(entropy > 7.5); // Close to 8.0 for uniform distribution

        // All same bytes should have zero entropy
        let same_bytes = vec![0x42; 1000];
        let entropy = calculate_entropy(&same_bytes);
        assert!(entropy < 0.1);
    }
}
