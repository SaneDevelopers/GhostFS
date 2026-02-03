/// Recovery module containing advanced algorithms and strategies
pub mod confidence;
pub mod directory;
pub mod engine;
pub mod signatures;

pub use confidence::{
    calculate_confidence_score, generate_confidence_report, ActivityLevel, ConfidenceContext,
    ConfidenceReport, RecoveryRecommendation,
};

pub use signatures::{
    analyze_file_signature, extract_content_metadata, init_signature_database, ContentMetadata,
    FileSignature, SignatureAnalysisResult, SignatureMatch,
};

pub use engine::{
    RecoveryConfig, RecoveryEngine, RecoveryError, RecoveryProgress, RecoveryResult, RecoveryStage,
    RecoveryStatistics, RecoveryStrategy, ScanDepth,
};

pub use directory::{
    BtrfsDirEntry, BtrfsDirReconstructor, DirectoryReconstructor, ExFatDirEntry,
    ExFatDirReconstructor, ReconstructionStats, XfsDirEntry, XfsDirReconstructor,
};
