// QSO logs on disk: one JSON file per log, N1MM-style — a log per contest,
// and any earlier one can be opened again.
//
//   <app_data_dir>/logs/index.json   which logs exist + which one is open
//   <app_data_dir>/logs/<id>.json    a log's QSOs
//
// Before named logs there was a single `<app_data_dir>/qsos.json`. The first
// run with an index moves it into the first log (and keeps the old file as
// `qsos.json.migrated`), so nothing logged before is lost.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::Manager;
use tokio::sync::Mutex;

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
    // Operator note (Ctrl-N). Absent on QSOs logged before the field existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LogMeta {
    pub id: String,
    pub name: String,
    /// Contest profile the log was made for ("" = not recorded yet).
    #[serde(default)]
    pub contest_id: String,
    pub created: i64, // unix ms
}

#[derive(Serialize, Deserialize, Default)]
struct Index {
    active: String,
    logs: Vec<LogMeta>,
}

/// A log as listed in the picker.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogInfo {
    #[serde(flatten)]
    pub meta: LogMeta,
    pub count: usize,
    pub first_ts: Option<i64>,
    pub last_ts: Option<i64>,
}

#[derive(Serialize)]
pub struct LogList {
    pub active: String,
    pub logs: Vec<LogInfo>,
}

/// An opened log: its details and its QSOs.
#[derive(Serialize)]
pub struct OpenLog {
    pub meta: LogMeta,
    pub qsos: Vec<Qso>,
}

// Index changes are read-modify-write; one at a time.
static LOCK: Mutex<()> = Mutex::const_new(());

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Where Diddle keeps its data; every function below works inside it.
pub fn data_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path().app_data_dir().map_err(|e| format!("app_data_dir: {e}"))
}

fn logs_dir(data: &Path) -> Result<PathBuf, String> {
    let dir = data.join("logs");
    std::fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    Ok(dir)
}

fn log_file(dir: &Path, id: &str) -> Result<PathBuf, String> {
    // Ids are ours ("log-<ms>"), but never let one reach outside the folder.
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(format!("bad log id {id:?}"));
    }
    Ok(dir.join(format!("{id}.json")))
}

/// Write via a temp file + rename, so a crash mid-write can't leave a log
/// half-written.
async fn write_atomic(path: &Path, content: String) -> Result<(), String> {
    let tmp = path.with_extension("json.tmp");
    tokio::fs::write(&tmp, content)
        .await
        .map_err(|e| format!("write {}: {e}", tmp.display()))?;
    tokio::fs::rename(&tmp, path)
        .await
        .map_err(|e| format!("rename to {}: {e}", path.display()))
}

async fn read_qsos(path: &Path) -> Result<Vec<Qso>, String> {
    if !path.exists() {
        return Ok(vec![]);
    }
    let json = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| format!("read {}: {e}", path.display()))?;
    serde_json::from_str(&json).map_err(|e| format!("parse {}: {e}", path.display()))
}

async fn write_qsos(path: &Path, qsos: &[Qso]) -> Result<(), String> {
    let json = serde_json::to_string_pretty(qsos).map_err(|e| format!("serialize: {e}"))?;
    write_atomic(path, json).await
}

async fn write_index(dir: &Path, idx: &Index) -> Result<(), String> {
    let json = serde_json::to_string_pretty(idx).map_err(|e| format!("serialize: {e}"))?;
    write_atomic(&dir.join("index.json"), json).await
}

/// A fresh, unique log id.
fn new_id(idx: &Index) -> String {
    let mut t = now_ms();
    while idx.logs.iter().any(|l| l.id == format!("log-{t}")) {
        t += 1;
    }
    format!("log-{t}")
}

/// Read the index, creating it (and migrating the old single log) on first
/// use. Always returns an index with at least one log and a valid `active`.
async fn load_index(data: &Path) -> Result<(PathBuf, Index), String> {
    let dir = logs_dir(data)?;
    let path = dir.join("index.json");
    let mut idx: Index = if path.exists() {
        let json = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| format!("read {}: {e}", path.display()))?;
        serde_json::from_str(&json).map_err(|e| format!("parse {}: {e}", path.display()))?
    } else {
        Index::default()
    };
    let mut dirty = false;

    if idx.logs.is_empty() {
        let id = new_id(&idx);
        let old = data.join("qsos.json");
        let migrated = if old.exists() {
            let qsos = read_qsos(&old).await?;
            write_qsos(&log_file(&dir, &id)?, &qsos).await?;
            let _ = tokio::fs::rename(&old, old.with_extension("json.migrated")).await;
            true
        } else {
            false
        };
        idx.logs.push(LogMeta {
            id: id.clone(),
            name: if migrated { "Log (before named logs)".into() } else { "Log 1".into() },
            contest_id: String::new(),
            created: now_ms(),
        });
        idx.active = id;
        dirty = true;
    }
    if !idx.logs.iter().any(|l| l.id == idx.active) {
        idx.active = idx.logs[0].id.clone();
        dirty = true;
    }
    if dirty {
        write_index(&dir, &idx).await?;
    }
    Ok((dir, idx))
}

