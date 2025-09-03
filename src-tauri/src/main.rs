// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::process::Command;
use tauri::{State, Emitter};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use std::sync::Mutex;
use std::collections::HashMap;

// Import our GhostFS core functionality
use ghostfs_core::{FileSystemType, scan_and_analyze, recover_files, RecoverySession};

#[derive(Debug, Serialize, Deserialize)]
struct ScanProgress {
    progress: f32,
    message: String,
    files_found: u32,
    recoverable_files: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct RecoveryResult {
    success: bool,
    message: String,
    files_recovered: usize,
    total_size: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct SessionInfo {
    id: String,
    fs_type: String,
    device_path: String,
    created_at: DateTime<Utc>,
    files_found: u32,
    recoverable_files: u32,
    confidence_threshold: f32,
}

// Application state
struct AppState {
    sessions: Mutex<HashMap<String, RecoverySession>>,
}

#[tauri::command]
async fn detect_filesystem(image_path: String) -> Result<String, String> {
    let path = PathBuf::from(&image_path);
    
    match ghostfs_core::fs::detect_filesystem(&path) {
        Ok(Some(fs_type)) => Ok(fs_type.to_string()),
        Ok(None) => Err("Unknown or unsupported file system".to_string()),
        Err(e) => Err(format!("Detection failed: {}", e)),
    }
}

#[tauri::command]
async fn get_filesystem_info(image_path: String, fs_type: String) -> Result<String, String> {
    let path = PathBuf::from(&image_path);
    let filesystem_type = match fs_type.as_str() {
        "XFS" => FileSystemType::Xfs,
        "Btrfs" => FileSystemType::Btrfs,
        "exFAT" => FileSystemType::ExFat,
        _ => return Err("Unsupported filesystem type".to_string()),
    };
    
    match ghostfs_core::fs::get_filesystem_info(&path, filesystem_type) {
        Ok(info) => Ok(info),
        Err(e) => Err(format!("Failed to get filesystem info: {}", e)),
    }
}

#[tauri::command]
async fn start_scan(
    image_path: String,
    fs_type: String,
    confidence: f32,
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<SessionInfo, String> {
    let path = PathBuf::from(&image_path);
    let filesystem_type = match fs_type.as_str() {
        "xfs" => FileSystemType::Xfs,
        "btrfs" => FileSystemType::Btrfs,
        "exfat" => FileSystemType::ExFat,
        _ => return Err("Unsupported filesystem type".to_string()),
    };
    
    // Emit progress updates during scan
    let app_handle_clone = app_handle.clone();
    tokio::spawn(async move {
        // Simulate progress updates
        let progress_steps = vec![
            (10.0, "Initializing recovery engine..."),
            (25.0, "Analyzing file system structure..."),
            (40.0, "Scanning directory tables..."),
            (60.0, "Scanning for file signatures..."),
            (80.0, "Reconstructing metadata..."),
            (95.0, "Calculating confidence scores..."),
            (100.0, "Scan complete!"),
        ];
        
        for (progress, message) in progress_steps {
            let scan_progress = ScanProgress {
                progress,
                message: message.to_string(),
                files_found: (progress / 10.0) as u32,
                recoverable_files: ((progress / 10.0) * confidence) as u32,
            };
            
            let _ = app_handle_clone.emit("scan-progress", &scan_progress);
            tokio::time::sleep(tokio::time::Duration::from_millis(800)).await;
        }
    });
    
    // Perform actual scan
    match scan_and_analyze(&path, filesystem_type, confidence) {
        Ok(session) => {
            let session_info = SessionInfo {
                id: session.id.to_string(),
                fs_type: session.fs_type.to_string(),
                device_path: session.device_path.to_string_lossy().to_string(),
                created_at: session.created_at,
                files_found: session.metadata.files_found,
                recoverable_files: session.metadata.recoverable_files,
                confidence_threshold: session.confidence_threshold,
            };
            
            // Store session in state
            let mut sessions = state.sessions.lock().unwrap();
            sessions.insert(session.id.to_string(), session);
            
            Ok(session_info)
        }
        Err(e) => Err(format!("Scan failed: {}", e)),
    }
}

#[tauri::command]
async fn recover_session_files(
    session_id: String,
    output_dir: String,
    file_ids: Option<Vec<u64>>,
    state: State<'_, AppState>,
) -> Result<RecoveryResult, String> {
    let sessions = state.sessions.lock().unwrap();
    let session = sessions.get(&session_id)
        .ok_or("Session not found")?;
    
    let image_path = &session.device_path;
    let output_path = PathBuf::from(&output_dir);
    
    match recover_files(image_path, session, &output_path, file_ids) {
        Ok(report) => Ok(RecoveryResult {
            success: true,
            message: format!("Successfully recovered {} files", report.recovered_files),
            files_recovered: report.recovered_files,
            total_size: report.total_bytes_recovered,
        }),
        Err(e) => Err(format!("Recovery failed: {}", e)),
    }
}

#[tauri::command]
async fn get_session_files(session_id: String, state: State<'_, AppState>) -> Result<Vec<serde_json::Value>, String> {
    let sessions = state.sessions.lock().unwrap();
    let session = sessions.get(&session_id)
        .ok_or("Session not found")?;
    
    let files: Vec<serde_json::Value> = session.scan_results.iter().map(|file| {
        serde_json::json!({
            "id": file.id,
            "name": file.original_path.as_ref()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or(&format!("recovered_file_{}", file.id)),
            "size": file.size,
            "confidence": file.confidence_score,
            "type": match file.file_type {
                ghostfs_core::FileType::RegularFile => "file",
                ghostfs_core::FileType::Directory => "directory",
                _ => "unknown"
            },
            "is_recoverable": file.is_recoverable
        })
    }).collect();
    
    Ok(files)
}

#[tauri::command]
async fn open_file_dialog() -> Result<Option<String>, String> {
    // Note: File dialogs are handled on the frontend with @tauri-apps/api
    // This is a placeholder for backend file operations
    Ok(None)
}

#[tauri::command]
async fn open_folder_dialog() -> Result<Option<String>, String> {
    // Note: Folder dialogs are handled on the frontend with @tauri-apps/api
    // This is a placeholder for backend folder operations
    Ok(None)
}

#[tauri::command]
async fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

fn main() {
    tauri::Builder::default()
        .manage(AppState {
            sessions: Mutex::new(HashMap::new()),
        })
        .invoke_handler(tauri::generate_handler![
            detect_filesystem,
            get_filesystem_info,
            start_scan,
            recover_session_files,
            get_session_files,
            open_file_dialog,
            open_folder_dialog,
            get_app_version
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
