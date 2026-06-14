mod cluster;
pub mod dsp;
mod ipc;
mod log_storage;
mod logbook;
mod scp;
mod tci;
mod wav_player;

use std::sync::Arc;
use tauri::Manager;
use tci::TciClient;
use wav_player::WavPlayer;

use crate::cluster::ClusterClient;
use crate::dsp::{RttyConfig, RttyTunable};
use crate::scp::ScpDb;

pub struct AppState {
    pub tci: Arc<TciClient>,
    pub wav: Arc<WavPlayer>,
    pub rtty: Arc<RttyTunable>,
    pub scp: Arc<ScpDb>,
    pub cluster: Arc<ClusterClient>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,diddle=debug".into()),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let rtty = Arc::new(RttyTunable::new(RttyConfig::default()));
            let scp = Arc::new(ScpDb::new());
            let tci = Arc::new(TciClient::new(handle.clone(), rtty.clone(), scp.clone()));
            let wav = Arc::new(WavPlayer::new(handle.clone(), rtty.clone(), scp.clone()));
            let cluster = Arc::new(ClusterClient::new(handle));
            app.manage(AppState { tci, wav, rtty, scp, cluster });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ipc::tci_connect,
            ipc::tci_disconnect,
            ipc::tci_status,
            ipc::set_freq,
            ipc::set_ptt,
            ipc::tci_send,
            ipc::play_wav,
            ipc::stop_wav,
            ipc::wav_status,
            ipc::get_rtty_config,
            ipc::set_rtty_config,
            ipc::save_log,
            ipc::load_log,
            ipc::transmit,
            ipc::tx_abort,
            ipc::save_file_text,
            ipc::scp_search,
            ipc::scp_load_file,
            ipc::scp_status,
            ipc::scp_contains_any,
            ipc::scp_auto_download,
            ipc::cluster_connect,
            ipc::cluster_disconnect,
            ipc::cluster_send,
            ipc::cluster_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
