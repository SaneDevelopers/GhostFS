/// Hash-based file verification for forensic integrity
///
/// This module provides cryptographic hash calculation and verification
/// to ensure recovered files are authentic and haven't been corrupted.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use sha2::{Sha256, Sha512, Digest};

/// Supported hash algorithms
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum HashAlgorithm {
    MD5,
    SHA1,
    SHA256,
    SHA512,
}

impl HashAlgorithm {
    /// Get all algorithms
    pub fn all() -> Vec<Self> {
        vec![Self::MD5, Self::SHA256, Self::SHA512]
    }
    
    /// Get algorithm name
    pub fn name(&self) -> &'static str {
        match self {
            Self::MD5 => "MD5",
            Self::SHA1 => "SHA1",
            Self::SHA256 => "SHA256",
            Self::SHA512 => "SHA512",
        }
    }
}

/// File hash result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileHash {
    /// Hash algorithm used
    pub algorithm: HashAlgorithm,
    
    /// Hexadecimal hash value
    pub hash: String,
    
    /// File size in bytes
    pub file_size: u64,
    
    /// When the hash was calculated
    #[serde(skip_serializing_if = "Option::is_none")]
    pub calculated_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Hash verification result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HashVerification {
    /// File path
    pub file_path: String,
    
    /// Expected hash (from metadata or database)
    pub expected_hash: Option<String>,
    
    /// Actual calculated hash
    pub actual_hash: String,
    
    /// Hash algorithm used
    pub algorithm: HashAlgorithm,
    
    /// Verification status
    pub status: VerificationStatus,
    
    /// Additional notes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

/// Verification status
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum VerificationStatus {
    /// Hash matches expected value
    Verified,
    
    /// Hash does not match (corruption detected)
    Corrupted,
    
    /// No expected hash to compare against
    NoReference,
    
    /// Hash calculated successfully (no reference to verify)
    Calculated,
}

/// Calculate hash for a file
pub fn calculate_file_hash(
    path: impl AsRef<Path>,
    algorithm: HashAlgorithm,
) -> io::Result<FileHash> {
    let mut file = File::open(path.as_ref())?;
    let metadata = file.metadata()?;
    let file_size = metadata.len();
    
    let hash = match algorithm {
        HashAlgorithm::MD5 => {
            let mut hasher = md5::Context::new();
            let mut buffer = vec![0; 8192];
            loop {
                let n = file.read(&mut buffer)?;
                if n == 0 {
                    break;
                }
                hasher.consume(&buffer[..n]);
            }
            format!("{:x}", hasher.compute())
        }
        HashAlgorithm::SHA256 => {
            let mut hasher = Sha256::new();
            let mut buffer = vec![0; 8192];
            loop {
                let n = file.read(&mut buffer)?;
                if n == 0 {
                    break;
                }
                hasher.update(&buffer[..n]);
            }
            format!("{:x}", hasher.finalize())
        }
        HashAlgorithm::SHA1 => {
            let mut hasher = sha1::Sha1::new();
            let mut buffer = vec![0; 8192];
            loop {
                let n = file.read(&mut buffer)?;
                if n == 0 {
                    break;
                }
                hasher.update(&buffer[..n]);
            }
            format!("{:x}", hasher.finalize())
        }
        HashAlgorithm::SHA512 => {
            let mut hasher = Sha512::new();
            let mut buffer = vec![0; 8192];
            loop {
                let n = file.read(&mut buffer)?;
                if n == 0 {
                    break;
                }
                hasher.update(&buffer[..n]);
            }
            format!("{:x}", hasher.finalize())
        }
    };
    
    Ok(FileHash {
        algorithm,
        hash,
        file_size,
        calculated_at: Some(chrono::Utc::now()),
    })
}

/// Calculate hash from byte slice
pub fn calculate_hash(data: &[u8], algorithm: HashAlgorithm) -> String {
    match algorithm {
        HashAlgorithm::MD5 => {
            format!("{:x}", md5::compute(data))
        }
        HashAlgorithm::SHA256 => {
            let mut hasher = Sha256::new();
            hasher.update(data);
            format!("{:x}", hasher.finalize())
        }
        HashAlgorithm::SHA1 => {
            let mut hasher = sha1::Sha1::new();
            hasher.update(data);
            format!("{:x}", hasher.finalize())
        }
        HashAlgorithm::SHA512 => {
            let mut hasher = Sha512::new();
            hasher.update(data);
            format!("{:x}", hasher.finalize())
        }
    }
}

/// Verify file integrity against expected hash
pub fn verify_file_integrity(
    path: impl AsRef<Path>,
    expected_hash: Option<&str>,
    algorithm: HashAlgorithm,
) -> io::Result<HashVerification> {
    let file_hash = calculate_file_hash(&path, algorithm)?;
    let actual_hash = file_hash.hash;
    
    let (status, notes) = if let Some(expected) = expected_hash {
        if actual_hash.eq_ignore_ascii_case(expected) {
            (VerificationStatus::Verified, Some("Hash matches - File is authentic".to_string()))
        } else {
            (VerificationStatus::Corrupted, Some("Hash mismatch - Possible corruption".to_string()))
        }
    } else {
        (VerificationStatus::NoReference, Some("No reference hash available".to_string()))
    };
    
    Ok(HashVerification {
        file_path: path.as_ref().display().to_string(),
        expected_hash: expected_hash.map(String::from),
        actual_hash,
        algorithm,
        status,
        notes,
    })
}

/// Verification result with detailed analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    /// All hash verifications performed
    pub verifications: Vec<HashVerification>,
    
    /// Summary statistics
    pub summary: VerificationSummary,
}

