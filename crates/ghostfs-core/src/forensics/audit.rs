/// Audit trail logging for forensic accountability
///
/// This module provides comprehensive logging of all recovery operations
/// for legal compliance, chain of custody, and investigative transparency.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Types of auditable events
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AuditEventType {
    /// Recovery session started
    SessionStart,
    /// Recovery session ended
    SessionEnd,
    /// Disk scan initiated
    DiskScanStart,
    /// Disk scan completed
    DiskScanComplete,
    /// File signature detected
    FileDetected,
    /// File successfully recovered
    FileRecovered,
    /// File export completed
    FileExported,
    /// Hash calculated for verification
    HashCalculated,
    /// Verification performed
    VerificationPerformed,
    /// Configuration changed
    ConfigurationChange,
    /// Error occurred
    ErrorOccurred,
    /// Warning issued
    Warning,
    /// User action logged
    UserAction,
}

/// Single audit log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Unique entry ID
    pub id: u64,

    /// Timestamp (UTC)
    pub timestamp: DateTime<Utc>,

    /// Event type
    pub event_type: AuditEventType,

    /// Session ID this event belongs to
    pub session_id: String,

    /// User/operator who triggered the event
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,

    /// Event description
    pub message: String,

    /// Additional structured data
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, String>,

    /// Severity level
    pub severity: AuditSeverity,
}

/// Severity levels for audit events
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "UPPERCASE")]
pub enum AuditSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

/// Audit event builder
#[derive(Debug, Clone)]
pub struct AuditEvent {
    event_type: AuditEventType,
    message: String,
    metadata: HashMap<String, String>,
    severity: AuditSeverity,
    user: Option<String>,
}

impl AuditEvent {
    /// Create a new audit event
    pub fn new(event_type: AuditEventType, message: impl Into<String>) -> Self {
        let severity = match event_type {
            AuditEventType::ErrorOccurred => AuditSeverity::Error,
            AuditEventType::Warning => AuditSeverity::Warning,
            AuditEventType::SessionStart | AuditEventType::SessionEnd => AuditSeverity::Info,
            _ => AuditSeverity::Info,
        };

        Self {
            event_type,
            message: message.into(),
            metadata: HashMap::new(),
            severity,
            user: None,
        }
    }

    /// Add metadata key-value pair
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Set severity level
    pub fn with_severity(mut self, severity: AuditSeverity) -> Self {
        self.severity = severity;
        self
    }

    /// Set user/operator
    pub fn with_user(mut self, user: impl Into<String>) -> Self {
        self.user = Some(user.into());
        self
    }
}

/// Audit log manager
pub struct AuditLog {
    /// Session ID
    session_id: String,

    /// Log file path
    log_path: PathBuf,

    /// In-memory entries (for quick access)
    entries: Arc<Mutex<Vec<AuditEntry>>>,

    /// Next entry ID
    next_id: Arc<Mutex<u64>>,

    /// Log file handle
    log_file: Arc<Mutex<File>>,
}

impl AuditLog {
    /// Create a new audit log
    pub fn new(session_id: impl Into<String>, log_dir: impl AsRef<Path>) -> io::Result<Self> {
        let session_id = session_id.into();
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let log_filename = format!("audit_{}_{}.jsonl", session_id, timestamp);
        let log_path = log_dir.as_ref().join(log_filename);

        // Create log directory if needed
        if let Some(parent) = log_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let log_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)?;

