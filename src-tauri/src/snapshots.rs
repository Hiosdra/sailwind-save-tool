use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use chrono::{SecondsFormat, Utc};
use fs2::FileExt;
use rusqlite::{Connection, params};
use sha2::{Digest, Sha256};
use ulid::Ulid;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::error::CoreError;
use crate::models::{
    ManifestFile, RestoreJournal, RestoreResult, SnapshotAnnotation, SnapshotManifest,
    SnapshotSummary,
};
use crate::saves;
use crate::storage::write_json_atomic;

const MIGRATION: &str = include_str!("../migrations/001_initial.sql");
const MAX_ARCHIVE_SIZE: u64 = 256 * 1024 * 1024;
const MAX_EXPANDED_SIZE: u64 = 512 * 1024 * 1024;
const MAX_ARCHIVE_ENTRIES: usize = 1_024;
const RETENTION_THRESHOLD: usize = 50;

struct OperationLock {
    _file: fs::File,
}
impl OperationLock {
    fn acquire(app_data: &Path) -> Result<Self, CoreError> {
        fs::create_dir_all(app_data)?;
        let path = app_data.join("operation.lock");
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(CoreError::Io)?;
        file.try_lock_exclusive().map_err(|error| {
            if error.kind() == std::io::ErrorKind::WouldBlock {
                CoreError::OperationBusy
            } else {
                CoreError::Io(error)
            }
        })?;
        Ok(Self { _file: file })
    }
}

#[cfg(test)]
fn create_snapshot(
    save_root: &Path,
    app_data: &Path,
    slot: u8,
) -> Result<SnapshotSummary, CoreError> {
    let _lock = OperationLock::acquire(app_data)?;
    create_snapshot_unlocked(save_root, app_data, slot)
}

pub fn create_manual_snapshot(
    save_root: &Path,
    app_data: &Path,
    slot: u8,
    prune_oldest: bool,
) -> Result<SnapshotSummary, CoreError> {
    let _lock = OperationLock::acquire(app_data)?;
    let existing = list_snapshots(app_data, Some(slot))?;
    if existing.len() >= RETENTION_THRESHOLD && !prune_oldest {
        return Err(CoreError::RetentionConfirmationRequired(slot));
    }
    let created = create_snapshot_unlocked(save_root, app_data, slot)?;
    if prune_oldest {
        prune_to_threshold_unlocked(app_data, slot, &created.id)?;
    }
    Ok(created)
}

fn create_snapshot_unlocked(
    save_root: &Path,
    app_data: &Path,
    slot: u8,
) -> Result<SnapshotSummary, CoreError> {
    let entries = saves::bundle_entries(save_root, slot)?;
    let id = Ulid::new().to_string();
    let slot_root = app_data.join("snapshots").join(format!("slot{slot}"));
    fs::create_dir_all(&slot_root)?;
    let staging = slot_root.join(format!(".staging-{id}"));
    let final_path = slot_root.join(&id);
    fs::create_dir_all(staging.join("bundle"))?;

    let result = (|| {
        let mut files = Vec::with_capacity(entries.len());
        for entry in entries {
            let destination = staging.join("bundle").join(&entry.relative);
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)?;
            }
            copy_verified(&entry.source, &destination)?;
            files.push(ManifestFile {
                relative_path: relative_string(&entry.relative),
                size: fs::metadata(&destination)?.len(),
                sha256: hash_file(&destination)?,
            });
        }
        let created_at_utc = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        let manifest = SnapshotManifest {
            schema_version: 1,
            id: id.clone(),
            slot,
            created_at_utc: created_at_utc.clone(),
            source_directory: save_root.display().to_string(),
            files,
        };
        write_json(&staging.join("manifest.json"), &manifest)?;
        fs::rename(&staging, &final_path)?;

        let annotation = SnapshotAnnotation {
            schema_version: 1,
            snapshot_id: id.clone(),
            label: None,
            note: None,
            protected: false,
        };
        let annotations = app_data.join("annotations");
        fs::create_dir_all(&annotations)?;
        write_json_atomic(&annotations.join(format!("{id}.json")), &annotation)?;

        let summary = summary_from(&manifest, &annotation);
        index_snapshot(app_data, &final_path, &summary)?;
        Ok(summary)
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    result
}

pub fn is_sailwind_running() -> bool {
    let system = sysinfo::System::new_all();
    system.processes().values().any(|process| {
        process
            .name()
            .to_string_lossy()
            .eq_ignore_ascii_case("Sailwind.exe")
    })
}

