/// Timeline recovery analysis module
///
/// Provides functionality to analyze deletion patterns, generate recovery timelines,
/// and detect suspicious file deletion activities from recovery sessions.
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{DeletedFile, RecoverySession, TimelineEntry, TimelineEventType};

/// Complete recovery timeline with events, patterns, and statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryTimeline {
    /// Chronologically sorted timeline events
    pub events: Vec<TimelineEntry>,
    /// Detected deletion patterns
    pub patterns: Vec<DeletionPattern>,
    /// Statistical analysis of the timeline
    pub statistics: TimelineStatistics,
}

/// A detected pattern in file deletion behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeletionPattern {
    /// Type of pattern detected
    pub pattern_type: PatternType,
    /// Confidence in this pattern detection (0.0-1.0)
    pub confidence: f32,
    /// File IDs affected by this pattern
    pub affected_files: Vec<u64>,
    /// Time window for this pattern
    pub timeframe: (DateTime<Utc>, DateTime<Utc>),
    /// Human-readable description
    pub description: String,
}

/// Types of deletion patterns that can be detected
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatternType {
    /// Many files deleted in a short time period
    BulkDeletion,
    /// Specific file types targeted for deletion
    SelectiveDeletion,
    /// Regular time-based deletion pattern
    PeriodicDeletion,
    /// Unusual or suspicious deletion activity
    SuspiciousActivity,
}

/// Statistical analysis of the timeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineStatistics {
    /// Total number of events in timeline
    pub total_events: usize,
    /// Number of deletion events
    pub deletion_events: usize,
    /// Time with highest deletion activity
    pub peak_deletion_time: Option<DateTime<Utc>>,
    /// Average deletions per day
    pub average_deletions_per_day: f32,
    /// Count of each file type affected
    pub file_types_affected: HashMap<String, usize>,
}

impl RecoveryTimeline {
    /// Build a timeline from a recovery session
    ///
    /// Extracts all timestamp-based events from deleted files, sorts them
    /// chronologically, detects patterns, and calculates statistics.
    pub fn from_session(session: &RecoverySession) -> Self {
        let mut events = Vec::new();

        // Extract all timestamp events from deleted files
        for file in &session.scan_results {
            // Creation events
            if let Some(created) = file.metadata.created_time {
                events.push(TimelineEntry {
                    timestamp: created,
                    event_type: TimelineEventType::FileCreated,
                    file_id: file.id,
                    description: format!(
                        "Created: {} ({})",
                        file.original_path
                            .as_ref()
                            .map(|p| p.display().to_string())
                            .unwrap_or_else(|| format!("inode_{}", file.inode_or_cluster)),
                        file.metadata
                            .mime_type
                            .as_ref()
                            .unwrap_or(&"unknown".to_string())
                    ),
                });
            }

            // Modification events
            if let Some(modified) = file.metadata.modified_time {
                events.push(TimelineEntry {
                    timestamp: modified,
                    event_type: TimelineEventType::FileModified,
                    file_id: file.id,
                    description: format!(
                        "Modified: {}",
                        file.original_path
                            .as_ref()
                            .map(|p| p.display().to_string())
                            .unwrap_or_else(|| format!("inode_{}", file.inode_or_cluster))
                    ),
                });
            }

            // Deletion events (most important for recovery)
            if let Some(deleted) = file.deletion_time {
                events.push(TimelineEntry {
                    timestamp: deleted,
                    event_type: TimelineEventType::FileDeleted,
                    file_id: file.id,
                    description: format!(
                        "Deleted: {} ({} bytes, {:.0}% confidence)",
                        file.original_path
                            .as_ref()
                            .map(|p| p.display().to_string())
                            .unwrap_or_else(|| format!("inode_{}", file.inode_or_cluster)),
                        file.size,
                        file.confidence_score * 100.0
                    ),
                });
            }
        }

        // Sort chronologically
        events.sort_by_key(|e| e.timestamp);

        // Detect patterns
        let patterns = Self::detect_patterns(&events, &session.scan_results);

        // Generate statistics
        let statistics = Self::calculate_statistics(&events, &session.scan_results);

        RecoveryTimeline {
            events,
            patterns,
            statistics,
        }
    }