/// Summary of verification results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationSummary {
    pub total_files: usize,
    pub verified: usize,
    pub corrupted: usize,
    pub no_reference: usize,
    pub success_rate: f32,
}

impl VerificationResult {
    /// Create from verifications
    pub fn from_verifications(verifications: Vec<HashVerification>) -> Self {
        let total_files = verifications.len();
        let verified = verifications.iter().filter(|v| v.status == VerificationStatus::Verified).count();
        let corrupted = verifications.iter().filter(|v| v.status == VerificationStatus::Corrupted).count();
        let no_reference = verifications.iter().filter(|v| v.status == VerificationStatus::NoReference).count();
        
        let success_rate = if total_files > 0 {
            verified as f32 / total_files as f32
        } else {
            0.0
        };
        
        Self {
            verifications,
            summary: VerificationSummary {
                total_files,
                verified,
                corrupted,
                no_reference,
                success_rate,
            },
        }
    }
}

/// Hash manifest for a collection of files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HashManifest {
    /// Session or collection ID
    pub manifest_id: String,
    
    /// When manifest was created
    pub created_at: chrono::DateTime<chrono::Utc>,
    
    /// Hash algorithm used
    pub algorithm: HashAlgorithm,
    
    /// File hashes
    pub files: HashMap<String, FileHash>,
}

impl HashManifest {
    /// Create new manifest
    pub fn new(manifest_id: impl Into<String>, algorithm: HashAlgorithm) -> Self {
        Self {
            manifest_id: manifest_id.into(),
            created_at: chrono::Utc::now(),
            algorithm,
            files: HashMap::new(),
        }
    }
    
    /// Add file hash
    pub fn add_file(&mut self, path: String, hash: FileHash) {
        self.files.insert(path, hash);
    }
    
    /// Get file hash
    pub fn get_file(&self, path: &str) -> Option<&FileHash> {
        self.files.get(path)
    }
    
    /// Verify all files in manifest
    pub fn verify_all(&self, base_path: impl AsRef<Path>) -> io::Result<VerificationResult> {
        let mut verifications = Vec::new();
        
        for (file_path, expected_hash) in &self.files {
            let full_path = base_path.as_ref().join(file_path);
            
            if !full_path.exists() {
                verifications.push(HashVerification {
                    file_path: file_path.clone(),
                    expected_hash: Some(expected_hash.hash.clone()),
                    actual_hash: String::new(),
                    algorithm: self.algorithm,
                    status: VerificationStatus::Corrupted,
                    notes: Some("File not found".to_string()),
                });
                continue;
            }
            
            let verification = verify_file_integrity(
                &full_path,
                Some(&expected_hash.hash),
                self.algorithm,
            )?;
            
            verifications.push(verification);
        }
        
        Ok(VerificationResult::from_verifications(verifications))
    }
    
