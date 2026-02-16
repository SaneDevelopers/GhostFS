//! High-level session management functionality

use anyhow::Result;
use std::path::Path;

use crate::RecoverySession;

use super::database::{SessionDatabase, SessionSummary};

/// High-level session manager
pub struct SessionManager {
    db: SessionDatabase,
}

impl SessionManager {
    /// Create a new session manager with the default database
    pub fn new() -> Result<Self> {
        let db_path = SessionDatabase::default_path()?;
        let db = SessionDatabase::open(db_path)?;
        Ok(Self { db })
    }

    /// Create a session manager with a custom database path
    pub fn with_path(path: impl AsRef<Path>) -> Result<Self> {
        let db = SessionDatabase::open(path)?;
        Ok(Self { db })
    }

    /// Get a reference to the underlying database
    pub fn database(&self) -> &SessionDatabase {
        &self.db
    }

    /// Save a session to the database
    pub fn save(&self, session: &RecoverySession) -> Result<()> {
        self.db.save_session(session)
    }

    /// Load a session by ID (supports short IDs)
    pub fn load(&self, id: &str) -> Result<RecoverySession> {
        self.db.load_session(id)
    }

    /// List all sessions
    pub fn list(&self) -> Result<Vec<SessionSummary>> {
        self.db.list_sessions()
    }

    /// List sessions by filesystem type
    pub fn list_sessions_by_fs(&self, fs_type: crate::FileSystemType) -> Result<Vec<SessionSummary>> {
        self.db.list_sessions_by_fs(fs_type)
    }

    /// List sessions by device path
    pub fn list_sessions_by_device(&self, device: &str) -> Result<Vec<SessionSummary>> {
        self.db.list_sessions_by_device(device)
    }

    /// Delete a session
    pub fn delete(&self, id: &str) -> Result<()> {
        self.db.delete_session(id)
    }

    /// Get the most recent session for a device
    pub fn find_recent_for_device(
        &self,
        device: impl AsRef<Path>,
    ) -> Result<Option<RecoverySession>> {
        let device_str = device.as_ref().display().to_string();
        let sessions = self.db.list_sessions_by_device(&device_str)?;

        if let Some(summary) = sessions.first() {
            Ok(Some(self.db.load_session(&summary.id.to_string())?))
        } else {
            Ok(None)
        }
    }

    /// Clean up old sessions
    pub fn cleanup(&self, days: u32) -> Result<usize> {
        self.db.cleanup_old_sessions(days)
    }

    /// Get session count
    pub fn count(&self) -> Result<usize> {
        self.db.count()
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new().expect("Failed to create default SessionManager")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FileSystemType, SessionMetadata};
    use chrono::Utc;
    use std::path::PathBuf;
    use tempfile::TempDir;
    use uuid::Uuid;

    fn create_test_session() -> RecoverySession {
        RecoverySession {
            id: Uuid::new_v4(),
            fs_type: FileSystemType::Xfs,
            device_path: PathBuf::from("/dev/sda1"),
            created_at: Utc::now(),
            scan_results: vec![],
            total_scanned: 1000,
            confidence_threshold: 0.5,
            metadata: SessionMetadata {
                device_size: 500_000_000_000,
                filesystem_size: 450_000_000_000,
                block_size: 4096,
                scan_duration_ms: 5000,
                files_found: 10,
                recoverable_files: 8,
            },
        }
    }

    #[test]
    fn test_manager_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let manager = SessionManager::with_path(&db_path).unwrap();

        let session = create_test_session();
        let session_id = session.id.to_string();

        manager.save(&session).unwrap();
        let loaded = manager.load(&session_id).unwrap();

        assert_eq!(loaded.id, session.id);
    }

    #[test]
    fn test_find_recent_for_device() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let manager = SessionManager::with_path(&db_path).unwrap();

        let session1 = create_test_session();
        let mut session2 = create_test_session();
        session2.created_at = Utc::now() + chrono::Duration::hours(1);

        manager.save(&session1).unwrap();
        manager.save(&session2).unwrap();

        let recent = manager
            .find_recent_for_device("/dev/sda1")
            .unwrap()
            .unwrap();

        // Should return the most recent (session2)
        assert_eq!(recent.id, session2.id);
    }
}