pub async fn list(data: &Path) -> Result<LogList, String> {
    let _g = LOCK.lock().await;
    let (dir, idx) = load_index(data).await?;
    let mut logs = Vec::with_capacity(idx.logs.len());
    for meta in idx.logs {
        let qsos = read_qsos(&log_file(&dir, &meta.id)?).await.unwrap_or_default();
        logs.push(LogInfo {
            count: qsos.len(),
            first_ts: qsos.iter().map(|q| q.ts).min(),
            last_ts: qsos.iter().map(|q| q.ts).max(),
            meta,
        });
    }
    Ok(LogList { active: idx.active, logs })
}

/// Open a log (None = the one that was open last) and make it active.
pub async fn open(data: &Path, id: Option<String>) -> Result<OpenLog, String> {
    let _g = LOCK.lock().await;
    let (dir, mut idx) = load_index(data).await?;
    let id = id.unwrap_or_else(|| idx.active.clone());
    let meta = idx
        .logs
        .iter()
        .find(|l| l.id == id)
        .cloned()
        .ok_or_else(|| format!("no log {id}"))?;
    let qsos = read_qsos(&log_file(&dir, &id)?).await?;
    if idx.active != id {
        idx.active = id;
        write_index(&dir, &idx).await?;
    }
    Ok(OpenLog { meta, qsos })
}

/// Make a new log (optionally already holding QSOs — an import) and open it.
pub async fn create(
    data: &Path,
    name: String,
    contest_id: String,
    qsos: Vec<Qso>,
) -> Result<OpenLog, String> {
    let _g = LOCK.lock().await;
    let (dir, mut idx) = load_index(data).await?;
    let id = new_id(&idx);
    let name = name.trim();
    let meta = LogMeta {
        id: id.clone(),
        name: if name.is_empty() { format!("Log {}", idx.logs.len() + 1) } else { name.to_string() },
        contest_id,
        created: now_ms(),
    };
    write_qsos(&log_file(&dir, &id)?, &qsos).await?;
    idx.logs.push(meta.clone());
    idx.active = id;
    write_index(&dir, &idx).await?;
    Ok(OpenLog { meta, qsos })
}

pub async fn save(data: &Path, id: &str, qsos: &[Qso]) -> Result<(), String> {
    let _g = LOCK.lock().await;
    let (dir, idx) = load_index(data).await?;
    // A save racing a delete must not bring the log back as a stray file.
    if !idx.logs.iter().any(|l| l.id == id) {
        return Err(format!("no log {id}"));
    }
    write_qsos(&log_file(&dir, id)?, qsos).await
}

pub async fn update_meta(
    data: &Path,
    id: &str,
    name: Option<String>,
    contest_id: Option<String>,
) -> Result<(), String> {
    let _g = LOCK.lock().await;
    let (dir, mut idx) = load_index(data).await?;
    let meta = idx
        .logs
        .iter_mut()
        .find(|l| l.id == id)
        .ok_or_else(|| format!("no log {id}"))?;
    if let Some(n) = name.map(|n| n.trim().to_string()).filter(|n| !n.is_empty()) {
        meta.name = n;
    }
    if let Some(c) = contest_id {
        meta.contest_id = c;
    }
    write_index(&dir, &idx).await
}

