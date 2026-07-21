use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactSummary {
    pub relative_path: String,
    pub kind: ArtifactKind,
    pub size: u64,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ArtifactKind {
    Primary,
    Metadata,
    Thumbnail,
    GameBackup,
    Sidecar,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SlotSummary {
    pub slot: u8,
    pub complete: bool,
    pub total_size: u64,
    pub artifacts: Vec<ArtifactSummary>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestFile {
    pub relative_path: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotManifest {
    pub schema_version: u32,
    pub id: String,
    pub slot: u8,
    pub created_at_utc: String,
    pub source_directory: String,
    pub files: Vec<ManifestFile>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotAnnotation {
    pub schema_version: u32,
    pub snapshot_id: String,
    pub label: Option<String>,
    pub note: Option<String>,
    pub protected: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotSummary {
    pub id: String,
    pub slot: u8,
    pub created_at_utc: String,
    pub file_count: usize,
    pub total_size: u64,
    pub label: Option<String>,
    pub note: Option<String>,
    pub protected: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RestoreResult {
    pub restored_snapshot_id: String,
    pub safety_snapshot_id: Option<String>,
    pub slot: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreJournal {
    pub schema_version: u32,
    pub operation_id: String,
    pub save_directory: String,
    pub slot: u8,
    pub staging_directory: String,
    pub rollback_directory: String,
    pub original_artifacts: Vec<String>,
}