    /// Detect suspicious deletion patterns
    fn detect_patterns(events: &[TimelineEntry], files: &[DeletedFile]) -> Vec<DeletionPattern> {
        let mut patterns = Vec::new();

        // Pattern 1: Bulk deletion detection
        // Find windows where many files were deleted in short time
        let deletion_events: Vec<_> = events
            .iter()
            .filter(|e| matches!(e.event_type, TimelineEventType::FileDeleted))
            .collect();

        let mut processed_windows = Vec::new();

        for window_start in 0..deletion_events.len() {
            let start_time = deletion_events[window_start].timestamp;
            let mut files_in_window = vec![deletion_events[window_start].file_id];

            // Check if this window overlaps with already processed windows
            if processed_windows
                .iter()
                .any(|&(start, _): &(usize, usize)| {
                    window_start >= start && window_start < start + 5
                })
            {
                continue;
            }

            for event in deletion_events.iter().skip(window_start + 1) {
                if event.timestamp - start_time <= Duration::minutes(5) {
                    files_in_window.push(event.file_id);
                } else {
                    break;
                }
            }

            // If 5+ files deleted within 5 minutes, flag as bulk deletion
            if files_in_window.len() >= 5 {
                let end_time = deletion_events
                    .get(window_start + files_in_window.len() - 1)
                    .map(|e| e.timestamp)
                    .unwrap_or(start_time + Duration::minutes(5));

                patterns.push(DeletionPattern {
                    pattern_type: PatternType::BulkDeletion,
                    confidence: 0.9,
                    affected_files: files_in_window.clone(),
                    timeframe: (start_time, end_time),
                    description: format!(
                        "{} files deleted within 5 minutes starting at {}",
                        files_in_window.len(),
                        start_time.format("%Y-%m-%d %H:%M:%S")
                    ),
                });

                processed_windows.push((window_start, files_in_window.len()));
            }
        }

        // Pattern 2: Selective deletion by file type
        let mut type_deletions: HashMap<String, Vec<u64>> = HashMap::new();

        for event in deletion_events {
            if let Some(file) = files.iter().find(|f| f.id == event.file_id) {
                if let Some(mime) = &file.metadata.mime_type {
                    type_deletions
                        .entry(mime.clone())
                        .or_default()
                        .push(file.id);
                }
            }
        }

        for (mime_type, file_ids) in type_deletions {
            if file_ids.len() >= 3 {
                let timeframe = if !events.is_empty() {
                    (
                        events.first().unwrap().timestamp,
                        events.last().unwrap().timestamp,
                    )
                } else {
                    (Utc::now(), Utc::now())
                };

                patterns.push(DeletionPattern {
                    pattern_type: PatternType::SelectiveDeletion,
                    confidence: 0.7,
                    affected_files: file_ids.clone(),
                    timeframe,
                    description: format!(
                        "{} files of type '{}' were deleted",
                        file_ids.len(),
                        mime_type
                    ),
                });
            }
        }

        patterns
    }

    /// Calculate timeline statistics
    fn calculate_statistics(events: &[TimelineEntry], files: &[DeletedFile]) -> TimelineStatistics {
        let deletion_events: Vec<_> = events
            .iter()
            .filter(|e| matches!(e.event_type, TimelineEventType::FileDeleted))
            .collect();

        // Find peak deletion time (hour with most deletions)
        let mut hourly_deletions: HashMap<i64, usize> = HashMap::new();

        for event in &deletion_events {
            let hour = event.timestamp.timestamp() / 3600;
            *hourly_deletions.entry(hour).or_insert(0) += 1;
        }

        let peak_deletion_time = hourly_deletions
            .iter()
            .max_by_key(|(_, count)| *count)
            .and_then(|(hour, _)| DateTime::from_timestamp(*hour * 3600, 0));

        // Calculate average deletions per day
        let avg_deletions_per_day = if deletion_events.len() > 1 {
            let first = deletion_events.first().unwrap().timestamp;
            let last = deletion_events.last().unwrap().timestamp;
            let time_span_days = (last - first).num_days() as f32;
            deletion_events.len() as f32 / time_span_days.max(1.0)
        } else {
            deletion_events.len() as f32
        };

        // Count file types
        let mut file_types_affected = HashMap::new();
        for file in files {
            if let Some(mime) = &file.metadata.mime_type {
                *file_types_affected.entry(mime.clone()).or_insert(0) += 1;
            }
        }

        TimelineStatistics {
            total_events: events.len(),
            deletion_events: deletion_events.len(),
            peak_deletion_time,
            average_deletions_per_day: avg_deletions_per_day,
            file_types_affected,
        }
    }

