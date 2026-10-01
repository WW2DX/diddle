// Country file (cty.dat / WL_CTY.DAT) storage. Diddle ships a copy of AD1C's
// cty.dat (served from the app's static assets); the operator can replace it
// with a fresh download from country-files.com or a file of their own
// (e.g. WL_CTY.DAT). The replacement lives in the app data dir and wins
// over the bundled copy. Parsing happens in the frontend (src/lib/cty.ts).

use std::path::PathBuf;
use std::time::Duration;

use tracing::info;

const REMOTE_URL: &str = "https://www.country-files.com/cty/cty.dat";

fn cached_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    use tauri::Manager;
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app_data_dir: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("create_dir_all: {e}"))?;
    Ok(dir.join("cty.dat"))
}

/// A real country file is tens of KB of `Name: cq: itu: …;` records. Refuse
/// anything that doesn't look like one, so an error page or a wrong file
/// never replaces a working copy.
fn looks_like_cty(text: &str) -> bool {
    text.len() > 20_000 && text.matches(';').count() > 200 && text.contains("United States")
}

/// The operator's replacement country file, if any.
#[tauri::command]
pub async fn cty_cached(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let path = cached_path(&app)?;
    match tokio::fs::read_to_string(&path).await {
        Ok(t) if looks_like_cty(&t) => Ok(Some(t)),
        _ => Ok(None),
    }
}

/// Download the current cty.dat from country-files.com and keep it.
#[tauri::command]
pub async fn cty_download(app: tauri::AppHandle) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent(concat!("diddle/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| e.to_string())?;
    let text = client
        .get(REMOTE_URL)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("download cty.dat: {e}"))?
        .text()
        .await
        .map_err(|e| format!("download cty.dat: {e}"))?;
    if !looks_like_cty(&text) {
        return Err("the download doesn't look like a country file — kept the current one".into());
    }
    let path = cached_path(&app)?;
    tokio::fs::write(&path, &text).await.map_err(|e| format!("write {}: {e}", path.display()))?;
    info!(bytes = text.len(), "cty: downloaded cty.dat");
    Ok(text)
}

/// Use a country file from disk (cty.dat or WL_CTY.DAT) and keep a copy.
#[tauri::command]
pub async fn cty_load_file(app: tauri::AppHandle, path: String) -> Result<String, String> {
    let bytes = tokio::fs::read(&path).await.map_err(|e| format!("read {path}: {e}"))?;
    // Some country files are Latin-1; keep whatever decodes.
    let text = String::from_utf8_lossy(&bytes).into_owned();
    if !looks_like_cty(&text) {
        return Err(format!("{path} doesn't look like a cty.dat / WL_CTY.DAT country file"));
    }
    let dest = cached_path(&app)?;
    tokio::fs::write(&dest, &text).await.map_err(|e| format!("write {}: {e}", dest.display()))?;
    info!(%path, "cty: loaded country file");
    Ok(text)
}

/// Go back to the bundled copy.
#[tauri::command]
pub async fn cty_reset(app: tauri::AppHandle) -> Result<(), String> {
    let path = cached_path(&app)?;
    let _ = tokio::fs::remove_file(&path).await;
    Ok(())
}
