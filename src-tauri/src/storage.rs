use std::fs;
use std::io::Write;
use std::path::Path;

use crate::error::CoreError;

pub fn write_json_atomic<T: serde::Serialize>(path: &Path, value: &T) -> Result<(), CoreError> {
    let parent = path.parent().ok_or_else(|| {
        CoreError::InvalidSettings("Atomic JSON destination has no parent directory".to_owned())
    })?;
    fs::create_dir_all(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer_pretty(&mut temporary, value)?;
    temporary.write_all(b"\n")?;
    temporary.as_file().sync_all()?;
    temporary
        .persist(path)
        .map_err(|error| CoreError::Io(error.error))?;
    Ok(())
}
