use serde::Serialize;
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("The selected save directory does not exist or is not a directory")]
    InvalidSaveDirectory,
    #[error("Slot {0} has no primary save file")]
    MissingPrimary(u8),
    #[error("A symbolic link or unsupported file redirect was found at {0}")]
    SymbolicLink(String),
    #[error("Another save operation is already in progress")]
    OperationBusy,
    #[error("Sailwind is running. Close the game before restoring a snapshot")]
    GameRunning,
    #[error("Snapshot {0} was not found")]
    SnapshotNotFound(String),
    #[error("Snapshot {0} is protected and cannot be deleted")]
    SnapshotProtected(String),
    #[error("Slot {0} reached the snapshot retention threshold")]
    RetentionConfirmationRequired(u8),
    #[error("Filesystem operation failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("Snapshot index operation failed: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("Manifest serialization failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Snapshot data is invalid: {0}")]
    InvalidSnapshot(String),
    #[error("Application settings are invalid: {0}")]
    InvalidSettings(String),
    #[error("Backup archive is invalid: {0}")]
    InvalidArchive(String),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub context: BTreeMap<String, String>,
}

impl From<CoreError> for AppError {
    fn from(error: CoreError) -> Self {
        let code = match &error {
            CoreError::InvalidSaveDirectory => "invalid_save_directory",
            CoreError::MissingPrimary(_) => "missing_primary_save",
            CoreError::SymbolicLink(_) => "symbolic_link_rejected",
            CoreError::OperationBusy => "operation_busy",
            CoreError::GameRunning => "game_running",
            CoreError::SnapshotNotFound(_) => "snapshot_not_found",
            CoreError::SnapshotProtected(_) => "snapshot_protected",
            CoreError::RetentionConfirmationRequired(_) => "retention_confirmation_required",
            CoreError::Io(_) => "filesystem_error",
            CoreError::Database(_) => "database_error",
            CoreError::Json(_) => "serialization_error",
            CoreError::InvalidSnapshot(_) => "invalid_snapshot",
            CoreError::InvalidSettings(_) => "invalid_settings",
            CoreError::InvalidArchive(_) => "invalid_archive",
        };
        Self {
            code,
            message: error.to_string(),
            context: BTreeMap::new(),
        }
    }
}
