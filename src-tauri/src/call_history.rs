// N1MM+ style Call History file. A plain text file where each line is a
// callsign followed by comma-separated fields; the column order is declared
// by a `!!Order!!,Call,Name,State,...` header (N1MM convention). Lines that
// start with `#` are comments. Lookups return the record as a field → value
// map so the frontend can compose the received exchange for whatever
// contest is active (e.g. Name + State for NAQP, State for Roundup).
//
// Example:
//   # NAQP history
//   !!Order!!,Call,Name,State
//   W1AW,HIRAM,CT
//   VE3XYZ,BOB,ON

use std::collections::HashMap;
use std::path::Path;
use std::sync::RwLock;

use serde::Serialize;
use tracing::info;

/// Default column order when the file carries no `!!Order!!` header — this
/// is N1MM's documented default.
const DEFAULT_ORDER: &[&str] = &[
    "Call", "Name", "Loc1", "Loc2", "Sect", "State", "CK", "BirthDate", "Exch1", "Misc",
    "UserText", "LastUpdateNote",
];

#[derive(Default, Debug, Clone, Serialize)]
pub struct CallHistoryStatus {
    pub count: usize,
    pub path: String,
    pub fields: Vec<String>,
}

#[derive(Default)]
pub struct CallHistory {
    records: RwLock<HashMap<String, HashMap<String, String>>>,
    path: RwLock<String>,
    fields: RwLock<Vec<String>>,
}

impl CallHistory {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn load_file(&self, path: &Path) -> anyhow::Result<usize> {
        let content = tokio::fs::read_to_string(path).await?;
        let (records, fields) = parse(&content);
        let n = records.len();
        *self.records.write().unwrap() = records;
        *self.fields.write().unwrap() = fields;
        *self.path.write().unwrap() = path.display().to_string();
        info!(count = n, path = %path.display(), "call history: loaded");
        Ok(n)
    }

    pub fn clear(&self) {
        self.records.write().unwrap().clear();
        self.fields.write().unwrap().clear();
        self.path.write().unwrap().clear();
    }

    pub fn status(&self) -> CallHistoryStatus {
        CallHistoryStatus {
            count: self.records.read().unwrap().len(),
            path: self.path.read().unwrap().clone(),
            fields: self.fields.read().unwrap().clone(),
        }
    }

    pub fn lookup(&self, call: &str) -> Option<HashMap<String, String>> {
        let c = call.trim().to_ascii_uppercase();
        self.records.read().unwrap().get(&c).cloned()
    }
}

/// Parse file text into (records keyed by call, ordered field names other
/// than Call).
fn parse(content: &str) -> (HashMap<String, HashMap<String, String>>, Vec<String>) {
    let mut order: Vec<String> = DEFAULT_ORDER.iter().map(|s| s.to_string()).collect();
    let mut records: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut seen_fields: Vec<String> = Vec::new();

    for raw in content.lines() {
        let line = raw.trim_end_matches('\r').trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.to_ascii_lowercase().starts_with("!!order!!") {
            order = line
                .split(',')
                .skip(1)
                .map(|f| f.trim().to_string())
                .filter(|f| !f.is_empty())
                .collect();
            continue;
        }
        if line.starts_with('!') {
            continue;
        }
        let mut parts = line.split(',').map(|p| p.trim());
        let call = match parts.next() {
            Some(c) if !c.is_empty() => c.to_ascii_uppercase(),
            _ => continue,
        };
        let mut rec: HashMap<String, String> = HashMap::new();
        // Column 0 is always Call regardless of the declared order; the
        // remaining columns follow the order list after its "Call" entry.
        let names: Vec<&String> = order.iter().filter(|f| !f.eq_ignore_ascii_case("Call")).collect();
        for (i, val) in parts.enumerate() {
            if val.is_empty() {
                continue;
            }
            let name = match names.get(i) {
                Some(n) => (*n).clone(),
                None => format!("Field{}", i + 1),
            };
            if !seen_fields.iter().any(|f| f.eq_ignore_ascii_case(&name)) {
                seen_fields.push(name.clone());
            }
            rec.insert(name, val.to_ascii_uppercase());
        }
        records.insert(call, rec);
    }
    (records, seen_fields)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_order_header_and_records() {
        let txt = "# comment\n!!Order!!,Call,Name,State\nW1AW,HIRAM,CT\nve3xyz,Bob,\n";
        let (recs, fields) = parse(txt);
        assert_eq!(recs.len(), 2);
        assert_eq!(recs["W1AW"]["Name"], "HIRAM");
        assert_eq!(recs["W1AW"]["State"], "CT");
        assert_eq!(recs["VE3XYZ"]["Name"], "BOB");
        assert!(recs["VE3XYZ"].get("State").is_none());
        assert_eq!(fields, vec!["Name".to_string(), "State".to_string()]);
    }

    #[test]
    fn default_order_without_header() {
        let (recs, _) = parse("K1ABC,JOE,,,,MA\n");
        assert_eq!(recs["K1ABC"]["Name"], "JOE");
        assert_eq!(recs["K1ABC"]["State"], "MA");
    }
}
