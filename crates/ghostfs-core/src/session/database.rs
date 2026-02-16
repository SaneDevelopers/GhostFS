//! SQLite database operations for session persistence

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::{FileSystemType, RecoverySession};

/// SQLite database for storing recovery sessions
pub struct SessionDatabase {
    conn: Connection,
    db_path: PathBuf,
}

/// Lightweight session summary for listings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub id: Uuid,
    pub fs_type: FileSystemType,
    pub device_path: PathBuf,
    pub created_at: DateTime<Utc>,
    pub files_found: u32,
    pub recoverable_files: u32,
    pub device_size: u64,
    pub scan_duration_ms: u64,
}

impl SessionDatabase {
    /// Open or create a session database at the specified path
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let db_path = path.as_ref().to_path_buf();

        // Create parent directory if it doesn't exist
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .context("Failed to create database directory")?;
        }

        let conn = Connection::open(&db_path)
            .context(format!("Failed to open database at {}", db_path.display()))?;

        let db = Self { conn, db_path };
        db.initialize_schema()?;

        Ok(db)
    }

    /// Get the default database path (~/.ghostfs/sessions.db)
    pub fn default_path() -> Result<PathBuf> {
        let home = dirs::home_dir()
            .context("Could not determine home directory")?;

        let ghostfs_dir = home.join(".ghostfs");
        Ok(ghostfs_dir.join("sessions.db"))
    }

    /// Initialize or migrate database schema
    fn initialize_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                fs_type TEXT NOT NULL,
                device_path TEXT NOT NULL,
                created_at TEXT NOT NULL,
                total_scanned INTEGER NOT NULL,
                confidence_threshold REAL NOT NULL,
                device_size INTEGER NOT NULL,
                filesystem_size INTEGER NOT NULL,
                block_size INTEGER NOT NULL,
                scan_duration_ms INTEGER NOT NULL,
                files_found INTEGER NOT NULL,
                recoverable_files INTEGER NOT NULL,
                scan_results_json TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_sessions_created_at 
                ON sessions(created_at DESC);
            
            CREATE INDEX IF NOT EXISTS idx_sessions_fs_type 
                ON sessions(fs_type);
            
            CREATE INDEX IF NOT EXISTS idx_sessions_device 
                ON sessions(device_path);
            "#,
        )
        .context("Failed to initialize database schema")?;

        Ok(())
    }

    /// Save a recovery session to the database
    pub fn save_session(&self, session: &RecoverySession) -> Result<()> {
        // Serialize scan results to JSON
        let scan_results_json = serde_json::to_string(&session.scan_results)
            .context("Failed to serialize scan results")?;

        // Convert filesystem type to string
        let fs_type_str = match session.fs_type {
            FileSystemType::Xfs => "xfs",
            FileSystemType::Btrfs => "btrfs",
            FileSystemType::ExFat => "exfat",
        };

        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO sessions (
                id, fs_type, device_path, created_at,
                total_scanned, confidence_threshold,
                device_size, filesystem_size, block_size,
                scan_duration_ms, files_found, recoverable_files,
                scan_results_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            "#,
            params![
                session.id.to_string(),
                fs_type_str,
                session.device_path.display().to_string(),
                session.created_at.to_rfc3339(),
                session.total_scanned as i64,
                session.confidence_threshold,
                session.metadata.device_size as i64,
                session.metadata.filesystem_size as i64,
                session.metadata.block_size,
                session.metadata.scan_duration_ms as i64,
                session.metadata.files_found,
                session.metadata.recoverable_files,
                scan_results_json,
            ],
        )
        .context("Failed to save session to database")?;

        tracing::info!("Saved session {} to database", session.id);
        Ok(())
    }

    /// Load a session by ID (supports full UUID or short prefix)
    pub fn load_session(&self, id: &str) -> Result<RecoverySession> {
        // Try exact match first
        let mut stmt = self.conn.prepare(
            r#"
            SELECT 
                id, fs_type, device_path, created_at,
                total_scanned, confidence_threshold,
                device_size, filesystem_size, block_size,
                scan_duration_ms, files_found, recoverable_files,
                scan_results_json
            FROM sessions
            WHERE id = ?1 OR id LIKE ?2
            LIMIT 1
            "#,
        )?;

        let session = stmt
            .query_row(params![id, format!("{}%", id)], |row| {
                let id_str: String = row.get(0)?;
                let fs_type_str: String = row.get(1)?;
                let device_path_str: String = row.get(2)?;
                let created_at_str: String = row.get(3)?;
                let total_scanned: i64 = row.get(4)?;
                let confidence_threshold: f64 = row.get(5)?;
                let device_size: i64 = row.get(6)?;
                let filesystem_size: i64 = row.get(7)?;
                let block_size: u32 = row.get(8)?;
                let scan_duration_ms: i64 = row.get(9)?;
                let files_found: u32 = row.get(10)?;
                let recoverable_files: u32 = row.get(11)?;
                let scan_results_json: String = row.get(12)?;

                Ok((
                    id_str,
                    fs_type_str,
                    device_path_str,
                    created_at_str,
                    total_scanned,
                    confidence_threshold,
                    device_size,
                    filesystem_size,
                    block_size,
                    scan_duration_ms,
                    files_found,
                    recoverable_files,
                    scan_results_json,
                ))
            })
            .optional()
            .context("Failed to query session from database")?
            .context(format!("Session not found: {}", id))?;

        // Parse the results
        let (
            id_str,
            fs_type_str,
            device_path_str,
            created_at_str,
            total_scanned,
            confidence_threshold,
            device_size,
            filesystem_size,
            block_size,
            scan_duration_ms,
            files_found,
            recoverable_files,
            scan_results_json,
        ) = session;

        let id = Uuid::parse_str(&id_str).context("Invalid UUID in database")?;

        let fs_type = match fs_type_str.as_str() {
            "xfs" => FileSystemType::Xfs,
            "btrfs" => FileSystemType::Btrfs,
            "exfat" => FileSystemType::ExFat,
            _ => anyhow::bail!("Unknown filesystem type: {}", fs_type_str),
        };

        let created_at = DateTime::parse_from_rfc3339(&created_at_str)
            .context("Invalid timestamp in database")?
            .with_timezone(&Utc);

        let scan_results = serde_json::from_str(&scan_results_json)
            .context("Failed to deserialize scan results")?;

        Ok(RecoverySession {
            id,
            fs_type,
            device_path: PathBuf::from(device_path_str),
            created_at,
            scan_results,
            total_scanned: total_scanned as u64,
            confidence_threshold: confidence_threshold as f32,
            metadata: crate::SessionMetadata {
                device_size: device_size as u64,
                filesystem_size: filesystem_size as u64,
                block_size,
                scan_duration_ms: scan_duration_ms as u64,
                files_found,
                recoverable_files,
            },
        })
    }

    /// List all sessions (summary only)
    pub fn list_sessions(&self) -> Result<Vec<SessionSummary>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT 
                id, fs_type, device_path, created_at,
                files_found, recoverable_files, device_size, scan_duration_ms
            FROM sessions
            ORDER BY created_at DESC
            "#,
        )?;

        let sessions = stmt
            .query_map([], |row| {
                let id_str: String = row.get(0)?;
                let fs_type_str: String = row.get(1)?;
                let device_path_str: String = row.get(2)?;
                let created_at_str: String = row.get(3)?;
                let files_found: u32 = row.get(4)?;
                let recoverable_files: u32 = row.get(5)?;
                let device_size: i64 = row.get(6)?;
                let scan_duration_ms: i64 = row.get(7)?;

                Ok((
                    id_str,
                    fs_type_str,
                    device_path_str,
                    created_at_str,
                    files_found,
                    recoverable_files,
                    device_size,
                    scan_duration_ms,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()
            .context("Failed to query sessions")?;

        let mut summaries = Vec::new();

        for session in sessions {
            let (
                id_str,
                fs_type_str,
                device_path_str,
                created_at_str,
                files_found,
                recoverable_files,
                device_size,
                scan_duration_ms,
            ) = session;

            let id = match Uuid::parse_str(&id_str) {
                Ok(id) => id,
                Err(e) => {
                    tracing::warn!("Invalid UUID in database: {} - {}", id_str, e);
                    continue;
                }
            };

            let fs_type = match fs_type_str.as_str() {
                "xfs" => FileSystemType::Xfs,
                "btrfs" => FileSystemType::Btrfs,
                "exfat" => FileSystemType::ExFat,
                _ => {
                    tracing::warn!("Unknown filesystem type: {}", fs_type_str);
                    continue;
                }
            };

            let created_at = match DateTime::parse_from_rfc3339(&created_at_str) {
                Ok(dt) => dt.with_timezone(&Utc),
                Err(e) => {
                    tracing::warn!("Invalid timestamp: {} - {}", created_at_str, e);
                    continue;
                }
            };

            summaries.push(SessionSummary {
                id,
                fs_type,
                device_path: PathBuf::from(device_path_str),
                created_at,
                files_found,
                recoverable_files,
                device_size: device_size as u64,
                scan_duration_ms: scan_duration_ms as u64,
            });
        }

        Ok(summaries)
    }

    /// Delete a session by ID (supports full UUID or short prefix)
    pub fn delete_session(&self, id: &str) -> Result<()> {
        let rows_affected = self.conn.execute(
            "DELETE FROM sessions WHERE id = ?1 OR id LIKE ?2",
            params![id, format!("{}%", id)],
        )?;

        if rows_affected == 0 {
            anyhow::bail!("Session not found: {}", id);
        }

        tracing::info!("Deleted {} session(s) matching: {}", rows_affected, id);
        Ok(())
    }

    /// List sessions filtered by filesystem type
    pub fn list_sessions_by_fs(&self, fs_type: FileSystemType) -> Result<Vec<SessionSummary>> {
        let fs_type_str = match fs_type {
            FileSystemType::Xfs => "xfs",
            FileSystemType::Btrfs => "btrfs",
            FileSystemType::ExFat => "exfat",
        };

        let mut stmt = self.conn.prepare(
            r#"
            SELECT 
                id, fs_type, device_path, created_at,
                files_found, recoverable_files, device_size, scan_duration_ms
            FROM sessions
            WHERE fs_type = ?1
            ORDER BY created_at DESC
            "#,
        )?;

        let sessions = stmt
            .query_map([fs_type_str], |row| {
                let id_str: String = row.get(0)?;
                let fs_type_str: String = row.get(1)?;
                let device_path_str: String = row.get(2)?;
                let created_at_str: String = row.get(3)?;
                let files_found: u32 = row.get(4)?;
                let recoverable_files: u32 = row.get(5)?;
                let device_size: i64 = row.get(6)?;
                let scan_duration_ms: i64 = row.get(7)?;

                Ok((
                    id_str,
                    fs_type_str,
                    device_path_str,
                    created_at_str,
                    files_found,
                    recoverable_files,
                    device_size,
                    scan_duration_ms,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let mut summaries = Vec::new();

        for session in sessions {
            let (
                id_str,
                fs_type_str,
                device_path_str,
                created_at_str,
                files_found,
                recoverable_files,
                device_size,
                scan_duration_ms,
            ) = session;

            let id = Uuid::parse_str(&id_str)?;
            let fs_type = match fs_type_str.as_str() {
                "xfs" => FileSystemType::Xfs,
                "btrfs" => FileSystemType::Btrfs,
                "exfat" => FileSystemType::ExFat,
                _ => continue,
            };

            let created_at = DateTime::parse_from_rfc3339(&created_at_str)?
                .with_timezone(&Utc);

            summaries.push(SessionSummary {
                id,
                fs_type,
                device_path: PathBuf::from(device_path_str),
                created_at,
                files_found,
                recoverable_files,
                device_size: device_size as u64,
                scan_duration_ms: scan_duration_ms as u64,
            });
        }

        Ok(summaries)
    }

    /// List sessions for a specific device path
    pub fn list_sessions_by_device(&self, device: &str) -> Result<Vec<SessionSummary>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT 
                id, fs_type, device_path, created_at,
                files_found, recoverable_files, device_size, scan_duration_ms
            FROM sessions
            WHERE device_path = ?1
            ORDER BY created_at DESC
            "#,
        )?;

        let sessions = stmt
            .query_map([device], |row| {
                let id_str: String = row.get(0)?;
                let fs_type_str: String = row.get(1)?;
                let device_path_str: String = row.get(2)?;
                let created_at_str: String = row.get(3)?;
                let files_found: u32 = row.get(4)?;
                let recoverable_files: u32 = row.get(5)?;
                let device_size: i64 = row.get(6)?;
                let scan_duration_ms: i64 = row.get(7)?;

                Ok((
                    id_str,
                    fs_type_str,
                    device_path_str,
                    created_at_str,
                    files_found,
                    recoverable_files,
                    device_size,
                    scan_duration_ms,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let mut summaries = Vec::new();

        for session in sessions {
            let (
                id_str,
                fs_type_str,
                device_path_str,
                created_at_str,
                files_found,
                recoverable_files,
                device_size,
                scan_duration_ms,
            ) = session;

            let id = Uuid::parse_str(&id_str)?;
            let fs_type = match fs_type_str.as_str() {
                "xfs" => FileSystemType::Xfs,
                "btrfs" => FileSystemType::Btrfs,
                "exfat" => FileSystemType::ExFat,
                _ => continue,
            };

            let created_at = DateTime::parse_from_rfc3339(&created_at_str)?
                .with_timezone(&Utc);

            summaries.push(SessionSummary {
                id,
                fs_type,
                device_path: PathBuf::from(device_path_str),
                created_at,
                files_found,
                recoverable_files,
                device_size: device_size as u64,
                scan_duration_ms: scan_duration_ms as u64,
            });
        }

        Ok(summaries)
    }

    /// Clean up old sessions (older than specified days)
    pub fn cleanup_old_sessions(&self, days: u32) -> Result<usize> {
        let cutoff = Utc::now() - chrono::Duration::days(days as i64);
        let cutoff_str = cutoff.to_rfc3339();

        let rows_affected = self.conn.execute(
            "DELETE FROM sessions WHERE created_at < ?1",
            params![cutoff_str],
        )?;

        tracing::info!("Cleaned up {} sessions older than {} days", rows_affected, days);
        Ok(rows_affected)
    }

    /// Get the database file path
    pub fn path(&self) -> &Path {
        &self.db_path
    }

    /// Get the count of sessions in the database
    pub fn count(&self) -> Result<usize> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))?;
        Ok(count as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DeletedFile, FileType, SessionMetadata};
    use tempfile::TempDir;

    fn create_test_session() -> RecoverySession {
        RecoverySession {
            id: Uuid::new_v4(),
            fs_type: FileSystemType::Xfs,
            device_path: PathBuf::from("/dev/sda1"),
            created_at: Utc::now(),
            scan_results: vec![
                DeletedFile {
                    id: 1,
                    inode_or_cluster: 12345,
                    original_path: Some(PathBuf::from("/home/user/document.txt")),
                    size: 1024,
                    deletion_time: Some(Utc::now()),
                    confidence_score: 0.95,
                    file_type: FileType::RegularFile,
                    data_blocks: vec![],
                    is_recoverable: true,
                    metadata: crate::FileMetadata {
                        mime_type: Some("text/plain".to_string()),
                        file_extension: Some("txt".to_string()),
                        permissions: Some(0o644),
                        owner_uid: Some(1000),
                        owner_gid: Some(1000),
                        created_time: None,
                        modified_time: None,
                        accessed_time: None,
                        extended_attributes: Default::default(),
                    },
                    fs_metadata: None,
                },
            ],
            total_scanned: 1000,
            confidence_threshold: 0.5,
            metadata: SessionMetadata {
                device_size: 500_000_000_000,
                filesystem_size: 450_000_000_000,
                block_size: 4096,
                scan_duration_ms: 5000,
                files_found: 1,
                recoverable_files: 1,
            },
        }
    }

    #[test]
    fn test_database_open() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let _db = SessionDatabase::open(&db_path).unwrap();
        assert!(db_path.exists());
    }

    #[test]
    fn test_save_and_load_session() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = SessionDatabase::open(&db_path).unwrap();

        let session = create_test_session();
        let session_id = session.id.to_string();

        // Save
        db.save_session(&session).unwrap();

        // Load
        let loaded = db.load_session(&session_id).unwrap();

        assert_eq!(loaded.id, session.id);
        assert_eq!(loaded.fs_type, session.fs_type);
        assert_eq!(loaded.device_path, session.device_path);
        assert_eq!(loaded.scan_results.len(), 1);
        assert_eq!(loaded.total_scanned, session.total_scanned);
    }

    #[test]
    fn test_load_session_short_id() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = SessionDatabase::open(&db_path).unwrap();

        let session = create_test_session();
        let session_id = session.id.to_string();
        let short_id = &session_id[..8]; // First 8 characters

        db.save_session(&session).unwrap();

        // Load with short ID
        let loaded = db.load_session(short_id).unwrap();
        assert_eq!(loaded.id, session.id);
    }

    #[test]
    fn test_list_sessions() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = SessionDatabase::open(&db_path).unwrap();

        // Save multiple sessions
        let session1 = create_test_session();
        let session2 = create_test_session();

        db.save_session(&session1).unwrap();
        db.save_session(&session2).unwrap();

        let summaries = db.list_sessions().unwrap();
        assert_eq!(summaries.len(), 2);
    }

    #[test]
    fn test_delete_session() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = SessionDatabase::open(&db_path).unwrap();

        let session = create_test_session();
        let session_id = session.id.to_string();

        db.save_session(&session).unwrap();
        assert_eq!(db.count().unwrap(), 1);

        db.delete_session(&session_id).unwrap();
        assert_eq!(db.count().unwrap(), 0);
    }

    #[test]
    fn test_list_by_filesystem() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = SessionDatabase::open(&db_path).unwrap();

        let mut session_xfs = create_test_session();
        session_xfs.fs_type = FileSystemType::Xfs;

        let mut session_btrfs = create_test_session();
        session_btrfs.fs_type = FileSystemType::Btrfs;

        db.save_session(&session_xfs).unwrap();
        db.save_session(&session_btrfs).unwrap();

        let xfs_sessions = db.list_sessions_by_fs(FileSystemType::Xfs).unwrap();
        assert_eq!(xfs_sessions.len(), 1);
        assert_eq!(xfs_sessions[0].fs_type, FileSystemType::Xfs);
    }

    #[test]
    fn test_cleanup_old_sessions() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = SessionDatabase::open(&db_path).unwrap();

        let mut old_session = create_test_session();
        old_session.created_at = Utc::now() - chrono::Duration::days(60);

        let new_session = create_test_session();

        db.save_session(&old_session).unwrap();
        db.save_session(&new_session).unwrap();

        let deleted = db.cleanup_old_sessions(30).unwrap();
        assert_eq!(deleted, 1);
        assert_eq!(db.count().unwrap(), 1);
    }
}