    /// Export timeline as JSON
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self)
    }

    /// Export timeline as CSV
    pub fn to_csv(&self) -> String {
        let mut csv = String::from("Timestamp,Event Type,File ID,Description\n");
        for event in &self.events {
            csv.push_str(&format!(
                "{},{:?},{},{}\n",
                event.timestamp.to_rfc3339(),
                event.event_type,
                event.file_id,
                event.description.replace(',', ";")
            ));
        }
        csv
    }

    /// Generate human-readable text report
    pub fn to_text_report(&self) -> String {
        let mut report = String::new();

        report.push_str("═══════════════════════════════════════════════════════\n");
        report.push_str("           RECOVERY TIMELINE ANALYSIS\n");
        report.push_str("═══════════════════════════════════════════════════════\n\n");

        // Statistics
        report.push_str("📊 STATISTICS\n");
        report.push_str("───────────────────────────────────────────────────────\n");
        report.push_str(&format!("Total events: {}\n", self.statistics.total_events));
        report.push_str(&format!(
            "Deletion events: {}\n",
            self.statistics.deletion_events
        ));
        report.push_str(&format!(
            "Avg deletions/day: {:.1}\n",
            self.statistics.average_deletions_per_day
        ));

        if let Some(peak) = self.statistics.peak_deletion_time {
            report.push_str(&format!(
                "Peak deletion time: {}\n",
                peak.format("%Y-%m-%d %H:%M:%S")
            ));
        }

        report.push_str("\n📁 FILE TYPES AFFECTED\n");
        report.push_str("───────────────────────────────────────────────────────\n");
        let mut types: Vec<_> = self.statistics.file_types_affected.iter().collect();
        types.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
        for (mime_type, count) in types {
            report.push_str(&format!("  {} x {}\n", count, mime_type));
        }

        // Patterns
        if !self.patterns.is_empty() {
            report.push_str("\n⚠️  SUSPICIOUS PATTERNS DETECTED\n");
            report.push_str("───────────────────────────────────────────────────────\n");
            for (i, pattern) in self.patterns.iter().enumerate() {
                report.push_str(&format!(
                    "\n{}. {:?} (Confidence: {:.0}%)\n",
                    i + 1,
                    pattern.pattern_type,
                    pattern.confidence * 100.0
                ));
                report.push_str(&format!("   {}\n", pattern.description));
                report.push_str(&format!(
                    "   Timeframe: {} to {}\n",
                    pattern.timeframe.0.format("%Y-%m-%d %H:%M:%S"),
                    pattern.timeframe.1.format("%Y-%m-%d %H:%M:%S")
                ));
                report.push_str(&format!(
                    "   Affected files: {} files\n",
                    pattern.affected_files.len()
                ));
            }
        }

        // Event timeline
        report.push_str("\n📅 EVENT TIMELINE\n");
        report.push_str("───────────────────────────────────────────────────────\n");
        let display_count = self.events.len().min(50);
        for event in self.events.iter().take(display_count) {
            let icon = match event.event_type {
                TimelineEventType::FileCreated => "📝",
                TimelineEventType::FileModified => "✏️ ",
                TimelineEventType::FileDeleted => "🗑️ ",
                TimelineEventType::FileRecovered => "✅",
            };
            report.push_str(&format!(
                "{} {} - {}\n",
                event.timestamp.format("%Y-%m-%d %H:%M:%S"),
                icon,
                event.description
            ));
        }

        if self.events.len() > 50 {
            report.push_str(&format!(
                "\n... and {} more events\n",
                self.events.len() - 50
            ));
        }

        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FileMetadata, FileType};
    use std::path::PathBuf;

    #[test]
    fn test_empty_timeline() {
        let session = RecoverySession {
            id: uuid::Uuid::new_v4(),
            device_path: PathBuf::from("/dev/test"),
            fs_type: crate::FileSystemType::Xfs,
            created_at: Utc::now(),
            scan_results: vec![],
            total_scanned: 0,
            confidence_threshold: 0.5,
            metadata: crate::SessionMetadata {
                device_size: 1024 * 1024 * 1024,
                filesystem_size: 1024 * 1024 * 1024,
                block_size: 4096,
                scan_duration_ms: 0,
                files_found: 0,
                recoverable_files: 0,
            },
        };

        let timeline = RecoveryTimeline::from_session(&session);
        assert_eq!(timeline.events.len(), 0);
        assert_eq!(timeline.statistics.total_events, 0);
        assert_eq!(timeline.statistics.deletion_events, 0);
    }

    #[test]
    fn test_timeline_with_deletions() {
        let now = Utc::now();
        let files = vec![
            DeletedFile {
                id: 1,
                inode_or_cluster: 100,
                original_path: Some(PathBuf::from("file1.txt")),
                size: 1024,
                deletion_time: Some(now),
                confidence_score: 0.9,
                file_type: FileType::RegularFile,
                data_blocks: vec![],
                is_recoverable: true,
                metadata: FileMetadata {
                    mime_type: Some("text/plain".to_string()),
                    file_extension: Some("txt".to_string()),
                    permissions: None,
                    owner_uid: None,
                    owner_gid: None,
                    created_time: Some(now - Duration::days(1)),
                    modified_time: Some(now - Duration::hours(1)),
                    accessed_time: None,
                    extended_attributes: HashMap::new(),
                },
                fs_metadata: None,
            },
            DeletedFile {
                id: 2,
                inode_or_cluster: 101,
                original_path: Some(PathBuf::from("file2.txt")),
                size: 2048,
                deletion_time: Some(now + Duration::seconds(10)),
                confidence_score: 0.8,
                file_type: FileType::RegularFile,
                data_blocks: vec![],
                is_recoverable: true,
                metadata: FileMetadata {
                    mime_type: Some("text/plain".to_string()),
                    file_extension: Some("txt".to_string()),
                    permissions: None,
                    owner_uid: None,
                    owner_gid: None,
                    created_time: Some(now - Duration::days(2)),
                    modified_time: Some(now - Duration::hours(2)),
                    accessed_time: None,
                    extended_attributes: HashMap::new(),
                },
                fs_metadata: None,
            },
        ];

        let session = RecoverySession {
            id: uuid::Uuid::new_v4(),
            device_path: PathBuf::from("/dev/test"),
            fs_type: crate::FileSystemType::Xfs,
            created_at: now,
            scan_results: files,
            total_scanned: 2,
            confidence_threshold: 0.5,
            metadata: crate::SessionMetadata {
                device_size: 1024 * 1024 * 1024,
                filesystem_size: 1024 * 1024 * 1024,
                block_size: 4096,
                scan_duration_ms: 100,
                files_found: 2,
                recoverable_files: 2,
            },
        };

        let timeline = RecoveryTimeline::from_session(&session);

        // Should have creation, modification, and deletion events for each file
        assert!(timeline.events.len() >= 2); // At least deletion events
        assert_eq!(timeline.statistics.deletion_events, 2);
        assert!(timeline
            .statistics
            .file_types_affected
            .contains_key("text/plain"));
    }

    #[test]
    fn test_csv_export() {
        let session = RecoverySession {
            id: uuid::Uuid::new_v4(),
            device_path: PathBuf::from("/dev/test"),
            fs_type: crate::FileSystemType::Xfs,
            created_at: Utc::now(),
            scan_results: vec![],
            total_scanned: 0,
            confidence_threshold: 0.5,
            metadata: crate::SessionMetadata {
                device_size: 1024 * 1024 * 1024,
                filesystem_size: 1024 * 1024 * 1024,
                block_size: 4096,
                scan_duration_ms: 0,
                files_found: 0,
                recoverable_files: 0,
            },
        };

        let timeline = RecoveryTimeline::from_session(&session);
        let csv = timeline.to_csv();
        assert!(csv.starts_with("Timestamp,Event Type,File ID,Description"));
    }
}