pub fn restore_snapshot(
    save_root: &Path,
    app_data: &Path,
    snapshot_id: &str,
) -> Result<RestoreResult, CoreError> {
    if is_sailwind_running() {
        return Err(CoreError::GameRunning);
    }
    let _lock = OperationLock::acquire(app_data)?;
    recover_interrupted_unlocked(app_data)?;

    let (snapshot_path, manifest) = load_snapshot(app_data, snapshot_id)?;
    validate_snapshot(&snapshot_path, &manifest)?;

    let current_slot = saves::discover_slots(save_root)?
        .into_iter()
        .find(|slot| slot.slot == manifest.slot);
    let safety_snapshot_id = match current_slot {
        Some(slot) if slot.complete => {
            Some(create_snapshot_unlocked(save_root, app_data, manifest.slot)?.id)
        }
        Some(_) => return Err(CoreError::MissingPrimary(manifest.slot)),
        None => None,
    };

    let operation_id = Ulid::new().to_string();
    let staging = save_root.join(format!(".sailwind-restore-{operation_id}"));
    let rollback = save_root.join(format!(".sailwind-rollback-{operation_id}"));
    fs::create_dir(&staging)?;
    fs::create_dir(&rollback)?;

    let prepared = (|| {
        for file in &manifest.files {
            let relative = checked_relative_path(&file.relative_path, manifest.slot)?;
            let source = snapshot_path.join("bundle").join(&relative);
            let destination = staging.join(&relative);
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)?;
            }
            copy_verified(&source, &destination)?;
        }
        validate_files_at(&staging, &manifest, true)?;
        Ok::<_, CoreError>(())
    })();
    if let Err(error) = prepared {
        let _ = fs::remove_dir_all(&staging);
        let _ = fs::remove_dir_all(&rollback);
        return Err(error);
    }
    if is_sailwind_running() {
        let _ = fs::remove_dir_all(&staging);
        let _ = fs::remove_dir_all(&rollback);
        return Err(CoreError::GameRunning);
    }

    let original_artifacts = existing_artifact_names(save_root, manifest.slot)?;
    let operation_dir = app_data.join("operations");
    fs::create_dir_all(&operation_dir)?;
    let journal_path = operation_dir.join("active-restore.json");
    let marker_path = operation_dir.join("restore-mutation-started");
    let committed_marker_path = operation_dir.join("restore-committed");
    let journal = RestoreJournal {
        schema_version: 1,
        operation_id,
        save_directory: save_root.display().to_string(),
        slot: manifest.slot,
        staging_directory: staging.display().to_string(),
        rollback_directory: rollback.display().to_string(),
        original_artifacts: original_artifacts.clone(),
    };
    write_json_atomic(&journal_path, &journal)?;
    fs::File::create(&marker_path)?.sync_all()?;

    let installation = (|| {
        for name in &original_artifacts {
            fs::rename(save_root.join(name), rollback.join(name))?;
        }
        for name in existing_artifact_names(&staging, manifest.slot)? {
            fs::rename(staging.join(&name), save_root.join(&name))?;
        }
        validate_files_at(save_root, &manifest, false)?;
        Ok::<_, CoreError>(())
    })();

    if let Err(error) = installation {
        let recovery = rollback_from_journal(&journal);
        return match recovery {
            Ok(()) => {
                let _ = fs::remove_file(&marker_path);
                let _ = fs::remove_file(&journal_path);
                Err(error)
            }
            Err(recovery_error) => Err(CoreError::InvalidSnapshot(format!(
                "Restore failed ({error}); automatic rollback also failed ({recovery_error})"
            ))),
        };
    }

    fs::File::create(&committed_marker_path)?.sync_all()?;
    fs::remove_dir_all(&rollback)?;
    fs::remove_dir_all(&staging)?;
    fs::remove_file(marker_path)?;
    fs::remove_file(committed_marker_path)?;
    fs::remove_file(journal_path)?;
    Ok(RestoreResult {
        restored_snapshot_id: manifest.id,
        safety_snapshot_id,
        slot: manifest.slot,
    })
}

pub fn recover_interrupted(app_data: &Path) -> Result<bool, CoreError> {
    let _lock = OperationLock::acquire(app_data)?;
    recover_interrupted_unlocked(app_data)
}

fn recover_interrupted_unlocked(app_data: &Path) -> Result<bool, CoreError> {
    let operation_dir = app_data.join("operations");
    let journal_path = operation_dir.join("active-restore.json");
    if !journal_path.is_file() {
        return Ok(false);
    }
    let journal: RestoreJournal = serde_json::from_reader(fs::File::open(&journal_path)?)?;
    validate_restore_journal(&journal)?;
    let marker_path = operation_dir.join("restore-mutation-started");
    let committed_marker_path = operation_dir.join("restore-committed");
    if committed_marker_path.exists() {
        remove_directory_if_present(Path::new(&journal.staging_directory))?;
        remove_directory_if_present(Path::new(&journal.rollback_directory))?;
        fs::remove_file(committed_marker_path)?;
        remove_file_if_present(&marker_path)?;
    } else if marker_path.exists() {
        rollback_from_journal(&journal)?;
        fs::remove_file(marker_path)?;
    } else {
        remove_directory_if_present(Path::new(&journal.staging_directory))?;
        remove_directory_if_present(Path::new(&journal.rollback_directory))?;
    }
    fs::remove_file(journal_path)?;
    Ok(true)
}

fn rollback_from_journal(journal: &RestoreJournal) -> Result<(), CoreError> {
    validate_restore_journal(journal)?;
    let save_root = Path::new(&journal.save_directory);
    let staging = Path::new(&journal.staging_directory);
    let rollback = Path::new(&journal.rollback_directory);
    let valid_artifacts = all_artifact_names(journal.slot);
    for name in valid_artifacts {
        let live = save_root.join(&name);
        let previous = rollback.join(&name);
        if previous.exists() {
            remove_path_if_present(&live)?;
            fs::rename(previous, live)?;
        } else if !journal.original_artifacts.contains(&name) {
            remove_path_if_present(&live)?;
        }
    }
    remove_directory_if_present(staging)?;
    remove_directory_if_present(rollback)?;
    Ok(())
}

fn validate_restore_journal(journal: &RestoreJournal) -> Result<(), CoreError> {
    let save_root = Path::new(&journal.save_directory);
    let staging = Path::new(&journal.staging_directory);
    let rollback = Path::new(&journal.rollback_directory);
    let valid_artifacts = all_artifact_names(journal.slot);
    let expected_staging = format!(".sailwind-restore-{}", journal.operation_id);
    let expected_rollback = format!(".sailwind-rollback-{}", journal.operation_id);
    let valid_paths = journal.schema_version == 1
        && journal.slot <= 5
        && journal.operation_id.parse::<Ulid>().is_ok()
        && save_root.is_dir()
        && fs::symlink_metadata(save_root).is_ok_and(|metadata| !metadata.file_type().is_symlink())
        && staging.parent() == Some(save_root)
        && rollback.parent() == Some(save_root)
        && staging
            .file_name()
            .is_some_and(|name| name == std::ffi::OsStr::new(&expected_staging))
        && rollback
            .file_name()
            .is_some_and(|name| name == std::ffi::OsStr::new(&expected_rollback))
        && journal
            .original_artifacts
            .iter()
            .all(|name| valid_artifacts.contains(name));
    if !valid_paths {
        return Err(CoreError::InvalidSnapshot(
            "Interrupted-operation journal contains unsafe paths".to_owned(),
        ));
    }
    Ok(())
}

fn load_snapshot(
    app_data: &Path,
    snapshot_id: &str,
) -> Result<(PathBuf, SnapshotManifest), CoreError> {
    if snapshot_id.parse::<Ulid>().is_err() {
        return Err(CoreError::SnapshotNotFound(snapshot_id.to_owned()));
    }
    for slot in 0..=5 {
        let path = app_data
            .join("snapshots")
            .join(format!("slot{slot}"))
            .join(snapshot_id);
        if path.is_dir() {
            let manifest: SnapshotManifest =
                serde_json::from_reader(fs::File::open(path.join("manifest.json"))?)?;
            if manifest.id != snapshot_id || manifest.slot != slot {
                return Err(CoreError::InvalidSnapshot(
                    "Manifest identity does not match its storage path".to_owned(),
                ));
            }
            return Ok((path, manifest));
        }
    }
    Err(CoreError::SnapshotNotFound(snapshot_id.to_owned()))
}

