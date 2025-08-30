/// Recovery module containing advanced algorithms and strategies
pub mod confidence;
pub mod signatures;
pub mod engine;

pub use confidence::{
    ConfidenceContext, ActivityLevel, calculate_confidence_score, 
    generate_confidence_report, ConfidenceReport, RecoveryRecommendation
};

pub use signatures::{
    FileSignature, SignatureMatch, SignatureAnalysisResult, ContentMetadata,
    init_signature_database, analyze_file_signature, extract_content_metadata
};

pub use engine::{
    RecoveryEngine, RecoveryConfig, RecoveryResult, RecoveryError,
    RecoveryProgress, RecoveryStage, RecoveryStrategy, ScanDepth,
    RecoveryStatistics
};
