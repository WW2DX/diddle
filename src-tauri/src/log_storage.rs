// Simple JSON-on-disk persistence for the QSO log.
// File lives at `<app_data_dir>/diddle/qsos.json`.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::Manager;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Qso {
    pub id: String,
    pub ts: i64, // unix ms
    pub call: String,
    pub freq_hz: u64,
    pub band: String,
    pub mode: String,
    pub rst_sent: String,
    pub rst_rcvd: String,
    pub exch_sent: String,
    pub exch_rcvd: String,
    pub serial_sent: u32,
}

fn log_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app_data_dir: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("create_dir_all: {e}"))?;
    Ok(dir.join("qsos.json"))
}

pub async fn save(app: &tauri::AppHandle, qsos: &[Qso]) -> Result<(), String> {
    let path = log_path(app)?;
    let json =
        serde_json::to_string_pretty(qsos).map_err(|e| format!("serialize: {e}"))?;
    tokio::fs::write(&path, json)
        .await
        .map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(())
}

pub async fn load(app: &tauri::AppHandle) -> Result<Vec<Qso>, String> {
    let path = log_path(app)?;
    if !path.exists() {
        return Ok(vec![]);
    }
    let json = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| format!("read {}: {e}", path.display()))?;
    serde_json::from_str(&json).map_err(|e| format!("parse {}: {e}", path.display()))
}