fn validate_snapshot(path: &Path, manifest: &SnapshotManifest) -> Result<(), CoreError> {
    if manifest.schema_version != 1 || manifest.files.is_empty() {
        return Err(CoreError::InvalidSnapshot(
            "Unsupported or empty snapshot manifest".to_owned(),
        ));
    }
    validate_files_at(&path.join("bundle"), manifest, true)
}

fn validate_files_at(
    root: &Path,
    manifest: &SnapshotManifest,
    require_exact_contents: bool,
) -> Result<(), CoreError> {
    let mut seen = std::collections::BTreeSet::new();
    for file in &manifest.files {
        let relative = checked_relative_path(&file.relative_path, manifest.slot)?;
        if !seen.insert(relative.clone()) {
            return Err(CoreError::InvalidSnapshot(format!(
                "Duplicate manifest path: {}",
                file.relative_path
            )));
        }
        let path = root.join(relative);
        let metadata = fs::symlink_metadata(&path).map_err(|_| {
            CoreError::InvalidSnapshot(format!("Missing file: {}", file.relative_path))
        })?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(CoreError::InvalidSnapshot(format!(
                "Unsupported file type: {}",
                file.relative_path
            )));
        }
        if metadata.len() != file.size || hash_file(&path)? != file.sha256 {
            return Err(CoreError::InvalidSnapshot(format!(
                "Checksum or size mismatch: {}",
                file.relative_path
            )));
        }
    }
    if require_exact_contents && collect_relative_files(root)? != seen {
        return Err(CoreError::InvalidSnapshot(
            "Bundle contents do not exactly match the manifest".to_owned(),
        ));
    }
    Ok(())
}

fn collect_relative_files(root: &Path) -> Result<std::collections::BTreeSet<PathBuf>, CoreError> {
    fn visit(
        root: &Path,
        directory: &Path,
        files: &mut std::collections::BTreeSet<PathBuf>,
    ) -> Result<(), CoreError> {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)?;
            if metadata.file_type().is_symlink() {
                return Err(CoreError::SymbolicLink(path.display().to_string()));
            }
            if metadata.is_dir() {
                visit(root, &path, files)?;
            } else if metadata.is_file() {
                files.insert(path.strip_prefix(root).expect("child path").to_owned());
            } else {
                return Err(CoreError::InvalidSnapshot(format!(
                    "Unsupported bundle entry type: {}",
                    path.display()
                )));
            }
        }
        Ok(())
    }
    let mut files = std::collections::BTreeSet::new();
    visit(root, root, &mut files)?;
    Ok(files)
}

fn checked_relative_path(value: &str, slot: u8) -> Result<PathBuf, CoreError> {
    let path = PathBuf::from(value);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(CoreError::InvalidSnapshot(format!(
            "Unsafe manifest path: {value}"
        )));
    }
    let first = path
        .components()
        .next()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .ok_or_else(|| CoreError::InvalidSnapshot("Empty manifest path".to_owned()))?;
    if !all_artifact_names(slot).contains(&first) {
        return Err(CoreError::InvalidSnapshot(format!(
            "Path does not belong to slot {slot}: {value}"
        )));
    }
    Ok(path)
}

fn existing_artifact_names(root: &Path, slot: u8) -> Result<Vec<String>, CoreError> {
    let mut result = Vec::new();
    for name in all_artifact_names(slot) {
        let path = root.join(&name);
        match fs::symlink_metadata(&path) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    return Err(CoreError::SymbolicLink(path.display().to_string()));
                }
                result.push(name);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error.into());
            }
        }
    }
    Ok(result)
}

fn all_artifact_names(slot: u8) -> Vec<String> {
    let base = format!("slot{slot}");
    let mut names = vec![
        format!("{base}.save"),
        format!("{base}.save.meta"),
        format!("{base}.save.png"),
        base.clone(),
    ];
    names.extend((1..=5).map(|backup| format!("{base}_backup{backup}.save")));
    names
}

fn remove_path_if_present(path: &Path) -> Result<(), CoreError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(path)?,
        Ok(_) => fs::remove_file(path)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

fn remove_directory_if_present(path: &Path) -> Result<(), CoreError> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn remove_file_if_present(path: &Path) -> Result<(), CoreError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

pub fn list_snapshots(
    app_data: &Path,
    slot: Option<u8>,
) -> Result<Vec<SnapshotSummary>, CoreError> {
    rebuild_index(app_data)?;
    let connection = open_index(app_data)?;
    let sql = if slot.is_some() {
        "SELECT id, slot, created_at_utc, file_count, total_size FROM snapshots WHERE slot = ?1 ORDER BY created_at_utc DESC"
    } else {
        "SELECT id, slot, created_at_utc, file_count, total_size FROM snapshots ORDER BY created_at_utc DESC"
    };
    let mut statement = connection.prepare(sql)?;
    let map_row = |row: &rusqlite::Row<'_>| -> rusqlite::Result<(String, u8, String, usize, u64)> {
        Ok((
            row.get(0)?,
            row.get(1)?,
            row.get(2)?,
            row.get(3)?,
            row.get(4)?,
        ))
    };
    let rows = if let Some(slot) = slot {
        statement.query_map([slot], map_row)?
    } else {
        statement.query_map([], map_row)?
    };
    let rows = rows.collect::<Result<Vec<_>, _>>()?;
    rows.into_iter()
        .map(|(id, slot, created_at_utc, file_count, total_size)| {
            let annotation = read_annotation_or_default(app_data, &id)?;
            Ok(SnapshotSummary {
                id,
                slot,
                created_at_utc,
                file_count,
                total_size,
                label: annotation.label,
                note: annotation.note,
                protected: annotation.protected,
            })
        })
        .collect()
}

pub fn update_annotation(
    app_data: &Path,
    snapshot_id: &str,
    label: Option<String>,
    note: Option<String>,
    protected: bool,
) -> Result<SnapshotSummary, CoreError> {
    let _lock = OperationLock::acquire(app_data)?;
    let (snapshot_path, manifest) = load_snapshot(app_data, snapshot_id)?;
    validate_snapshot(&snapshot_path, &manifest)?;
    let label = normalize_text(label, 80, "label")?;
    let note = normalize_text(note, 2_000, "note")?;
    let annotation = SnapshotAnnotation {
        schema_version: 1,
        snapshot_id: snapshot_id.to_owned(),
        label,
        note,
        protected,
    };
    write_json_atomic(
        &app_data
            .join("annotations")
            .join(format!("{snapshot_id}.json")),
        &annotation,
    )?;
    Ok(summary_from(&manifest, &annotation))
}

