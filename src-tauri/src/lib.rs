mod diagnostics;
mod error;
mod models;
mod saves;
mod settings;
mod snapshots;
mod storage;

use std::path::{Path, PathBuf};

use error::AppError;
use models::{RestoreResult, SlotSummary, SnapshotSummary};
use tauri::Manager;

#[tauri::command]
fn load_settings(app: tauri::AppHandle) -> Result<settings::AppSettings, AppError> {
    let config = app.path().app_config_dir().map_err(|error| AppError {
        code: "app_config_unavailable",
        message: error.to_string(),
        context: Default::default(),
    })?;
    settings::load(&config).map_err(Into::into)
}

#[tauri::command]
fn set_active_save_directory(
    app: tauri::AppHandle,
    save_directory: Option<String>,
) -> Result<settings::AppSettings, AppError> {
    let config = app.path().app_config_dir().map_err(|error| AppError {
        code: "app_config_unavailable",
        message: error.to_string(),
        context: Default::default(),
    })?;
    settings::set_active_directory(&config, save_directory).map_err(Into::into)
}

#[tauri::command]
fn detect_save_directories() -> Vec<String> {
    let mut candidates = Vec::new();
    #[cfg(target_os = "windows")]
    if let Some(profile) = std::env::var_os("USERPROFILE") {
        candidates.push(PathBuf::from(profile).join("AppData/LocalLow/Raw Lion Workshop/Sailwind"));
    }
    #[cfg(target_os = "linux")]
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        for steam in [home.join(".steam/steam"), home.join(".local/share/Steam")] {
            candidates.push(steam.join("steamapps/compatdata/1764530/pfx/drive_c/users/steamuser/AppData/LocalLow/Raw Lion Workshop/Sailwind"));
        }
    }
    candidates
        .into_iter()
        .filter(|path| path.is_dir())
        .map(|path| path.display().to_string())
        .collect()
}

#[tauri::command]
fn discover_slots(save_directory: String) -> Result<Vec<SlotSummary>, AppError> {
    saves::discover_slots(Path::new(&save_directory)).map_err(Into::into)
}

#[tauri::command]
fn create_snapshot(
    app: tauri::AppHandle,
    save_directory: String,
    slot: u8,
    prune_oldest: bool,
) -> Result<SnapshotSummary, AppError> {
    let app_data = app.path().app_data_dir().map_err(|error| AppError {
        code: "app_data_unavailable",
        message: error.to_string(),
        context: Default::default(),
    })?;
    let result = snapshots::create_manual_snapshot(
        Path::new(&save_directory),
        &app_data,
        slot,
        prune_oldest,
    );
    diagnostics::record_result(&app_data, "create_snapshot", Some(slot), &result);
    result.map_err(Into::into)
}

#[tauri::command]
fn list_snapshots(
    app: tauri::AppHandle,
    slot: Option<u8>,
) -> Result<Vec<SnapshotSummary>, AppError> {
    let app_data = app.path().app_data_dir().map_err(|error| AppError {
        code: "app_data_unavailable",
        message: error.to_string(),
        context: Default::default(),
    })?;
    snapshots::list_snapshots(&app_data, slot).map_err(Into::into)
}

#[tauri::command]
fn restore_snapshot(
    app: tauri::AppHandle,
    save_directory: String,
    snapshot_id: String,
) -> Result<RestoreResult, AppError> {
    let app_data = app.path().app_data_dir().map_err(|error| AppError {
        code: "app_data_unavailable",
        message: error.to_string(),
        context: Default::default(),
    })?;
    let result = snapshots::restore_snapshot(Path::new(&save_directory), &app_data, &snapshot_id);
    let slot = result.as_ref().ok().map(|restore| restore.slot);
    diagnostics::record_result(&app_data, "restore_snapshot", slot, &result);
    result.map_err(Into::into)
}

#[tauri::command]
fn update_snapshot_annotation(
    app: tauri::AppHandle,
    snapshot_id: String,
    label: Option<String>,
    note: Option<String>,
    protected: bool,
) -> Result<SnapshotSummary, AppError> {
    let app_data = app.path().app_data_dir().map_err(|error| AppError {
        code: "app_data_unavailable",
        message: error.to_string(),
        context: Default::default(),
    })?;
    let result = snapshots::update_annotation(&app_data, &snapshot_id, label, note, protected);
    diagnostics::record_result(&app_data, "update_annotation", None, &result);
    result.map_err(Into::into)
}

#[tauri::command]
fn delete_snapshot(app: tauri::AppHandle, snapshot_id: String) -> Result<(), AppError> {
    let app_data = app.path().app_data_dir().map_err(|error| AppError {
        code: "app_data_unavailable",
        message: error.to_string(),
        context: Default::default(),
    })?;
    let result = snapshots::delete_snapshot(&app_data, &snapshot_id);
    diagnostics::record_result(&app_data, "delete_snapshot", None, &result);
    result.map_err(Into::into)
}

#[tauri::command]
fn export_snapshot(
    app: tauri::AppHandle,
    snapshot_id: String,
    destination: String,
) -> Result<(), AppError> {
    let app_data = app.path().app_data_dir().map_err(|error| AppError {
        code: "app_data_unavailable",
        message: error.to_string(),
        context: Default::default(),
    })?;
    let result = snapshots::export_snapshot(&app_data, &snapshot_id, Path::new(&destination));
    diagnostics::record_result(&app_data, "export_snapshot", None, &result);
    result.map_err(Into::into)
}

#[tauri::command]
fn import_snapshot(app: tauri::AppHandle, source: String) -> Result<SnapshotSummary, AppError> {
    let app_data = app.path().app_data_dir().map_err(|error| AppError {
        code: "app_data_unavailable",
        message: error.to_string(),
        context: Default::default(),
    })?;
    let result = snapshots::import_snapshot(&app_data, Path::new(&source));
    let slot = result.as_ref().ok().map(|snapshot| snapshot.slot);
    diagnostics::record_result(&app_data, "import_snapshot", slot, &result);
    result.map_err(Into::into)
}

#[tauri::command]
fn load_diagnostics(app: tauri::AppHandle) -> Result<diagnostics::DiagnosticReport, AppError> {
    let app_data = app.path().app_data_dir().map_err(|error| AppError {
        code: "app_data_unavailable",
        message: error.to_string(),
        context: Default::default(),
    })?;
    diagnostics::report(&app_data).map_err(Into::into)
}

#[tauri::command]
fn acknowledge_recovery(app: tauri::AppHandle) -> Result<(), AppError> {
    let app_data = app.path().app_data_dir().map_err(|error| AppError {
        code: "app_data_unavailable",
        message: error.to_string(),
        context: Default::default(),
    })?;
    diagnostics::acknowledge_recovery(&app_data).map_err(Into::into)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_data = app.path().app_data_dir()?;
            let recovery = snapshots::recover_interrupted(&app_data);
            diagnostics::record_recovery(&app_data, &recovery);
            if let Err(error) = recovery {
                eprintln!("Failed to recover an interrupted restore: {error}");
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            load_settings,
            set_active_save_directory,
            detect_save_directories,
            discover_slots,
            create_snapshot,
            list_snapshots,
            restore_snapshot,
            update_snapshot_annotation,
            delete_snapshot,
            export_snapshot,
            import_snapshot,
            load_diagnostics,
            acknowledge_recovery
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Sailwind Save Manager");
}