        Ok(Self {
            session_id,
            log_path,
            entries: Arc::new(Mutex::new(Vec::new())),
            next_id: Arc::new(Mutex::new(1)),
            log_file: Arc::new(Mutex::new(log_file)),
        })
    }

    /// Log an audit event
    pub fn log(&self, event: AuditEvent) -> io::Result<u64> {
        let mut next_id = self.next_id.lock().unwrap();
        let id = *next_id;
        *next_id += 1;
        drop(next_id);

        let entry = AuditEntry {
            id,
            timestamp: Utc::now(),
            event_type: event.event_type,
            session_id: self.session_id.clone(),
            user: event.user,
            message: event.message,
            metadata: event.metadata,
            severity: event.severity,
        };

        // Write to file (JSONL format - one JSON object per line)
        let json = serde_json::to_string(&entry)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        let mut file = self.log_file.lock().unwrap();
        writeln!(file, "{}", json)?;
        file.flush()?;
        drop(file);

        // Store in memory
        let mut entries = self.entries.lock().unwrap();
        entries.push(entry);

        Ok(id)
    }

    /// Get all entries
    pub fn get_entries(&self) -> Vec<AuditEntry> {
        self.entries.lock().unwrap().clone()
    }

    /// Get entries by event type
    pub fn get_entries_by_type(&self, event_type: AuditEventType) -> Vec<AuditEntry> {
        self.entries
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.event_type == event_type)
            .cloned()
            .collect()
    }

    /// Get entries by severity
    pub fn get_entries_by_severity(&self, severity: AuditSeverity) -> Vec<AuditEntry> {
        self.entries
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.severity == severity)
            .cloned()
            .collect()
    }

    /// Get log file path
    pub fn log_path(&self) -> &Path {
        &self.log_path
    }

    /// Get session ID
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Export to JSON
    pub fn export_json(&self, output_path: impl AsRef<Path>) -> io::Result<()> {
        let entries = self.get_entries();
        let json = serde_json::to_string_pretty(&entries)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        std::fs::write(output_path, json)?;
        Ok(())
    }

    /// Export to CSV
    pub fn export_csv(&self, output_path: impl AsRef<Path>) -> io::Result<()> {
        let entries = self.get_entries();
        let mut csv = String::new();

        // Header
        csv.push_str("ID,Timestamp,EventType,SessionID,User,Message,Severity\n");

        // Rows
        for entry in entries {
            csv.push_str(&format!(
                "{},{},{:?},{},{},{},{:?}\n",
                entry.id,
                entry.timestamp.to_rfc3339(),
                entry.event_type,
                entry.session_id,
                entry.user.as_deref().unwrap_or("N/A"),
                entry.message.replace(',', ";"), // Escape commas
                entry.severity
            ));
        }

        std::fs::write(output_path, csv)?;
        Ok(())
    }

    /// Get statistics
    pub fn get_statistics(&self) -> AuditStatistics {
        let entries = self.entries.lock().unwrap();

        let mut event_counts = HashMap::new();
        let mut severity_counts = HashMap::new();

        for entry in entries.iter() {
            *event_counts.entry(entry.event_type.clone()).or_insert(0) += 1;
            *severity_counts.entry(entry.severity).or_insert(0) += 1;
        }

        AuditStatistics {
            total_entries: entries.len(),
            event_type_counts: event_counts,
            severity_counts: severity_counts,
            first_entry_time: entries.first().map(|e| e.timestamp),
            last_entry_time: entries.last().map(|e| e.timestamp),
        }
    }
}

/// Audit statistics
#[derive(Debug, Clone, Serialize)]
pub struct AuditStatistics {
    pub total_entries: usize,
    pub event_type_counts: HashMap<AuditEventType, usize>,
    pub severity_counts: HashMap<AuditSeverity, usize>,
    pub first_entry_time: Option<DateTime<Utc>>,
    pub last_entry_time: Option<DateTime<Utc>>,
}

/// Convenience wrapper for audit logging
pub struct AuditLogger {
    log: Arc<AuditLog>,
}

impl AuditLogger {
    /// Create a new audit logger
    pub fn new(log: Arc<AuditLog>) -> Self {
        Self { log }
    }

    /// Log session start
    pub fn session_start(&self, device: &str) -> io::Result<()> {
        self.log.log(
            AuditEvent::new(AuditEventType::SessionStart, "Recovery session started")
                .with_metadata("device", device),
        )?;
        Ok(())
    }

    /// Log session end
    pub fn session_end(&self, status: &str) -> io::Result<()> {
        self.log.log(
            AuditEvent::new(AuditEventType::SessionEnd, "Recovery session ended")
                .with_metadata("status", status),
        )?;
        Ok(())
    }

    /// Log file detected
    pub fn file_detected(&self, path: &str, signature: &str, confidence: f32) -> io::Result<()> {
        self.log.log(
            AuditEvent::new(
                AuditEventType::FileDetected,
                format!("File detected: {}", path),
            )
            .with_metadata("signature", signature)
            .with_metadata("confidence", confidence.to_string()),
        )?;
        Ok(())
    }