pub fn delete_snapshot(app_data: &Path, snapshot_id: &str) -> Result<(), CoreError> {
    let _lock = OperationLock::acquire(app_data)?;
    delete_snapshot_unlocked(app_data, snapshot_id)
}

fn delete_snapshot_unlocked(app_data: &Path, snapshot_id: &str) -> Result<(), CoreError> {
    let (snapshot_path, _) = load_snapshot(app_data, snapshot_id)?;
    let annotation = read_annotation_or_default(app_data, snapshot_id)?;
    if annotation.protected {
        return Err(CoreError::SnapshotProtected(snapshot_id.to_owned()));
    }
    let deleting = snapshot_path.with_file_name(format!(".deleting-{snapshot_id}"));
    fs::rename(&snapshot_path, &deleting)?;
    let connection = open_index(app_data)?;
    connection.execute("DELETE FROM snapshots WHERE id = ?1", [snapshot_id])?;
    fs::remove_dir_all(deleting)?;
    let annotation_path = app_data
        .join("annotations")
        .join(format!("{snapshot_id}.json"));
    if annotation_path.exists() {
        fs::remove_file(annotation_path)?;
    }
    Ok(())
}

fn prune_to_threshold_unlocked(
    app_data: &Path,
    slot: u8,
    newest_id: &str,
) -> Result<(), CoreError> {
    let mut snapshots = list_snapshots(app_data, Some(slot))?;
    let mut remaining = snapshots.len();
    snapshots.reverse();
    for snapshot in snapshots {
        if remaining <= RETENTION_THRESHOLD {
            break;
        }
        if snapshot.id != newest_id && !snapshot.protected {
            delete_snapshot_unlocked(app_data, &snapshot.id)?;
            remaining -= 1;
        }
    }
    Ok(())
}

pub fn export_snapshot(
    app_data: &Path,
    snapshot_id: &str,
    destination: &Path,
) -> Result<(), CoreError> {
    let _lock = OperationLock::acquire(app_data)?;
    let (snapshot_path, manifest) = load_snapshot(app_data, snapshot_id)?;
    validate_snapshot(&snapshot_path, &manifest)?;
    let annotation = read_annotation_or_default(app_data, snapshot_id)?;
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = tempfile::NamedTempFile::new_in(
        destination
            .parent()
            .ok_or_else(|| CoreError::InvalidArchive("Export path has no parent".to_owned()))?,
    )?;
    let mut archive = ZipWriter::new(temporary);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o644);
    archive
        .start_file("manifest.json", options)
        .map_err(zip_error)?;
    serde_json::to_writer_pretty(&mut archive, &manifest)?;
    archive
        .start_file("annotation.json", options)
        .map_err(zip_error)?;
    serde_json::to_writer_pretty(&mut archive, &annotation)?;
    for file in &manifest.files {
        let relative = checked_relative_path(&file.relative_path, manifest.slot)?;
        archive
            .start_file(format!("bundle/{}", file.relative_path), options)
            .map_err(zip_error)?;
        let mut source = fs::File::open(snapshot_path.join("bundle").join(relative))?;
        std::io::copy(&mut source, &mut archive)?;
    }
    let temporary = archive.finish().map_err(zip_error)?;
    temporary
        .persist(destination)
        .map_err(|error| CoreError::Io(error.error))?;
    Ok(())
}

pub fn import_snapshot(app_data: &Path, source: &Path) -> Result<SnapshotSummary, CoreError> {
    if fs::metadata(source)?.len() > MAX_ARCHIVE_SIZE {
        return Err(CoreError::InvalidArchive(
            "Archive exceeds the 256 MB compressed-size limit".to_owned(),
        ));
    }
    let _lock = OperationLock::acquire(app_data)?;
    let mut archive = ZipArchive::new(fs::File::open(source)?).map_err(zip_error)?;
    if archive.len() > MAX_ARCHIVE_ENTRIES {
        return Err(CoreError::InvalidArchive(format!(
            "Archive contains more than {MAX_ARCHIVE_ENTRIES} entries"
        )));
    }
    let mut expanded_size = 0_u64;
    for index in 0..archive.len() {
        let file = archive.by_index(index).map_err(zip_error)?;
        expanded_size = expanded_size.checked_add(file.size()).ok_or_else(|| {
            CoreError::InvalidArchive("Expanded archive size overflow".to_owned())
        })?;
    }
    if expanded_size > MAX_EXPANDED_SIZE {
        return Err(CoreError::InvalidArchive(
            "Archive exceeds the 512 MB expanded-size limit".to_owned(),
        ));
    }

    let import_id = Ulid::new().to_string();
    let staging = app_data
        .join("imports")
        .join(format!(".staging-{import_id}"));
    fs::create_dir_all(&staging)?;
    let extraction = extract_archive(&mut archive, &staging);
    if let Err(error) = extraction {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }

    let validation = (|| {
        let manifest: SnapshotManifest = serde_json::from_reader(fs::File::open(
            staging.join("manifest.json"),
        )?)
        .map_err(|error| CoreError::InvalidArchive(format!("Invalid manifest JSON: {error}")))?;
        if manifest.id.parse::<Ulid>().is_err() || manifest.schema_version != 1 || manifest.slot > 5
        {
            return Err(CoreError::InvalidArchive(
                "Manifest has an invalid snapshot identity or schema".to_owned(),
            ));
        }
        validate_snapshot(&staging, &manifest)?;
        if load_snapshot(app_data, &manifest.id).is_ok() {
            return Err(CoreError::InvalidArchive(format!(
                "Snapshot {} already exists",
                manifest.id
            )));
        }
        let mut annotation: SnapshotAnnotation = if staging.join("annotation.json").is_file() {
            serde_json::from_reader(fs::File::open(staging.join("annotation.json"))?).map_err(
                |error| CoreError::InvalidArchive(format!("Invalid annotation JSON: {error}")),
            )?
        } else {
            default_annotation(&manifest.id)
        };
        if annotation.snapshot_id != manifest.id || annotation.schema_version != 1 {
            return Err(CoreError::InvalidArchive(
                "Annotation does not match the imported snapshot".to_owned(),
            ));
        }
        annotation.label = normalize_text(annotation.label, 80, "label")?;
        annotation.note = normalize_text(annotation.note, 2_000, "note")?;
        Ok::<_, CoreError>((manifest, annotation))
    })();
    let (manifest, annotation) = match validation {
        Ok(values) => values,
        Err(error) => {
            let _ = fs::remove_dir_all(&staging);
            return Err(error);
        }
    };

    let slot_root = app_data
        .join("snapshots")
        .join(format!("slot{}", manifest.slot));
    fs::create_dir_all(&slot_root)?;
    let final_path = slot_root.join(&manifest.id);
    if staging.join("annotation.json").exists() {
        fs::remove_file(staging.join("annotation.json"))?;
    }
    fs::rename(&staging, &final_path)?;
    write_json_atomic(
        &app_data
            .join("annotations")
            .join(format!("{}.json", manifest.id)),
        &annotation,
    )?;
    let summary = summary_from(&manifest, &annotation);
    index_snapshot(app_data, &final_path, &summary)?;
    Ok(summary)
}

