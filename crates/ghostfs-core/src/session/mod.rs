//! Session persistence module
//!
//! This module provides SQLite-based persistence for recovery sessions,
//! allowing users to save scan results and resume recovery operations
//! without needing to rescan the filesystem.

pub mod database;
pub mod manager;

// Re-export main types
pub use database::{SessionDatabase, SessionSummary};
pub use manager::SessionManager;
