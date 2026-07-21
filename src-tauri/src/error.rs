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
    #[error("Diagnostic data is invalid: {0}")]
    InvalidDiagnostics(String),
}

impl CoreError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidSaveDirectory => "invalid_save_directory",
            Self::MissingPrimary(_) => "missing_primary_save",
            Self::SymbolicLink(_) => "symbolic_link_rejected",
            Self::OperationBusy => "operation_busy",
            Self::GameRunning => "game_running",
            Self::SnapshotNotFound(_) => "snapshot_not_found",
            Self::SnapshotProtected(_) => "snapshot_protected",
            Self::RetentionConfirmationRequired(_) => "retention_confirmation_required",
            Self::Io(_) => "filesystem_error",
            Self::Database(_) => "database_error",
            Self::Json(_) => "serialization_error",
            Self::InvalidSnapshot(_) => "invalid_snapshot",
            Self::InvalidSettings(_) => "invalid_settings",
            Self::InvalidArchive(_) => "invalid_archive",
            Self::InvalidDiagnostics(_) => "invalid_diagnostics",
        }
    }
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
        let code = error.code();
        Self {
            code,
            message: error.to_string(),
            context: BTreeMap::new(),
        }
    }
}
