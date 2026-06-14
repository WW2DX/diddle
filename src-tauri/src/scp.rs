// Super Check Partial database. Loads callsigns from disk (user-supplied
// MASTER.SCP from supercheckpartial.com) and/or from a tiny embedded
// starter list, then offers fast prefix/substring search for entry-field
// autocomplete and an exact-match check for filtering decoder spots.
//
// Uses a std RwLock (not tokio) so the hot audio path in the multi-decoder
// can do exact-match lookups synchronously without entering an async
// context. Lookups are binary searches — microseconds — so brief lock
// contention is a non-issue.

use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::time::Duration;

use serde::Serialize;
use tracing::info;

const STARTER: &str = include_str!("scp_starter.txt");

/// Canonical source for the community Super Check Partial database.
pub const REMOTE_URL: &str = "https://www.supercheckpartial.com/MASTER.SCP";

/// Download MASTER.SCP from supercheckpartial.com to `dest`. Returns the
/// number of bytes written. Caller is responsible for creating the parent
/// directory.
pub async fn download_master_scp(dest: &Path) -> anyhow::Result<usize> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent(concat!("diddle/", env!("CARGO_PKG_VERSION")))
        .build()?;
    let resp = client.get(REMOTE_URL).send().await?.error_for_status()?;
    let body = resp.bytes().await?;
    let n = body.len();
    if n < 10_000 {
        // Real MASTER.SCP is hundreds of KB. Anything tiny is almost certainly
        // an error page from a CDN/captive portal — bail out before we
        // overwrite a good file.
        anyhow::bail!("MASTER.SCP response too small ({n} bytes) — refusing to write");
    }
    tokio::fs::write(dest, &body).await?;
    info!(path = %dest.display(), bytes = n, "scp: downloaded MASTER.SCP");
    Ok(n)
}

/// Resolves the on-disk location where the auto-downloaded MASTER.SCP lives.
pub fn cached_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    use tauri::Manager;
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app_data_dir: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("create_dir_all: {e}"))?;
    Ok(dir.join("MASTER.SCP"))
}

#[derive(Default, Debug, Clone, Serialize)]
pub struct ScpStatus {
    pub count: usize,
    pub source: String, // "starter" | "file:/path/..."
}

pub struct ScpDb {
    inner: RwLock<Vec<String>>,
    source: RwLock<String>,
}

impl ScpDb {
    pub fn new() -> Self {
        let mut calls: Vec<String> = STARTER
            .lines()
            .map(|l| l.trim().to_ascii_uppercase())
            .filter(|l| Self::looks_like_call(l))
            .collect();
        calls.sort();
        calls.dedup();
        info!(count = calls.len(), "scp: loaded starter list");
        Self {
            inner: RwLock::new(calls),
            source: RwLock::new("starter".to_string()),
        }
    }

    pub async fn load_file(&self, path: &Path) -> anyhow::Result<usize> {
        let content = tokio::fs::read_to_string(path).await?;
        let mut calls: Vec<String> = content
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty() && !l.starts_with('!') && !l.starts_with('#'))
            .map(|l| l.to_ascii_uppercase())
            .filter(|l| Self::looks_like_call(l))
            .collect();
        calls.sort();
        calls.dedup();
        let n = calls.len();
        *self.inner.write().unwrap() = calls;
        *self.source.write().unwrap() = format!("file:{}", path.display());
        info!(count = n, path = %path.display(), "scp: loaded file");
        Ok(n)
    }

    pub fn status(&self) -> ScpStatus {
        ScpStatus {
            count: self.inner.read().unwrap().len(),
            source: self.source.read().unwrap().clone(),
        }
    }

    /// Exact membership test (binary search). Used to filter decoder spots
    /// so only real callsigns reach the bandmap.
    pub fn contains(&self, call: &str) -> bool {
        let c = call.trim().to_ascii_uppercase();
        self.inner.read().unwrap().binary_search(&c).is_ok()
    }

    pub fn search(&self, query: &str, max: usize) -> Vec<String> {
        let q = query.trim().to_ascii_uppercase();
        if q.len() < 2 {
            return Vec::new();
        }
        let calls = self.inner.read().unwrap();
        let mut results: Vec<String> = Vec::with_capacity(max);

        let start = calls.partition_point(|s| s.as_str() < q.as_str());
        for c in &calls[start..] {
            if !c.starts_with(&q) {
                break;
            }
            results.push(c.clone());
            if results.len() >= max {
                return results;
            }
        }

        if results.len() < max {
            for c in calls.iter() {
                if c.starts_with(&q) {
                    continue;
                }
                if c.contains(&q) {
                    results.push(c.clone());
                    if results.len() >= max {
                        return results;
                    }
                }
            }
        }
        results
    }

    fn looks_like_call(s: &str) -> bool {
        let len = s.len();
        if !(3..=12).contains(&len) {
            return false;
        }
        let mut has_letter = false;
        let mut has_digit = false;
        for c in s.chars() {
            if c.is_ascii_uppercase() {
                has_letter = true;
            } else if c.is_ascii_digit() {
                has_digit = true;
            } else if c == '/' {
            } else {
                return false;
            }
        }
        has_letter && has_digit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starter_loaded() {
        let db = ScpDb::new();
        assert!(db.status().count > 50);
    }

    #[test]
    fn prefix_match() {
        let db = ScpDb::new();
        let r = db.search("K1A", 5);
        assert!(r.iter().any(|c| c == "K1AR"));
    }

    #[test]
    fn substring_match_fallback() {
        let db = ScpDb::new();
        let r = db.search("150ITU", 5);
        assert!(r.iter().any(|c| c == "SX150ITU"));
    }

    #[test]
    fn too_short_no_results() {
        let db = ScpDb::new();
        assert!(db.search("K", 5).is_empty());
    }

    #[test]
    fn contains_exact() {
        let db = ScpDb::new();
        assert!(db.contains("K5ZD"));
        assert!(db.contains("k5zd")); // case-insensitive
        assert!(!db.contains("JD2")); // garbage partial
        assert!(!db.contains("ISFXG9"));
    }
}