fn extract_archive(
    archive: &mut ZipArchive<fs::File>,
    destination: &Path,
) -> Result<(), CoreError> {
    let mut seen = std::collections::BTreeSet::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(zip_error)?;
        let enclosed = entry.enclosed_name().ok_or_else(|| {
            CoreError::InvalidArchive(format!("Unsafe archive path: {}", entry.name()))
        })?;
        let normalized = relative_string(&enclosed);
        if !seen.insert(normalized.clone()) {
            return Err(CoreError::InvalidArchive(format!(
                "Duplicate archive path: {normalized}"
            )));
        }
        if entry
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err(CoreError::InvalidArchive(format!(
                "Symbolic link entry rejected: {normalized}"
            )));
        }
        if entry.unix_mode().is_some_and(|mode| {
            let file_type = mode & 0o170000;
            file_type != 0 && file_type != 0o100000 && file_type != 0o040000
        }) {
            return Err(CoreError::InvalidArchive(format!(
                "Unsupported archive entry type: {normalized}"
            )));
        }
        let allowed = normalized == "manifest.json"
            || normalized == "annotation.json"
            || normalized.starts_with("bundle/");
        if !allowed {
            return Err(CoreError::InvalidArchive(format!(
                "Unsupported archive entry: {normalized}"
            )));
        }
        if entry.is_dir() {
            continue;
        }
        let output = destination.join(enclosed);
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output)?;
        std::io::copy(&mut entry, &mut file)?;
        file.sync_all()?;
    }
    if !destination.join("manifest.json").is_file() {
        return Err(CoreError::InvalidArchive(
            "Archive has no manifest.json".to_owned(),
        ));
    }
    Ok(())
}

fn zip_error(error: zip::result::ZipError) -> CoreError {
    CoreError::InvalidArchive(error.to_string())
}

fn rebuild_index(app_data: &Path) -> Result<(), CoreError> {
    cleanup_pending_deletions(app_data)?;
    cleanup_abandoned_imports(app_data)?;
    let connection = open_index(app_data)?;
    connection.execute("DELETE FROM snapshots", [])?;
    let snapshots_root = app_data.join("snapshots");
    if !snapshots_root.exists() {
        return Ok(());
    }
    for slot_entry in fs::read_dir(snapshots_root)? {
        let slot_entry = slot_entry?;
        if !slot_entry.file_type()?.is_dir() {
            continue;
        }
        for snapshot_entry in fs::read_dir(slot_entry.path())? {
            let snapshot_entry = snapshot_entry?;
            if !snapshot_entry.file_type()?.is_dir()
                || snapshot_entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with('.')
            {
                continue;
            }
            let manifest_path = snapshot_entry.path().join("manifest.json");
            if !manifest_path.is_file() {
                continue;
            }
            let manifest: SnapshotManifest =
                serde_json::from_reader(fs::File::open(manifest_path)?)?;
            let annotation = read_annotation_or_default(app_data, &manifest.id)?;
            index_snapshot_with(
                &connection,
                &snapshot_entry.path(),
                &summary_from(&manifest, &annotation),
            )?;
        }
    }
    Ok(())
}

fn cleanup_pending_deletions(app_data: &Path) -> Result<(), CoreError> {
    let snapshots_root = app_data.join("snapshots");
    if !snapshots_root.exists() {
        return Ok(());
    }
    for slot_entry in fs::read_dir(snapshots_root)? {
        let slot_entry = slot_entry?;
        if !slot_entry.file_type()?.is_dir() {
            continue;
        }
        for entry in fs::read_dir(slot_entry.path())? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if entry.file_type()?.is_dir() && name.starts_with(".deleting-") {
                fs::remove_dir_all(entry.path())?;
                let id = name.trim_start_matches(".deleting-");
                let annotation = app_data.join("annotations").join(format!("{id}.json"));
                if annotation.exists() {
                    fs::remove_file(annotation)?;
                }
            }
        }
    }
    Ok(())
}

fn cleanup_abandoned_imports(app_data: &Path) -> Result<(), CoreError> {
    let imports_root = app_data.join("imports");
    if !imports_root.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(imports_root)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let staging_id = name.strip_prefix(".staging-");
        if entry.file_type()?.is_dir() && staging_id.is_some_and(|id| id.parse::<Ulid>().is_ok()) {
            fs::remove_dir_all(entry.path())?;
        }
    }
    Ok(())
}

fn normalize_text(
    value: Option<String>,
    maximum_characters: usize,
    field: &str,
) -> Result<Option<String>, CoreError> {
    let value = value
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty());
    if value
        .as_ref()
        .is_some_and(|text| text.chars().count() > maximum_characters)
    {
        return Err(CoreError::InvalidSnapshot(format!(
            "Snapshot {field} exceeds {maximum_characters} characters"
        )));
    }
    Ok(value)
}

fn open_index(app_data: &Path) -> Result<Connection, CoreError> {
    fs::create_dir_all(app_data)?;
    let connection = Connection::open(app_data.join("index.db"))?;
    connection.execute_batch(MIGRATION)?;
    Ok(connection)
}

fn index_snapshot(
    app_data: &Path,
    path: &Path,
    summary: &SnapshotSummary,
) -> Result<(), CoreError> {
    let connection = open_index(app_data)?;
    index_snapshot_with(&connection, path, summary)
}

