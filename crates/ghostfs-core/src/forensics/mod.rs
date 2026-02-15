/// Forensic features for legal and investigative use cases
pub mod audit;
pub mod recovery;
pub mod verification;

pub use audit::{AuditEntry, AuditEvent, AuditEventType, AuditLog, AuditLogger};

pub use verification::{
    calculate_file_hash, calculate_hash, verify_file_integrity, FileHash, HashAlgorithm,
    HashManifest, HashVerification, VerificationResult, VerificationStatus,
};

pub use recovery::{recover_files_with_forensics, ForensicsConfig, ForensicsRecoveryReport};