/// Delete a log that isn't open. Its file is kept as `<id>.json.deleted`
/// rather than erased, as a last resort.
pub async fn delete(data: &Path, id: &str) -> Result<(), String> {
    let _g = LOCK.lock().await;
    let (dir, mut idx) = load_index(data).await?;
    if idx.active == id {
        return Err("can't delete the open log — open another one first".into());
    }
    let before = idx.logs.len();
    idx.logs.retain(|l| l.id != id);
    if idx.logs.len() == before {
        return Err(format!("no log {id}"));
    }
    let file = log_file(&dir, id)?;
    if file.exists() {
        let _ = tokio::fs::rename(&file, file.with_extension("json.deleted")).await;
    }
    write_index(&dir, &idx).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp() -> PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static N: AtomicU32 = AtomicU32::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let d = std::env::temp_dir().join(format!("diddle-logs-{}-{}-{n}", std::process::id(), now_ms()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn qso(call: &str, ts: i64, serial: u32) -> Qso {
        Qso {
            id: format!("{call}-{ts}"),
            ts,
            call: call.into(),
            freq_hz: 14_085_000,
            band: "20m".into(),
            mode: "digl".into(),
            rst_sent: "599".into(),
            rst_rcvd: "599".into(),
            exch_sent: "001".into(),
            exch_rcvd: "KR".into(),
            serial_sent: serial,
            note: None,
        }
    }

    #[tokio::test]
    async fn old_single_log_becomes_the_first_named_log() {
        let data = tmp();
        let old = vec![qso("UR5ZZ", 1000, 1), qso("W1AW", 2000, 2)];
        std::fs::write(data.join("qsos.json"), serde_json::to_string(&old).unwrap()).unwrap();

        let opened = open(&data, None).await.unwrap();
        assert_eq!(opened.qsos.len(), 2);
        assert_eq!(opened.qsos[0].call, "UR5ZZ");
        assert_eq!(opened.meta.name, "Log (before named logs)");
        assert!(!data.join("qsos.json").exists());
        assert!(data.join("qsos.json.migrated").exists());

        // Second start: same log, nothing migrated twice.
        let l = list(&data).await.unwrap();
        assert_eq!(l.logs.len(), 1);
        assert_eq!(l.logs[0].count, 2);
        assert_eq!(l.logs[0].first_ts, Some(1000));
        let _ = std::fs::remove_dir_all(&data);
    }

    #[tokio::test]
    async fn fresh_install_starts_with_an_empty_log() {
        let data = tmp();
        let opened = open(&data, None).await.unwrap();
        assert!(opened.qsos.is_empty());
        assert_eq!(opened.meta.name, "Log 1");
        let _ = std::fs::remove_dir_all(&data);
    }

    #[tokio::test]
    async fn new_logs_keep_the_old_ones_reopenable() {
        let data = tmp();
        let first = open(&data, None).await.unwrap().meta;
        save(&data, &first.id, &[qso("UR5ZZ", 1000, 1)]).await.unwrap();

        let urc = create(&data, "URC DX".into(), "generic".into(), vec![]).await.unwrap();
        assert!(urc.qsos.is_empty());
        save(&data, &urc.meta.id, &[qso("K1A", 5000, 1), qso("K2B", 6000, 2)]).await.unwrap();
        update_meta(&data, &urc.meta.id, Some(" URC DX RTTY ".into()), Some("cqww-rtty".into()))
            .await
            .unwrap();

        // The newest is what opens on the next start…
        let again = open(&data, None).await.unwrap();
        assert_eq!(again.meta.id, urc.meta.id);
        assert_eq!(again.meta.name, "URC DX RTTY");
        assert_eq!(again.meta.contest_id, "cqww-rtty");
        assert_eq!(again.qsos.len(), 2);
        // …and the first is still there, untouched.
        let back = open(&data, Some(first.id.clone())).await.unwrap();
        assert_eq!(back.qsos.len(), 1);
        assert_eq!(back.qsos[0].call, "UR5ZZ");
        assert_eq!(list(&data).await.unwrap().active, first.id);

        // The open log can't be deleted; another can, and it's kept aside.
        assert!(delete(&data, &first.id).await.is_err());
        delete(&data, &urc.meta.id).await.unwrap();
        assert_eq!(list(&data).await.unwrap().logs.len(), 1);
        assert!(data.join("logs").join(format!("{}.json.deleted", urc.meta.id)).exists());
        // A late save to the deleted log doesn't resurrect it.
        assert!(save(&data, &urc.meta.id, &[]).await.is_err());
        let _ = std::fs::remove_dir_all(&data);
    }

    #[tokio::test]
    async fn ids_cant_escape_the_logs_folder() {
        let data = tmp();
        assert!(open(&data, Some("../../etc/passwd".into())).await.is_err());
        assert!(log_file(&data, "../x").is_err());
        let _ = std::fs::remove_dir_all(&data);
    }
}
