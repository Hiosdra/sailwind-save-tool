use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::CoreError;
use crate::storage::write_json_atomic;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub schema_version: u32,
    pub active_save_directory: Option<String>,
    pub update_checks_enabled: bool,
    pub theme: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            schema_version: 1,
            active_save_directory: None,
            update_checks_enabled: true,
            theme: "dark".to_owned(),
        }
    }
}

pub fn load(config_directory: &Path) -> Result<AppSettings, CoreError> {
    let path = config_directory.join("settings.json");
    if !path.exists() {
        return Ok(AppSettings::default());
    }
    let settings: AppSettings = serde_json::from_reader(fs::File::open(path)?)?;
    if settings.schema_version != 1 {
        return Err(CoreError::InvalidSettings(format!(
            "Unsupported settings schema version: {}",
            settings.schema_version
        )));
    }
    Ok(settings)
}

pub fn set_active_directory(
    config_directory: &Path,
    active_save_directory: Option<String>,
) -> Result<AppSettings, CoreError> {
    let mut settings = load(config_directory)?;
    settings.active_save_directory = active_save_directory;
    write_json_atomic(&config_directory.join("settings.json"), &settings)?;
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_and_replace_atomically() {
        let directory = tempfile::tempdir().unwrap();
        assert_eq!(load(directory.path()).unwrap(), AppSettings::default());

        set_active_directory(directory.path(), Some("first".to_owned())).unwrap();
        set_active_directory(directory.path(), Some("second".to_owned())).unwrap();

        assert_eq!(
            load(directory.path()).unwrap().active_save_directory,
            Some("second".to_owned())
        );
    }
}
