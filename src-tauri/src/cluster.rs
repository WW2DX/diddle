// DX cluster (telnet/TCP) client. Connects to a packet/web cluster server,
// logs in with the operator's callsign, parses incoming DX spots and emits
// them as `cluster:spot` events. Also exposes the raw line stream as
// `cluster:line` for users who want to type cluster commands.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, RwLock};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ClusterState {
    Disconnected,
    Connecting { host: String, port: u16 },
    Connected { host: String, port: u16 },
    Error { message: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct ClusterSpot {
    pub source: String,
    pub dx_call: String,
    pub freq_hz: u64,
    pub band: String,
    pub comment: String,
    pub time_utc: String,
    pub timestamp_ms: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClusterLine {
    pub dir: &'static str, // "rx" | "tx"
    pub text: String,
}

pub struct ClusterClient {
    app: AppHandle,
    state: RwLock<ClusterState>,
    cmd_tx: RwLock<Option<mpsc::Sender<String>>>,
    task: RwLock<Option<JoinHandle<()>>>,
}

impl ClusterClient {
    pub fn new(app: AppHandle) -> Self {
        Self {
            app,
            state: RwLock::new(ClusterState::Disconnected),
            cmd_tx: RwLock::new(None),
            task: RwLock::new(None),
        }
    }

    pub async fn state(&self) -> ClusterState {
        self.state.read().await.clone()
    }

    pub async fn connect(
        self: Arc<Self>,
        host: String,
        port: u16,
        login: String,
    ) -> anyhow::Result<()> {
        // Cancel any existing connection.
        if let Some(h) = self.task.write().await.take() {
            h.abort();
        }
        *self.cmd_tx.write().await = None;

        let me = self.clone();
        let handle = tokio::spawn(async move {
            me.run_loop(host, port, login).await;
        });
        *self.task.write().await = Some(handle);
        Ok(())
    }

    pub async fn disconnect(&self) {
        if let Some(h) = self.task.write().await.take() {
            h.abort();
        }
        *self.cmd_tx.write().await = None;
        self.set_state(ClusterState::Disconnected).await;
    }

    pub async fn send(&self, line: String) -> anyhow::Result<()> {
        let tx = self.cmd_tx.read().await.clone();
        match tx {
            Some(tx) => {
                tx.send(line)
                    .await
                    .map_err(|e| anyhow::anyhow!("cluster channel closed: {e}"))?;
                Ok(())
            }
            None => anyhow::bail!("cluster not connected"),
        }
    }

    async fn set_state(&self, s: ClusterState) {
        *self.state.write().await = s.clone();
        let _ = self.app.emit("cluster:state", &s);
    }

    async fn run_loop(self: Arc<Self>, host: String, port: u16, login: String) {
        self.set_state(ClusterState::Connecting {
            host: host.clone(),
            port,
        })
        .await;
        info!(%host, port, "cluster: connecting");

        let stream = match TcpStream::connect((host.as_str(), port)).await {
            Ok(s) => s,
            Err(e) => {
                error!("cluster connect failed: {e}");
                self.set_state(ClusterState::Error {
                    message: e.to_string(),
                })
                .await;
                return;
            }
        };
        info!("cluster: TCP connected");
        self.set_state(ClusterState::Connected {
            host: host.clone(),
            port,
        })
        .await;

        let (read_half, mut write_half) = stream.into_split();
        let mut reader = BufReader::new(read_half).lines();

        let (cmd_tx, mut cmd_rx) = mpsc::channel::<String>(32);
        *self.cmd_tx.write().await = Some(cmd_tx.clone());

        // Auto-send login on first prompt: most cluster servers will say
        // "Please enter your call:" or similar. We just send the login
        // unconditionally after a short delay so it works for variants
        // that don't prompt at all.
        let login_clone = login.clone();
        let cmd_tx2 = cmd_tx.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            let _ = cmd_tx2.send(login_clone).await;
        });

        let mut logged_in = false;
        loop {
            tokio::select! {
                outbound = cmd_rx.recv() => {
                    match outbound {
                        Some(line) => {
                            debug!(tx=%line, "cluster tx");
                            let _ = self.app.emit("cluster:line", &ClusterLine {
                                dir: "tx",
                                text: line.clone(),
                            });
                            let to_send = format!("{line}\r\n");
                            if write_half.write_all(to_send.as_bytes()).await.is_err() {
                                warn!("cluster write failed");
                                break;
                            }
                        }
                        None => break,
                    }
                }
                line = reader.next_line() => {
                    match line {
                        Ok(Some(l)) => {
                            let trimmed = l.trim_end_matches('\r').to_string();
                            let _ = self.app.emit("cluster:line", &ClusterLine {
                                dir: "rx",
                                text: trimmed.clone(),
                            });
                            // Detect a few common "logged in" markers so we
                            // know not to send the callsign a second time
                            // when more prompts come up.
                            if !logged_in {
                                if trimmed.to_lowercase().contains("login") ||
                                   trimmed.to_lowercase().contains("logged in") ||
                                   trimmed.to_lowercase().contains("dx spider") ||
                                   trimmed.to_lowercase().contains(login.to_lowercase().as_str()) {
                                    logged_in = true;
                                }
                            }
                            if let Some(spot) = parse_spot_line(&trimmed) {
                                let _ = self.app.emit("cluster:spot", &spot);
                            }
                        }
                        Ok(None) => {
                            info!("cluster: server closed connection");
                            break;
                        }
                        Err(e) => {
                            warn!("cluster read error: {e}");
                            break;
                        }
                    }
                }
            }
        }

        self.set_state(ClusterState::Disconnected).await;
        *self.cmd_tx.write().await = None;
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn band_from_khz(khz: f64) -> &'static str {
    let hz = khz * 1000.0;
    if hz < 2_000_000.0 { "160m" }
    else if hz < 4_000_000.0 { "80m" }
    else if hz < 7_300_000.0 { "40m" }
    else if hz < 10_500_000.0 { "30m" }
    else if hz < 14_500_000.0 { "20m" }
    else if hz < 18_500_000.0 { "17m" }
    else if hz < 22_000_000.0 { "15m" }
    else if hz < 25_500_000.0 { "12m" }
    else if hz < 30_000_000.0 { "10m" }
    else if (50_000_000.0..54_000_000.0).contains(&hz) { "6m" }
    else if (144_000_000.0..148_000_000.0).contains(&hz) { "2m" }
    else if (222_000_000.0..225_000_000.0).contains(&hz) { "1.25m" }
    else if (420_000_000.0..450_000_000.0).contains(&hz) { "70cm" }
    else if (902_000_000.0..928_000_000.0).contains(&hz) { "33cm" }
    else if (1_240_000_000.0..1_300_000_000.0).contains(&hz) { "23cm" }
    else { "?" }
}

/// Parse a classic cluster spot line:
///   DX de N1MM:     14080.5  R120RB          CQ Test                         1234Z
fn parse_spot_line(line: &str) -> Option<ClusterSpot> {
    let line = line.trim();
    if !line.starts_with("DX de ") {
        return None;
    }
    let rest = &line[6..];
    let colon = rest.find(':')?;
    let source = rest[..colon].trim().to_string();
    let after = rest[colon + 1..].trim_start();
    let mut iter = after.split_whitespace();
    let freq_str = iter.next()?;
    let freq_khz: f64 = freq_str.parse().ok()?;
    if !(100.0..100_000.0).contains(&freq_khz) {
        return None;
    }
    let dx_call = iter.next()?.trim_end_matches(':').to_string();
    if dx_call.is_empty() {
        return None;
    }
    let rest_tokens: Vec<&str> = iter.collect();
    if rest_tokens.is_empty() {
        return Some(ClusterSpot {
            source,
            dx_call,
            freq_hz: (freq_khz * 1000.0) as u64,
            band: band_from_khz(freq_khz).to_string(),
            comment: String::new(),
            time_utc: String::new(),
            timestamp_ms: now_ms(),
        });
    }
    // The last token is conventionally HHMMZ; if it matches, peel it off.
    let last = rest_tokens.last().copied().unwrap();
    let (comment_tokens, time_utc) = if looks_like_time(last) {
        let n = rest_tokens.len();
        (
            rest_tokens[..n - 1].to_vec(),
            last.to_string(),
        )
    } else {
        (rest_tokens, String::new())
    };

    Some(ClusterSpot {
        source,
        dx_call,
        freq_hz: (freq_khz * 1000.0) as u64,
        band: band_from_khz(freq_khz).to_string(),
        comment: comment_tokens.join(" "),
        time_utc,
        timestamp_ms: now_ms(),
    })
}

fn looks_like_time(s: &str) -> bool {
    let s = s.trim_end_matches('Z').trim_end_matches('z');
    s.len() == 4 && s.chars().all(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_classic_spot() {
        let line = "DX de N1MM:     14080.5  R120RB          CQ Test  1234Z";
        let s = parse_spot_line(line).unwrap();
        assert_eq!(s.source, "N1MM");
        assert_eq!(s.dx_call, "R120RB");
        assert_eq!(s.freq_hz, 14_080_500);
        assert_eq!(s.band, "20m");
        assert!(s.comment.contains("CQ"));
        assert_eq!(s.time_utc, "1234Z");
    }

    #[test]
    fn parses_spot_with_long_comment() {
        let line = "DX de DL3XYZ:     14082.5 SX150ITU       CQ TEST QSL DIRECT  1547Z";
        let s = parse_spot_line(line).unwrap();
        assert_eq!(s.dx_call, "SX150ITU");
        assert_eq!(s.freq_hz, 14_082_500);
        assert!(s.comment.contains("CQ TEST QSL DIRECT"));
    }

    #[test]
    fn rejects_non_spot() {
        assert!(parse_spot_line("Welcome to DX Spider!").is_none());
        assert!(parse_spot_line("WWV de WX5XYZ:    18 67 5.0").is_none());
    }
}
