use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use chrono::{SecondsFormat, Utc};
use fs2::FileExt;
use serde::{Deserialize, Serialize};

use crate::error::CoreError;
use crate::storage::write_json_atomic;

const LOG_SCHEMA_VERSION: u32 = 1;
const REPORT_RECORD_LIMIT: usize = 50;
const RETAINED_RECORD_LIMIT: usize = 500;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OperationRecord {
    pub schema_version: u32,
    pub occurred_at_utc: String,
    pub operation: String,
    pub component: String,
    pub severity: String,
    pub event_code: String,
    pub outcome: String,
    pub error_code: Option<String>,
    pub slot: Option<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryNotice {
    pub schema_version: u32,
    pub occurred_at_utc: String,
    pub outcome: String,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticReport {
    pub schema_version: u32,
    pub generated_at_utc: String,
    pub app_version: String,
    pub operating_system: String,
    pub architecture: String,
    pub privacy_note: String,
    pub recovery_notice: Option<RecoveryNotice>,
    pub recent_operations: Vec<OperationRecord>,
}

pub fn record_result<T>(
    app_data: &Path,
    operation: &str,
    slot: Option<u8>,
    result: &Result<T, CoreError>,
) {
    let (outcome, error_code) = match result {
        Ok(_) => ("succeeded", None),
        Err(error) => ("failed", Some(error.code().to_owned())),
    };
    let record = OperationRecord {
        schema_version: LOG_SCHEMA_VERSION,
        occurred_at_utc: now(),
        operation: operation.to_owned(),
        component: "snapshot_engine".to_owned(),
        severity: if result.is_ok() { "info" } else { "error" }.to_owned(),
        event_code: format!("{operation}_{outcome}"),
        outcome: outcome.to_owned(),
        error_code,
        slot,
    };
    if let Err(error) = append_record(app_data, &record) {
        eprintln!("Failed to write privacy-safe diagnostic record: {error}");
    }
}

pub fn record_recovery(app_data: &Path, result: &Result<bool, CoreError>) {
    if !matches!(result, Ok(false)) {
        record_result(app_data, "startup_recovery", None, result);
    }
    let notice = match result {
        Ok(true) => Some(RecoveryNotice {
            schema_version: LOG_SCHEMA_VERSION,
            occurred_at_utc: now(),
            outcome: "succeeded".to_owned(),
            message: "An interrupted restore was detected and recovered safely.".to_owned(),
        }),
        Err(error) => Some(RecoveryNotice {
            schema_version: LOG_SCHEMA_VERSION,
            occurred_at_utc: now(),
            outcome: "failed".to_owned(),
            message: format!(
                "An interrupted restore needs attention (diagnostic code: {}).",
                error.code()
            ),
        }),
        Ok(false) => None,
    };
    if let Some(notice) = notice
        && let Err(error) = write_json_atomic(&notice_path(app_data), &notice)
    {
        eprintln!("Failed to persist recovery notice: {error}");
    }
}

pub fn report(app_data: &Path) -> Result<DiagnosticReport, CoreError> {
    Ok(DiagnosticReport {
        schema_version: LOG_SCHEMA_VERSION,
        generated_at_utc: now(),
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
        operating_system: std::env::consts::OS.to_owned(),
        architecture: std::env::consts::ARCH.to_owned(),
        privacy_note:
            "Save paths, user names, snapshot labels, notes and save contents are excluded."
                .to_owned(),
        recovery_notice: read_notice(app_data)?,
        recent_operations: read_records(app_data, REPORT_RECORD_LIMIT)?,
    })
}

pub fn acknowledge_recovery(app_data: &Path) -> Result<(), CoreError> {
    match fs::remove_file(notice_path(app_data)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn append_record(app_data: &Path, record: &OperationRecord) -> Result<(), CoreError> {
    fs::create_dir_all(app_data)?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(app_data.join("diagnostics.lock"))?;
    lock.lock_exclusive()?;
    let path = log_path(app_data);
    repair_partial_tail(&path)?;
    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    let mut line = serde_json::to_vec(record)?;
    line.push(b'\n');
    file.write_all(&line)?;
    file.sync_all()?;
    drop(file);

    let records = read_records(app_data, RETAINED_RECORD_LIMIT + 1)?;
    if records.len() > RETAINED_RECORD_LIMIT {
        let retained = &records[records.len() - RETAINED_RECORD_LIMIT..];
        let mut temporary = tempfile::NamedTempFile::new_in(app_data)?;
        for item in retained {
            serde_json::to_writer(&mut temporary, item)?;
            temporary.write_all(b"\n")?;
        }
        temporary.as_file().sync_all()?;
        temporary
            .persist(path)
            .map_err(|error| CoreError::Io(error.error))?;
    }
    Ok(())
}

fn repair_partial_tail(path: &Path) -> Result<(), CoreError> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    if bytes.is_empty() || bytes.last() == Some(&b'\n') {
        return Ok(());
    }
    let complete_length = bytes
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |position| position + 1);
    OpenOptions::new()
        .write(true)
        .open(path)?
        .set_len(complete_length as u64)?;
    Ok(())
}

fn read_records(app_data: &Path, limit: usize) -> Result<Vec<OperationRecord>, CoreError> {
    let path = log_path(app_data);
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    let lines = BufReader::new(file)
        .lines()
        .collect::<Result<Vec<_>, _>>()?;
    let mut records = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let record: OperationRecord = match serde_json::from_str(line) {
            Ok(record) => record,
            Err(_) if index + 1 == lines.len() => break,
            Err(error) => return Err(error.into()),
        };
        if record.schema_version != LOG_SCHEMA_VERSION {
            return Err(CoreError::InvalidDiagnostics(
                "Unsupported operation-log schema version".to_owned(),
            ));
        }
        records.push(record);
    }
    if records.len() > limit {
        records.drain(..records.len() - limit);
    }
    Ok(records)
}

fn read_notice(app_data: &Path) -> Result<Option<RecoveryNotice>, CoreError> {
    let path = notice_path(app_data);
    match fs::File::open(path) {
        Ok(file) => {
            let notice: RecoveryNotice = serde_json::from_reader(file)?;
            if notice.schema_version != LOG_SCHEMA_VERSION {
                return Err(CoreError::InvalidDiagnostics(
                    "Unsupported recovery-notice schema version".to_owned(),
                ));
            }
            Ok(Some(notice))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn log_path(app_data: &Path) -> std::path::PathBuf {
    app_data.join("operations.jsonl")
}

fn notice_path(app_data: &Path) -> std::path::PathBuf {
    app_data.join("recovery-notice.json")
}

fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_contains_only_bounded_privacy_safe_records() {
        let data = tempfile::tempdir().unwrap();
        for slot in 0..60 {
            let result: Result<(), CoreError> = Ok(());
            record_result(data.path(), "create_snapshot", Some(slot % 6), &result);
        }

        let report = report(data.path()).unwrap();
        assert_eq!(report.recent_operations.len(), REPORT_RECORD_LIMIT);
        let serialized = serde_json::to_string(&report).unwrap();
        assert!(!serialized.contains(data.path().to_string_lossy().as_ref()));
        assert!(serialized.contains("Save paths"));
    }

    #[test]
    fn recovery_notice_persists_until_acknowledged() {
        let data = tempfile::tempdir().unwrap();
        record_recovery(data.path(), &Ok(true));
        assert_eq!(
            report(data.path())
                .unwrap()
                .recovery_notice
                .unwrap()
                .outcome,
            "succeeded"
        );

        acknowledge_recovery(data.path()).unwrap();
        assert!(report(data.path()).unwrap().recovery_notice.is_none());
    }

    #[test]
    fn report_ignores_a_partial_final_jsonl_record_after_a_crash() {
        let data = tempfile::tempdir().unwrap();
        let result: Result<(), CoreError> = Ok(());
        record_result(data.path(), "create_snapshot", Some(1), &result);
        let mut file = OpenOptions::new()
            .append(true)
            .open(log_path(data.path()))
            .unwrap();
        file.write_all(b"{\"schemaVersion\":").unwrap();
        drop(file);

        let next_result: Result<(), CoreError> = Err(CoreError::OperationBusy);
        record_result(data.path(), "restore_snapshot", Some(1), &next_result);

        let report = report(data.path()).unwrap();
        assert_eq!(report.recent_operations.len(), 2);
        assert_eq!(
            report.recent_operations[0].event_code,
            "create_snapshot_succeeded"
        );
        assert_eq!(
            report.recent_operations[1].event_code,
            "restore_snapshot_failed"
        );
    }
}