fn index_snapshot_with(
    connection: &Connection,
    path: &Path,
    summary: &SnapshotSummary,
) -> Result<(), CoreError> {
    connection.execute(
        "INSERT INTO snapshots(id, slot, created_at_utc, file_count, total_size, snapshot_path) VALUES(?1, ?2, ?3, ?4, ?5, ?6) ON CONFLICT(id) DO UPDATE SET slot=excluded.slot, created_at_utc=excluded.created_at_utc, file_count=excluded.file_count, total_size=excluded.total_size, snapshot_path=excluded.snapshot_path",
        params![summary.id, summary.slot, summary.created_at_utc, summary.file_count, summary.total_size, path.display().to_string()],
    )?;
    Ok(())
}

fn summary_from(manifest: &SnapshotManifest, annotation: &SnapshotAnnotation) -> SnapshotSummary {
    SnapshotSummary {
        id: manifest.id.clone(),
        slot: manifest.slot,
        created_at_utc: manifest.created_at_utc.clone(),
        file_count: manifest.files.len(),
        total_size: manifest.files.iter().map(|file| file.size).sum(),
        label: annotation.label.clone(),
        note: annotation.note.clone(),
        protected: annotation.protected,
    }
}

fn default_annotation(id: &str) -> SnapshotAnnotation {
    SnapshotAnnotation {
        schema_version: 1,
        snapshot_id: id.to_owned(),
        label: None,
        note: None,
        protected: false,
    }
}

fn read_annotation(app_data: &Path, id: &str) -> Result<SnapshotAnnotation, CoreError> {
    let annotation: SnapshotAnnotation = serde_json::from_reader(fs::File::open(
        app_data.join("annotations").join(format!("{id}.json")),
    )?)?;
    if annotation.schema_version != 1 || annotation.snapshot_id != id {
        return Err(CoreError::InvalidSnapshot(format!(
            "Annotation identity does not match snapshot {id}"
        )));
    }
    Ok(annotation)
}

fn read_annotation_or_default(app_data: &Path, id: &str) -> Result<SnapshotAnnotation, CoreError> {
    match read_annotation(app_data, id) {
        Ok(annotation) => Ok(annotation),
        Err(CoreError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(default_annotation(id))
        }
        Err(error) => Err(error),
    }
}

fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<(), CoreError> {
    let mut file = fs::File::create(path)?;
    serde_json::to_writer_pretty(&mut file, value)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    Ok(())
}

fn copy_verified(source: &Path, destination: &Path) -> Result<(), CoreError> {
    fs::copy(source, destination)?;
    if hash_file(source)? != hash_file(destination)? {
        return Err(CoreError::InvalidSnapshot(format!(
            "Copy verification failed for {}",
            source.display()
        )));
    }
    Ok(())
}

fn hash_file(path: &Path) -> Result<String, CoreError> {
    let mut file = fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(hex::encode(hash.finalize()))
}

