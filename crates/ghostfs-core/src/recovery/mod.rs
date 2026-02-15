/// Recovery module containing advanced algorithms and strategies
pub mod confidence;
pub mod directory;
pub mod engine;
pub mod signatures;

// Fragment reassembly modules
pub mod fragment_matcher;
pub mod fragments;
pub mod partial;
pub mod reassembly;
pub mod reconstruction;

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

pub use fragment_matcher::{calculate_entropy, FragmentMatcher, MatchScore};
pub use fragments::{Fragment, FragmentCatalog, FragmentId};
pub use partial::{PartialRecovery, PartialRecoveryResult};
pub use reassembly::{GapInfo, ReassemblyEngine, ReassemblyResult, ReassemblyStatistics};
pub use reconstruction::{ExtentReconstructor, ReconstructionResult, ReconstructionStrategy};
