// Tauri IPC commands exposed to the Svelte frontend.

use tauri::State;

use std::path::PathBuf;

use crate::cluster::ClusterState;
use crate::dsp::RttyConfig;
use crate::log_storage::{self, Qso};
use crate::scp::{self, ScpStatus};
use crate::tci::{RigState, TciState};
use crate::wav_player::WavStatus;
use crate::AppState;

#[tauri::command]
pub async fn tci_connect(state: State<'_, AppState>, url: String) -> Result<(), String> {
    state
        .tci
        .clone()
        .connect(url)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn tci_disconnect(state: State<'_, AppState>) -> Result<(), String> {
    state.tci.disconnect().await;
    Ok(())
}

#[tauri::command]
pub async fn tci_status(state: State<'_, AppState>) -> Result<(TciState, RigState), String> {
    Ok((state.tci.state().await, state.tci.rig().await))
}

#[tauri::command]
pub async fn set_freq(state: State<'_, AppState>, hz: u64) -> Result<(), String> {
    state
        .tci
        .send(format!("vfo:0,0,{};", hz))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_ptt(state: State<'_, AppState>, on: bool) -> Result<(), String> {
    state
        .tci
        .send(format!("trx:0,{};", on))
        .await
        .map_err(|e| e.to_string())
}

/// Send a raw TCI text command. Used by the debug console while we probe
/// the protocol; auto-appends ';' if missing.
#[tauri::command]
pub async fn tci_send(state: State<'_, AppState>, raw: String) -> Result<(), String> {
    let line = if raw.trim_end().ends_with(';') {
        raw
    } else {
        format!("{};", raw.trim_end())
    };
    state.tci.send(line).await.map_err(|e| e.to_string())
}

/// Load and play a WAV file through the DSP pipeline (Spectrum + RttyDemod).
/// Auto-stops any live TCI audio first so streams don't mix.
#[tauri::command]
pub async fn play_wav(state: State<'_, AppState>, path: String) -> Result<(), String> {
    // Best-effort: stop TCI audio. Errors are fine — we may not be connected.
    let _ = state.tci.send("audio_stop:0;".to_string()).await;
    state
        .wav
        .clone()
        .play(path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn stop_wav(state: State<'_, AppState>) -> Result<(), String> {
    state.wav.stop().await;
    Ok(())
}

#[tauri::command]
pub async fn wav_status(state: State<'_, AppState>) -> Result<WavStatus, String> {
    Ok(state.wav.status().await)
}

#[tauri::command]
pub async fn get_rtty_config(state: State<'_, AppState>) -> Result<RttyConfig, String> {
    Ok(state.rtty.get().await)
}

#[tauri::command]
pub async fn set_rtty_config(
    state: State<'_, AppState>,
    mark_hz: f32,
    space_hz: f32,
    baud: f32,
) -> Result<(), String> {
    if !(mark_hz > 50.0 && mark_hz < 20_000.0) || !(space_hz > 50.0 && space_hz < 20_000.0) {
        return Err(format!(
            "tones out of range: mark={mark_hz} space={space_hz}"
        ));
    }
    if !(5.0..=300.0).contains(&baud) {
        return Err(format!("baud out of range: {baud}"));
    }
    let cfg = RttyConfig {
        mark_hz,
        space_hz,
        baud,
    };
    state.rtty.set(cfg).await;
    Ok(())
}

#[tauri::command]
pub async fn save_log(app: tauri::AppHandle, qsos: Vec<Qso>) -> Result<(), String> {
    log_storage::save(&app, &qsos).await
}

#[tauri::command]
pub async fn load_log(app: tauri::AppHandle) -> Result<Vec<Qso>, String> {
    log_storage::load(&app).await
}

#[tauri::command]
pub async fn transmit(state: State<'_, AppState>, text: String) -> Result<(), String> {
    state.tci.transmit(text).await.map_err(|e| e.to_string())
}

/// Abort any in-flight transmission immediately. No-op if nothing is TXing.
#[tauri::command]
pub async fn tx_abort(state: State<'_, AppState>) -> Result<(), String> {
    state.tci.abort_tx().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_file_text(path: String, content: String) -> Result<(), String> {
    tokio::fs::write(&path, content)
        .await
        .map_err(|e| format!("write {path}: {e}"))
}

#[tauri::command]
pub async fn scp_search(
    state: State<'_, AppState>,
    query: String,
    max: usize,
) -> Result<Vec<String>, String> {
    Ok(state.scp.search(&query, max.min(50)))
}

#[tauri::command]
pub async fn scp_load_file(
    state: State<'_, AppState>,
    path: String,
) -> Result<ScpStatus, String> {
    state
        .scp
        .load_file(&PathBuf::from(&path))
        .await
        .map_err(|e| format!("scp load {path}: {e}"))?;
    Ok(state.scp.status())
}

#[tauri::command]
pub async fn scp_status(state: State<'_, AppState>) -> Result<ScpStatus, String> {
    Ok(state.scp.status())
}

/// Membership test against the loaded SCP database. Returns the first
/// token that matched (or empty string). The frontend uses this to gate
/// decoder output so pure-noise lines don't reach the operator.
#[tauri::command]
pub async fn scp_contains_any(
    state: State<'_, AppState>,
    calls: Vec<String>,
) -> Result<String, String> {
    for c in calls {
        if state.scp.contains(&c) {
            return Ok(c.to_ascii_uppercase());
        }
    }
    Ok(String::new())
}

#[derive(serde::Serialize)]
pub struct ScpAutoDownloadResult {
    pub status: ScpStatus,
    pub path: String,
}

/// Download MASTER.SCP into the app's data dir and load it. Returns the
/// resolved status and the on-disk path the caller can persist so future
/// launches load it without re-downloading.
#[tauri::command]
pub async fn scp_auto_download(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<ScpAutoDownloadResult, String> {
    let path = scp::cached_path(&app)?;
    scp::download_master_scp(&path)
        .await
        .map_err(|e| format!("download MASTER.SCP: {e}"))?;
    state
        .scp
        .load_file(&path)
        .await
        .map_err(|e| format!("load {}: {e}", path.display()))?;
    Ok(ScpAutoDownloadResult {
        status: state.scp.status(),
        path: path.to_string_lossy().to_string(),
    })
}

#[tauri::command]
pub async fn cluster_connect(
    state: State<'_, AppState>,
    host: String,
    port: u16,
    login: String,
) -> Result<(), String> {
    state
        .cluster
        .clone()
        .connect(host, port, login)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cluster_disconnect(state: State<'_, AppState>) -> Result<(), String> {
    state.cluster.disconnect().await;
    Ok(())
}

#[tauri::command]
pub async fn cluster_send(state: State<'_, AppState>, line: String) -> Result<(), String> {
    state.cluster.send(line).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cluster_status(state: State<'_, AppState>) -> Result<ClusterState, String> {
    Ok(state.cluster.state().await)
}
