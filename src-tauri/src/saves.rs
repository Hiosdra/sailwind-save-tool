use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::CoreError;
use crate::models::{ArtifactKind, ArtifactSummary, SlotSummary};

#[derive(Clone, Debug)]
pub struct BundleEntry {
    pub source: PathBuf,
    pub relative: PathBuf,
}

pub fn discover_slots(root: &Path) -> Result<Vec<SlotSummary>, CoreError> {
    validate_root(root)?;
    let mut slots: BTreeMap<u8, Vec<ArtifactSummary>> = BTreeMap::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        reject_symlink(&entry.path())?;
        if let Some((slot, kind)) = classify_top_level(
            &entry.file_name().to_string_lossy(),
            entry.file_type()?.is_dir(),
        ) {
            let size = if entry.file_type()?.is_dir() {
                directory_size(&entry.path())?
            } else {
                entry.metadata()?.len()
            };
            slots.entry(slot).or_default().push(ArtifactSummary {
                relative_path: entry.file_name().to_string_lossy().into_owned(),
                kind,
                size,
            });
        }
    }
    Ok(slots
        .into_iter()
        .map(|(slot, mut artifacts)| {
            artifacts.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
            let complete = artifacts.iter().any(|a| a.kind == ArtifactKind::Primary);
            let total_size = artifacts.iter().map(|a| a.size).sum();
            SlotSummary {
                slot,
                complete,
                total_size,
                artifacts,
            }
        })
        .collect())
}

pub fn bundle_entries(root: &Path, slot: u8) -> Result<Vec<BundleEntry>, CoreError> {
    validate_root(root)?;
    let mut entries = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        reject_symlink(&path)?;
        if classify_top_level(
            &entry.file_name().to_string_lossy(),
            entry.file_type()?.is_dir(),
        )
        .is_some_and(|value| value.0 == slot)
        {
            if entry.file_type()?.is_dir() {
                collect_directory(root, &path, &mut entries)?;
            } else {
                entries.push(BundleEntry {
                    relative: path.strip_prefix(root).expect("child path").to_owned(),
                    source: path,
                });
            }
        }
    }
    entries.sort_by(|a, b| a.relative.cmp(&b.relative));
    let primary_name = format!("slot{slot}.save");
    if !entries
        .iter()
        .any(|entry| entry.relative == Path::new(&primary_name))
    {
        return Err(CoreError::MissingPrimary(slot));
    }
    Ok(entries)
}

fn validate_root(root: &Path) -> Result<(), CoreError> {
    if !root.is_dir() {
        return Err(CoreError::InvalidSaveDirectory);
    }
    reject_symlink(root)
}

fn classify_top_level(name: &str, is_dir: bool) -> Option<(u8, ArtifactKind)> {
    for slot in 0..=5 {
        let base = format!("slot{slot}");
        let kind = if is_dir && name == base {
            Some(ArtifactKind::Sidecar)
        } else if !is_dir && name == format!("{base}.save") {
            Some(ArtifactKind::Primary)
        } else if !is_dir && name == format!("{base}.save.meta") {
            Some(ArtifactKind::Metadata)
        } else if !is_dir && name == format!("{base}.save.png") {
            Some(ArtifactKind::Thumbnail)
        } else if !is_dir && (1..=5).any(|backup| name == format!("{base}_backup{backup}.save")) {
            Some(ArtifactKind::GameBackup)
        } else {
            None
        };
        if let Some(kind) = kind {
            return Some((slot, kind));
        }
    }
    None
}

fn collect_directory(
    root: &Path,
    directory: &Path,
    entries: &mut Vec<BundleEntry>,
) -> Result<(), CoreError> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        reject_symlink(&path)?;
        if entry.file_type()?.is_dir() {
            collect_directory(root, &path, entries)?;
        } else if entry.file_type()?.is_file() {
            entries.push(BundleEntry {
                relative: path.strip_prefix(root).expect("child path").to_owned(),
                source: path,
            });
        } else {
            return Err(CoreError::InvalidSnapshot(format!(
                "Unsupported sidecar entry type: {}",
                path.display()
            )));
        }
    }
    Ok(())
}

fn directory_size(path: &Path) -> Result<u64, CoreError> {
    let mut size = 0;
    let mut entries = Vec::new();
    collect_directory(path.parent().expect("directory parent"), path, &mut entries)?;
    for entry in entries {
        size += fs::metadata(entry.source)?.len();
    }
    Ok(size)
}

fn reject_symlink(path: &Path) -> Result<(), CoreError> {
    if fs::symlink_metadata(path)?.file_type().is_symlink() {
        return Err(CoreError::SymbolicLink(path.display().to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovers_complete_bundle_and_ignores_unrelated_files() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("slot1.save"), b"save").unwrap();
        fs::write(temp.path().join("slot1.save.png"), b"png").unwrap();
        fs::write(temp.path().join("slot1_backup2.save"), b"backup").unwrap();
        fs::write(temp.path().join("notes.txt"), b"ignore").unwrap();
        fs::create_dir(temp.path().join("slot1")).unwrap();
        fs::write(temp.path().join("slot1/mod.bin"), b"mod").unwrap();

        let slots = discover_slots(temp.path()).unwrap();
        assert_eq!(slots.len(), 1);
        assert!(slots[0].complete);
        assert_eq!(slots[0].artifacts.len(), 4);
        assert_eq!(bundle_entries(temp.path(), 1).unwrap().len(), 4);
    }

    #[test]
    fn reports_bundle_without_primary_as_incomplete() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("slot2.save.meta"), b"meta").unwrap();
        let slots = discover_slots(temp.path()).unwrap();
        assert!(!slots[0].complete);
        assert!(matches!(
            bundle_entries(temp.path(), 2),
            Err(CoreError::MissingPrimary(2))
        ));
    }
}