    /// Log file recovered
    pub fn file_recovered(&self, path: &str, size: u64, inode: u64) -> io::Result<()> {
        self.log.log(
            AuditEvent::new(
                AuditEventType::FileRecovered,
                format!("File recovered: {}", path),
            )
            .with_metadata("size_bytes", size.to_string())
            .with_metadata("inode", inode.to_string()),
        )?;
        Ok(())
    }

    /// Log file exported
    pub fn file_exported(&self, source: &str, destination: &str, bytes: u64) -> io::Result<()> {
        self.log.log(
            AuditEvent::new(
                AuditEventType::FileExported,
                "File exported to evidence directory",
            )
            .with_metadata("source", source)
            .with_metadata("destination", destination)
            .with_metadata("bytes_written", bytes.to_string()),
        )?;
        Ok(())
    }

    /// Log hash calculation
    pub fn hash_calculated(&self, file: &str, algorithm: &str, hash: &str) -> io::Result<()> {
        self.log.log(
            AuditEvent::new(
                AuditEventType::HashCalculated,
                format!("Hash calculated: {}", file),
            )
            .with_metadata("algorithm", algorithm)
            .with_metadata("hash", hash),
        )?;
        Ok(())
    }

    /// Log error
    pub fn error(&self, message: &str, details: &str) -> io::Result<()> {
        self.log.log(
            AuditEvent::new(AuditEventType::ErrorOccurred, message)
                .with_metadata("details", details)
                .with_severity(AuditSeverity::Error),
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_audit_log_creation() {
        let temp_dir = TempDir::new().unwrap();
        let log = AuditLog::new("test-session", temp_dir.path()).unwrap();

        assert_eq!(log.session_id(), "test-session");
        assert!(log.log_path().exists());
    }

    #[test]
    fn test_audit_event_logging() {
        let temp_dir = TempDir::new().unwrap();
        let log = AuditLog::new("test-session", temp_dir.path()).unwrap();

        let event = AuditEvent::new(AuditEventType::FileDetected, "Test file detected")
            .with_metadata("path", "/test/file.txt")
            .with_metadata("size", "1024");

        let id = log.log(event).unwrap();
        assert_eq!(id, 1);

        let entries = log.get_entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].message, "Test file detected");
        assert_eq!(
            entries[0].metadata.get("path"),
            Some(&"/test/file.txt".to_string())
        );
    }

    #[test]
    fn test_audit_logger_convenience() {
        let temp_dir = TempDir::new().unwrap();
        let log = Arc::new(AuditLog::new("test-session", temp_dir.path()).unwrap());
        let logger = AuditLogger::new(log.clone());

        logger.session_start("/dev/sda1").unwrap();
        logger
            .file_detected("test.txt", "text/plain", 0.95)
            .unwrap();
        logger
            .hash_calculated("test.txt", "SHA256", "abc123")
            .unwrap();

        let entries = log.get_entries();
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn test_audit_export_json() {
        let temp_dir = TempDir::new().unwrap();
        let log = AuditLog::new("test-session", temp_dir.path()).unwrap();

        log.log(AuditEvent::new(AuditEventType::SessionStart, "Test"))
            .unwrap();

        let json_path = temp_dir.path().join("audit.json");
        log.export_json(&json_path).unwrap();

        assert!(json_path.exists());
    }

    #[test]
    fn test_audit_statistics() {
        let temp_dir = TempDir::new().unwrap();
        let log = AuditLog::new("test-session", temp_dir.path()).unwrap();

        log.log(AuditEvent::new(AuditEventType::FileDetected, "File 1"))
            .unwrap();
        log.log(AuditEvent::new(AuditEventType::FileDetected, "File 2"))
            .unwrap();
        log.log(AuditEvent::new(AuditEventType::ErrorOccurred, "Error 1"))
            .unwrap();

        let stats = log.get_statistics();
        assert_eq!(stats.total_entries, 3);
        assert_eq!(
            stats.event_type_counts.get(&AuditEventType::FileDetected),
            Some(&2)
        );
    }
}