fn relative_string(path: &Path) -> String {
    path.components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_immutable_snapshot_and_rebuildable_index() {
        let source = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        fs::write(source.path().join("slot3.save"), b"main save").unwrap();
        fs::write(source.path().join("slot3.save.meta"), b"meta").unwrap();
        fs::create_dir(source.path().join("slot3")).unwrap();
        fs::write(source.path().join("slot3/mod.json"), b"{}").unwrap();

        let snapshot = create_snapshot(source.path(), data.path(), 3).unwrap();
        assert_eq!(snapshot.file_count, 3);
        let snapshot_path = data.path().join("snapshots/slot3").join(&snapshot.id);
        assert!(snapshot_path.join("manifest.json").is_file());
        assert!(snapshot_path.join("bundle/slot3/mod.json").is_file());

        fs::remove_file(data.path().join("index.db")).unwrap();
        let rebuilt = list_snapshots(data.path(), Some(3)).unwrap();
        assert_eq!(rebuilt, vec![snapshot]);
    }

    #[test]
    fn restore_creates_safety_snapshot_and_reconciles_bundle() {
        let source = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        fs::write(source.path().join("slot1.save"), b"original").unwrap();
        fs::create_dir(source.path().join("slot1")).unwrap();
        fs::write(source.path().join("slot1/mod.json"), b"old sidecar").unwrap();
        let original = create_snapshot(source.path(), data.path(), 1).unwrap();

        fs::write(source.path().join("slot1.save"), b"current").unwrap();
        fs::write(source.path().join("slot1/mod.json"), b"new sidecar").unwrap();
        fs::write(source.path().join("slot1_backup1.save"), b"new backup").unwrap();

        let restored = restore_snapshot(source.path(), data.path(), &original.id).unwrap();
        assert!(restored.safety_snapshot_id.is_some());
        assert_eq!(
            fs::read(source.path().join("slot1.save")).unwrap(),
            b"original"
        );
        assert_eq!(
            fs::read(source.path().join("slot1/mod.json")).unwrap(),
            b"old sidecar"
        );
        assert!(!source.path().join("slot1_backup1.save").exists());
        assert_eq!(list_snapshots(data.path(), Some(1)).unwrap().len(), 2);
    }

    #[test]
    fn restore_rejects_corrupted_snapshot_without_touching_live_save() {
        let source = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        fs::write(source.path().join("slot0.save"), b"original").unwrap();
        let snapshot = create_snapshot(source.path(), data.path(), 0).unwrap();
        fs::write(source.path().join("slot0.save"), b"current").unwrap();
        fs::write(
            data.path()
                .join("snapshots/slot0")
                .join(&snapshot.id)
                .join("bundle/slot0.save"),
            b"corrupted",
        )
        .unwrap();

        assert!(matches!(
            restore_snapshot(source.path(), data.path(), &snapshot.id),
            Err(CoreError::InvalidSnapshot(_))
        ));
        assert_eq!(
            fs::read(source.path().join("slot0.save")).unwrap(),
            b"current"
        );
        assert_eq!(list_snapshots(data.path(), Some(0)).unwrap().len(), 1);
    }

    #[test]
    fn durable_annotation_survives_index_rebuild_and_protects_deletion() {
        let source = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        fs::write(source.path().join("slot4.save"), b"save").unwrap();
        let snapshot = create_snapshot(source.path(), data.path(), 4).unwrap();

        update_annotation(
            data.path(),
            &snapshot.id,
            Some("Before long voyage".to_owned()),
            Some("Known-good state".to_owned()),
            true,
        )
        .unwrap();
        fs::remove_file(data.path().join("index.db")).unwrap();
        let rebuilt = list_snapshots(data.path(), Some(4)).unwrap();
        assert_eq!(rebuilt[0].label.as_deref(), Some("Before long voyage"));
        assert_eq!(rebuilt[0].note.as_deref(), Some("Known-good state"));
        assert!(matches!(
            delete_snapshot(data.path(), &snapshot.id),
            Err(CoreError::SnapshotProtected(_))
        ));

        update_annotation(data.path(), &snapshot.id, None, None, false).unwrap();
        delete_snapshot(data.path(), &snapshot.id).unwrap();
        assert!(list_snapshots(data.path(), Some(4)).unwrap().is_empty());
    }

    #[test]
    fn swbackup_export_and_import_round_trip() {
        let source = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        fs::write(source.path().join("slot2.save"), b"portable save").unwrap();
        let snapshot = create_snapshot(source.path(), data.path(), 2).unwrap();
        update_annotation(
            data.path(),
            &snapshot.id,
            Some("Exported voyage".to_owned()),
            None,
            false,
        )
        .unwrap();
        let archive = data.path().join("voyage.swbackup");
        export_snapshot(data.path(), &snapshot.id, &archive).unwrap();
        delete_snapshot(data.path(), &snapshot.id).unwrap();

        let imported = import_snapshot(data.path(), &archive).unwrap();
        assert_eq!(imported.id, snapshot.id);
        assert_eq!(imported.label.as_deref(), Some("Exported voyage"));
        assert_eq!(imported.file_count, 1);
    }

    #[test]
    fn swbackup_import_rejects_parent_traversal() {
        let data = tempfile::tempdir().unwrap();
        let archive_path = data.path().join("malicious.swbackup");
        let file = fs::File::create(&archive_path).unwrap();
        let mut archive = ZipWriter::new(file);
        archive
            .start_file("../outside.txt", SimpleFileOptions::default())
            .unwrap();
        archive.write_all(b"do not extract").unwrap();
        archive.finish().unwrap();

        let result = import_snapshot(data.path(), &archive_path);
        assert!(
            matches!(result, Err(CoreError::InvalidArchive(_))),
            "unexpected result: {result:?}"
        );
        assert!(!data.path().join("outside.txt").exists());
    }

    #[test]
    fn swbackup_import_rejects_duplicate_archive_paths() {
        let data = tempfile::tempdir().unwrap();
        let archive_path = data.path().join("duplicate.swbackup");
        let file = fs::File::create(&archive_path).unwrap();
        let mut archive = ZipWriter::new(file);
        archive
            .start_file("manifest.json", SimpleFileOptions::default())
            .unwrap();
        archive.write_all(b"{}").unwrap();
        archive
            .start_file("manifesz.json", SimpleFileOptions::default())
            .unwrap();
        archive.write_all(b"{}").unwrap();
        archive.finish().unwrap();
        let mut bytes = fs::read(&archive_path).unwrap();
        for start in 0..=bytes.len() - b"manifesz.json".len() {
            if &bytes[start..start + b"manifesz.json".len()] == b"manifesz.json" {
                bytes[start..start + b"manifest.json".len()].copy_from_slice(b"manifest.json");
            }
        }
        fs::write(&archive_path, bytes).unwrap();

        let result = import_snapshot(data.path(), &archive_path);
        assert!(
            matches!(result, Err(CoreError::InvalidArchive(_))),
            "unexpected result: {result:?}"
        );
    }

    #[test]
    fn swbackup_import_rejects_excessive_entry_count_before_extraction() {
        let data = tempfile::tempdir().unwrap();
        let archive_path = data.path().join("too-many.swbackup");
        let file = fs::File::create(&archive_path).unwrap();
        let mut archive = ZipWriter::new(file);
        for index in 0..=MAX_ARCHIVE_ENTRIES {
            archive
                .start_file(
                    format!("bundle/entry-{index}"),
                    SimpleFileOptions::default(),
                )
                .unwrap();
        }
        archive.finish().unwrap();

        assert!(matches!(
            import_snapshot(data.path(), &archive_path),
            Err(CoreError::InvalidArchive(_))
        ));
        assert!(!data.path().join("imports").exists());
    }

    #[test]
    fn swbackup_import_cleans_staging_after_checksum_failure() {
        let data = tempfile::tempdir().unwrap();
        let archive_path = data.path().join("bad-checksum.swbackup");
        let file = fs::File::create(&archive_path).unwrap();
        let mut archive = ZipWriter::new(file);
        let manifest = SnapshotManifest {
            schema_version: 1,
            id: Ulid::new().to_string(),
            slot: 0,
            created_at_utc: Utc::now().to_rfc3339(),
            source_directory: "excluded-from-import-trust".to_owned(),
            files: vec![ManifestFile {
                relative_path: "slot0.save".to_owned(),
                size: 4,
                sha256: "00".repeat(32),
            }],
        };
        archive
            .start_file("manifest.json", SimpleFileOptions::default())
            .unwrap();
        serde_json::to_writer(&mut archive, &manifest).unwrap();
        archive
            .start_file("bundle/slot0.save", SimpleFileOptions::default())
            .unwrap();
        archive.write_all(b"save").unwrap();
        archive.finish().unwrap();

        assert!(matches!(
            import_snapshot(data.path(), &archive_path),
            Err(CoreError::InvalidSnapshot(_))
        ));
        let imports = data.path().join("imports");
        assert!(
            !imports.exists() || fs::read_dir(imports).unwrap().next().is_none(),
            "failed imports must not leave staging directories"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn verified_copy_reports_device_out_of_space_without_changing_source() {
        let source = tempfile::NamedTempFile::new().unwrap();
        fs::write(source.path(), b"preserve me").unwrap();

        assert!(matches!(
            copy_verified(source.path(), Path::new("/dev/full")),
            Err(CoreError::Io(_))
        ));
        assert_eq!(fs::read(source.path()).unwrap(), b"preserve me");
    }

    #[test]
    fn manual_snapshot_requires_retention_confirmation_then_prunes() {
        let source = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        fs::write(source.path().join("slot5.save"), b"save").unwrap();
        for _ in 0..RETENTION_THRESHOLD {
            create_snapshot(source.path(), data.path(), 5).unwrap();
        }

        assert!(matches!(
            create_manual_snapshot(source.path(), data.path(), 5, false),
            Err(CoreError::RetentionConfirmationRequired(5))
        ));
        create_manual_snapshot(source.path(), data.path(), 5, true).unwrap();
        assert_eq!(
            list_snapshots(data.path(), Some(5)).unwrap().len(),
            RETENTION_THRESHOLD
        );
    }

    #[test]
    fn corrupted_annotation_never_disables_snapshot_protection_silently() {
        let source = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        fs::write(source.path().join("slot0.save"), b"save").unwrap();
        let snapshot = create_snapshot(source.path(), data.path(), 0).unwrap();
        fs::write(
            data.path()
                .join("annotations")
                .join(format!("{}.json", snapshot.id)),
            b"not valid json",
        )
        .unwrap();

        assert!(list_snapshots(data.path(), Some(0)).is_err());
        assert!(delete_snapshot(data.path(), &snapshot.id).is_err());
        assert!(
            data.path()
                .join("snapshots/slot0")
                .join(snapshot.id)
                .is_dir()
        );
    }

    #[test]
    fn committed_restore_recovery_keeps_verified_live_bundle() {
        let save = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let operation_id = Ulid::new().to_string();
        let staging = save
            .path()
            .join(format!(".sailwind-restore-{operation_id}"));
        let rollback = save
            .path()
            .join(format!(".sailwind-rollback-{operation_id}"));
        fs::create_dir(&staging).unwrap();
        fs::create_dir(&rollback).unwrap();
        fs::write(save.path().join("slot1.save"), b"new verified save").unwrap();
        fs::write(rollback.join("slot1.save"), b"previous save").unwrap();

        let operation_dir = data.path().join("operations");
        fs::create_dir(&operation_dir).unwrap();
        let journal = RestoreJournal {
            schema_version: 1,
            operation_id,
            save_directory: save.path().display().to_string(),
            slot: 1,
            staging_directory: staging.display().to_string(),
            rollback_directory: rollback.display().to_string(),
            original_artifacts: vec!["slot1.save".to_owned()],
        };
        write_json_atomic(&operation_dir.join("active-restore.json"), &journal).unwrap();
        fs::write(operation_dir.join("restore-mutation-started"), b"").unwrap();
        fs::write(operation_dir.join("restore-committed"), b"").unwrap();

        assert!(recover_interrupted(data.path()).unwrap());
        assert_eq!(
            fs::read(save.path().join("slot1.save")).unwrap(),
            b"new verified save"
        );
        assert!(!rollback.exists());
        assert!(!staging.exists());
    }

    #[test]
    fn pre_mutation_restore_recovery_only_discards_staging() {
        let save = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let operation_id = Ulid::new().to_string();
        let staging = save
            .path()
            .join(format!(".sailwind-restore-{operation_id}"));
        let rollback = save
            .path()
            .join(format!(".sailwind-rollback-{operation_id}"));
        fs::create_dir(&staging).unwrap();
        fs::create_dir(&rollback).unwrap();
        fs::write(staging.join("slot3.save"), b"prepared").unwrap();
        fs::write(save.path().join("slot3.save"), b"live untouched").unwrap();

        let operation_dir = data.path().join("operations");
        fs::create_dir(&operation_dir).unwrap();
        let journal = RestoreJournal {
            schema_version: 1,
            operation_id,
            save_directory: save.path().display().to_string(),
            slot: 3,
            staging_directory: staging.display().to_string(),
            rollback_directory: rollback.display().to_string(),
            original_artifacts: vec!["slot3.save".to_owned()],
        };
        write_json_atomic(&operation_dir.join("active-restore.json"), &journal).unwrap();

        assert!(recover_interrupted(data.path()).unwrap());
        assert_eq!(
            fs::read(save.path().join("slot3.save")).unwrap(),
            b"live untouched"
        );
        assert!(!staging.exists());
        assert!(!rollback.exists());
    }

    #[test]
    fn uncommitted_restore_recovery_rolls_back_previous_bundle() {
        let save = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let operation_id = Ulid::new().to_string();
        let staging = save
            .path()
            .join(format!(".sailwind-restore-{operation_id}"));
        let rollback = save
            .path()
            .join(format!(".sailwind-rollback-{operation_id}"));
        fs::create_dir(&staging).unwrap();
        fs::create_dir(&rollback).unwrap();
        fs::write(save.path().join("slot2.save"), b"partially installed").unwrap();
        fs::write(rollback.join("slot2.save"), b"previous save").unwrap();

        let operation_dir = data.path().join("operations");
        fs::create_dir(&operation_dir).unwrap();
        let journal = RestoreJournal {
            schema_version: 1,
            operation_id,
            save_directory: save.path().display().to_string(),
            slot: 2,
            staging_directory: staging.display().to_string(),
            rollback_directory: rollback.display().to_string(),
            original_artifacts: vec!["slot2.save".to_owned()],
        };
        write_json_atomic(&operation_dir.join("active-restore.json"), &journal).unwrap();
        fs::write(operation_dir.join("restore-mutation-started"), b"").unwrap();

        assert!(recover_interrupted(data.path()).unwrap());
        assert_eq!(
            fs::read(save.path().join("slot2.save")).unwrap(),
            b"previous save"
        );
        assert!(!rollback.exists());
        assert!(!staging.exists());
    }

    #[test]
    fn interrupted_restore_rejects_journal_paths_outside_save_directory() {
        let save = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let operation_id = Ulid::new().to_string();
        let rollback = save
            .path()
            .join(format!(".sailwind-rollback-{operation_id}"));
        fs::create_dir(&rollback).unwrap();
        let protected_file = outside.path().join("must-not-be-removed");
        fs::write(&protected_file, b"safe").unwrap();

        let operation_dir = data.path().join("operations");
        fs::create_dir(&operation_dir).unwrap();
        let journal = RestoreJournal {
            schema_version: 1,
            operation_id,
            save_directory: save.path().display().to_string(),
            slot: 0,
            staging_directory: outside.path().display().to_string(),
            rollback_directory: rollback.display().to_string(),
            original_artifacts: Vec::new(),
        };
        write_json_atomic(&operation_dir.join("active-restore.json"), &journal).unwrap();

        assert!(matches!(
            recover_interrupted(data.path()),
            Err(CoreError::InvalidSnapshot(_))
        ));
        assert_eq!(fs::read(protected_file).unwrap(), b"safe");
    }
}