    /// Export to JSON
    pub fn export_json(&self, path: impl AsRef<Path>) -> io::Result<()> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, json)
    }
    
    /// Import from JSON
    pub fn import_json(path: impl AsRef<Path>) -> io::Result<Self> {
        let json = std::fs::read_to_string(path)?;
        serde_json::from_str(&json)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;
    
    #[test]
    fn test_calculate_hash_from_bytes() {
        let data = b"Hello, World!";
        
        let md5 = calculate_hash(data, HashAlgorithm::MD5);
        let sha256 = calculate_hash(data, HashAlgorithm::SHA256);
        
        assert_eq!(md5.len(), 32); // MD5 = 128 bits = 32 hex chars
        assert_eq!(sha256.len(), 64); // SHA256 = 256 bits = 64 hex chars
    }
    
    #[test]
    fn test_calculate_file_hash() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        
        let mut file = File::create(&file_path).unwrap();
        file.write_all(b"Test data").unwrap();
        drop(file);
        
        let hash = calculate_file_hash(&file_path, HashAlgorithm::SHA256).unwrap();
        
        assert_eq!(hash.algorithm, HashAlgorithm::SHA256);
        assert_eq!(hash.file_size, 9);
        assert!(!hash.hash.is_empty());
    }
    
    #[test]
    fn test_verify_integrity_success() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        
        let mut file = File::create(&file_path).unwrap();
        file.write_all(b"Test data").unwrap();
        drop(file);
        
        // Calculate expected hash
        let expected = calculate_file_hash(&file_path, HashAlgorithm::SHA256).unwrap();
        
        // Verify
        let verification = verify_file_integrity(
            &file_path,
            Some(&expected.hash),
            HashAlgorithm::SHA256,
        ).unwrap();
        
        assert_eq!(verification.status, VerificationStatus::Verified);
    }
    
    #[test]
    fn test_verify_integrity_corruption() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        
        let mut file = File::create(&file_path).unwrap();
        file.write_all(b"Test data").unwrap();
        drop(file);
        
        // Use wrong hash
        let verification = verify_file_integrity(
            &file_path,
            Some("0000000000000000000000000000000000000000000000000000000000000000"),
            HashAlgorithm::SHA256,
        ).unwrap();
        
        assert_eq!(verification.status, VerificationStatus::Corrupted);
    }
    
    #[test]
    fn test_hash_manifest() {
        let temp_dir = TempDir::new().unwrap();
        
        // Create test files
        let file1 = temp_dir.path().join("file1.txt");
        let file2 = temp_dir.path().join("file2.txt");
        
        std::fs::write(&file1, b"File 1 data").unwrap();
        std::fs::write(&file2, b"File 2 data").unwrap();
        
        // Create manifest
        let mut manifest = HashManifest::new("test-manifest", HashAlgorithm::SHA256);
        
        let hash1 = calculate_file_hash(&file1, HashAlgorithm::SHA256).unwrap();
        let hash2 = calculate_file_hash(&file2, HashAlgorithm::SHA256).unwrap();
        
        manifest.add_file("file1.txt".to_string(), hash1);
        manifest.add_file("file2.txt".to_string(), hash2);
        
        // Verify all
        let result = manifest.verify_all(temp_dir.path()).unwrap();
        
        assert_eq!(result.summary.total_files, 2);
        assert_eq!(result.summary.verified, 2);
        assert_eq!(result.summary.corrupted, 0);
        assert_eq!(result.summary.success_rate, 1.0);
    }
    
    #[test]
    fn test_manifest_export_import() {
        let temp_dir = TempDir::new().unwrap();
        
        let mut manifest = HashManifest::new("test-manifest", HashAlgorithm::SHA256);
        manifest.add_file(
            "test.txt".to_string(),
            FileHash {
                algorithm: HashAlgorithm::SHA256,
                hash: "abc123".to_string(),
                file_size: 100,
                calculated_at: Some(chrono::Utc::now()),
            },
        );
        
        let export_path = temp_dir.path().join("manifest.json");
        manifest.export_json(&export_path).unwrap();
        
        let imported = HashManifest::import_json(&export_path).unwrap();
        assert_eq!(imported.manifest_id, "test-manifest");
        assert_eq!(imported.files.len(), 1);
    }
}
