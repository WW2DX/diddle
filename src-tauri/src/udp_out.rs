// QSOs out over UDP, in the two formats other ham software listens for:
//
// - WSJT-X (NetworkMessage, default port 2237): "QSO Logged" (type 5) and
//   "Logged ADIF" (type 12) for each new QSO, and a Heartbeat (type 0) every
//   15 s so listeners see a live client. GridTracker, JTAlert, Log4OM and the
//   World Radio League app take these. QDataStream encoding, big-endian, as
//   WSJT-X's Network/NetworkMessage.hpp documents it (schema 2).
// - N1MM Logger+ "contactinfo" XML (default port 12060), plus contactreplace
//   for an edited QSO and contactdelete for a deleted one — what contest
//   tools and N1MM-aware loggers take.
//
// Each format goes to a list of host:port addresses (a broadcast address
// works too); nothing is sent unless the operator turns it on in Settings.

use std::net::{ToSocketAddrs, UdpSocket};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tracing::warn;

const WSJTX_MAGIC: u32 = 0xADBC_CBDA;
const WSJTX_SCHEMA: u32 = 2;
/// Our client id in WSJT-X messages.
const CLIENT_ID: &str = "Diddle";
const HEARTBEAT_EVERY: Duration = Duration::from_secs(15);

/// Where to send what (from Settings).
#[derive(Deserialize, Serialize, Clone, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UdpConfig {
    pub wsjtx_enabled: bool,
    /// host:port list, comma- or space-separated.
    pub wsjtx_addrs: String,
    pub n1mm_enabled: bool,
    pub n1mm_addrs: String,
}

/// One QSO as the frontend describes it — everything both formats need,
/// already worked out (country, zone, points) where the frontend knows it.
#[derive(Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct UdpQso {
    pub id: String,
    pub ts: i64, // unix ms, UTC
    pub call: String,
    pub freq_hz: u64,
    pub band: String, // "20m"
    pub mode: String, // "RTTY"
    pub rst_sent: String,
    pub rst_rcvd: String,
    pub exch_sent: String,
    pub exch_rcvd: String,
    pub serial_sent: u32,
    pub serial_rcvd: Option<u32>,
    pub my_call: String,
    pub my_grid: String,
    pub dx_grid: String,
    pub name: String,
    pub comment: String,
    pub country_prefix: String,
    pub wpx_prefix: String,
    pub continent: String,
    pub zone: u32,
    pub points: u32,
    pub is_mult: bool,
    pub run: bool,
    pub contest: String, // Cabrillo name, e.g. CQ-WW-RTTY
    /// The QSO as an ADIF file (for WSJT-X "Logged ADIF").
    pub adif: String,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum QsoEvent {
    Add,
    Replace,
    Delete,
}

// ---- WSJT-X QDataStream encoding ----

struct QStream(Vec<u8>);

impl QStream {
    fn new(msg_type: u32) -> Self {
        let mut s = QStream(Vec::new());
        s.u32(WSJTX_MAGIC);
        s.u32(WSJTX_SCHEMA);
        s.u32(msg_type);
        s.utf8(CLIENT_ID);
        s
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_be_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_be_bytes());
    }
    /// utf8 = QByteArray: quint32 length, then the bytes.
    fn utf8(&mut self, s: &str) {
        self.u32(s.len() as u32);
        self.0.extend_from_slice(s.as_bytes());
    }
    /// QDateTime (Qt_5_4 format): QDate as qint64 Julian day, QTime as
    /// quint32 ms since midnight, timespec quint8 1 = UTC.
    fn datetime_utc(&mut self, unix_ms: i64) {
        let day = unix_ms.div_euclid(86_400_000);
        let ms = unix_ms.rem_euclid(86_400_000) as u32;
        self.0.extend_from_slice(&(day + 2_440_588).to_be_bytes());
        self.u32(ms);
        self.0.push(1);
    }
}

pub fn wsjtx_heartbeat() -> Vec<u8> {
    let mut s = QStream::new(0);
    s.u32(3); // maximum schema number we understand
    s.utf8(env!("CARGO_PKG_VERSION"));
    s.utf8("");
    s.0
}

pub fn wsjtx_qso_logged(q: &UdpQso) -> Vec<u8> {
    let mut s = QStream::new(5);
    s.datetime_utc(q.ts); // time off
    s.utf8(&q.call);
    s.utf8(&q.dx_grid);
    s.u64(q.freq_hz);
    s.utf8(&q.mode);
    s.utf8(&q.rst_sent);
    s.utf8(&q.rst_rcvd);
    s.utf8(""); // tx power
    s.utf8(&q.comment);
    s.utf8(&q.name);
    s.datetime_utc(q.ts); // time on
    s.utf8(""); // operator call
    s.utf8(&q.my_call);
    s.utf8(&q.my_grid);
    s.utf8(&q.exch_sent);
    s.utf8(&q.exch_rcvd);
    s.utf8(""); // ADIF propagation mode
    s.0
}

pub fn wsjtx_logged_adif(q: &UdpQso) -> Vec<u8> {
    let mut s = QStream::new(12);
    s.utf8(&q.adif);
    s.0
}

// ---- N1MM XML ----

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn n1mm_timestamp(unix_ms: i64) -> String {
    // YYYY-MM-DD HH:MM:SS, UTC — civil date from days since the epoch.
    let secs = unix_ms.div_euclid(1000);
    let (days, sod) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}", sod / 3600, sod % 3600 / 60, sod % 60)
}

/// "20m" → "14", "40m" → "7", "160m" → "1.8" (N1MM's band field, MHz).
fn n1mm_band(band: &str) -> &'static str {
    match band {
        "160m" => "1.8",
        "80m" => "3.5",
        "40m" => "7",
        "30m" => "10",
        "20m" => "14",
        "17m" => "18",
        "15m" => "21",
        "12m" => "24",
        "10m" => "28",
        "6m" => "50",
        "2m" => "144",
        _ => "",
    }
}

fn n1mm_id(id: &str) -> String {
    id.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_lowercase()
}

pub fn n1mm_xml(ev: QsoEvent, q: &UdpQso, station: &str) -> String {
    let e = xml_escape;
    let ts = n1mm_timestamp(q.ts);
    let band = n1mm_band(&q.band);
    if ev == QsoEvent::Delete {
        return format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<contactdelete>\n\t<app>Diddle</app>\n\t<timestamp>{ts}</timestamp>\n\t<mycall>{}</mycall>\n\t<band>{band}</band>\n\t<call>{}</call>\n\t<contestnr>1</contestnr>\n\t<StationName>{}</StationName>\n\t<ID>{}</ID>\n</contactdelete>\n",
            e(&q.my_call), e(&q.call), e(station), n1mm_id(&q.id),
        );
    }
    let tag = if ev == QsoEvent::Replace { "contactreplace" } else { "contactinfo" };
    let f10 = q.freq_hz / 10; // tens of Hz
    let mut x = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
    x.push_str(&format!("<{tag}>\n"));
    let mut el = |k: &str, v: &str| x.push_str(&format!("\t<{k}>{}</{k}>\n", e(v)));
    el("app", "Diddle");
    el("contestname", &q.contest);
    el("contestnr", "1");
    el("timestamp", &ts);
    el("mycall", &q.my_call);
    el("band", band);
    el("rxfreq", &f10.to_string());
    el("txfreq", &f10.to_string());
    el("operator", "");
    el("mode", &q.mode);
    el("call", &q.call);
    el("countryprefix", &q.country_prefix);
    el("wpxprefix", &q.wpx_prefix);
    el("stationprefix", &q.my_call);
    el("continent", &q.continent);
    el("snt", &q.rst_sent);
    el("sntnr", &q.serial_sent.to_string());
    el("rcv", &q.rst_rcvd);
    el("rcvnr", &q.serial_rcvd.unwrap_or(0).to_string());
    el("gridsquare", &q.dx_grid);
    el("exchange1", &q.exch_rcvd);
    el("section", "");
    el("comment", &q.comment);
    el("qth", "");
    el("name", &q.name);
    el("power", "");
    el("misctext", "");
    el("zone", &q.zone.to_string());
    el("prec", "");
    el("ck", "0");
    el("ismultiplier1", if q.is_mult { "1" } else { "0" });
    el("ismultiplier2", "0");
    el("ismultiplier3", "0");
    el("points", &q.points.to_string());
    el("radionr", "1");
    el("run1run2", "1");
    el("RoverLocation", "");
    el("RadioInterfaced", "1");
    el("NetworkedCompNr", "0");
    el("IsOriginal", "True");
    el("NetBiosName", station);
    el("IsRunQSO", if q.run { "1" } else { "0" });
    el("StationName", station);
    el("ID", &n1mm_id(&q.id));
    el("IsClaimedQso", "1");
    el("SentExchange", &q.exch_sent);
    x.push_str(&format!("</{tag}>\n"));
    x
}

// ---- Sending ----

struct State {
    cfg: UdpConfig,
    socket: Option<UdpSocket>,
}

fn state() -> &'static Mutex<State> {
    static S: OnceLock<Mutex<State>> = OnceLock::new();
    S.get_or_init(|| {
        // Heartbeats for WSJT-X listeners, for as long as the app runs.
        std::thread::Builder::new()
            .name("udp-heartbeat".into())
            .spawn(|| loop {
                std::thread::sleep(HEARTBEAT_EVERY);
                let addrs = {
                    let s = state().lock().unwrap();
                    if !s.cfg.wsjtx_enabled {
                        continue;
                    }
                    s.cfg.wsjtx_addrs.clone()
                };
                let _ = send_to(&addrs, &wsjtx_heartbeat());
            })
            .ok();
        Mutex::new(State { cfg: UdpConfig::default(), socket: None })
    })
}

fn split_addrs(list: &str) -> impl Iterator<Item = &str> {
    list.split(|c: char| c == ',' || c.is_whitespace()).filter(|a| !a.is_empty())
}

/// Send to every address in the list. Errors name the address.
fn send_to(addrs: &str, bytes: &[u8]) -> Result<(), String> {
    let mut st = state().lock().unwrap();
    if st.socket.is_none() {
        let s = UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("UDP socket: {e}"))?;
        let _ = s.set_broadcast(true);
        st.socket = Some(s);
    }
    let sock = st.socket.as_ref().unwrap();
    let mut errors = Vec::new();
    for a in split_addrs(addrs) {
        match a.to_socket_addrs().map(|mut it| it.next()) {
            Ok(Some(sa)) => {
                if let Err(e) = sock.send_to(bytes, sa) {
                    errors.push(format!("{a}: {e}"));
                }
            }
            Ok(None) => errors.push(format!("{a}: no address")),
            Err(e) => errors.push(format!("{a}: {e}")),
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

pub fn set_config(cfg: UdpConfig) {
    let wsjtx_now_on = cfg.wsjtx_enabled;
    let addrs = cfg.wsjtx_addrs.clone();
    state().lock().unwrap().cfg = cfg;
    // Announce ourselves straight away rather than in up to 15 s.
    if wsjtx_now_on {
        let _ = send_to(&addrs, &wsjtx_heartbeat());
    }
}

/// Broadcast a logged / edited / deleted QSO in every enabled format.
pub fn send_qso(ev: QsoEvent, q: &UdpQso) -> Result<(), String> {
    let cfg = state().lock().unwrap().cfg.clone();
    let mut errors = Vec::new();
    // WSJT-X has no edit or delete messages: new QSOs only.
    if cfg.wsjtx_enabled && ev == QsoEvent::Add {
        for msg in [wsjtx_qso_logged(q), wsjtx_logged_adif(q)] {
            if let Err(e) = send_to(&cfg.wsjtx_addrs, &msg) {
                errors.push(format!("WSJT-X {e}"));
                break;
            }
        }
    }
    if cfg.n1mm_enabled {
        let station = hostname();
        if let Err(e) = send_to(&cfg.n1mm_addrs, n1mm_xml(ev, q, &station).as_bytes()) {
            errors.push(format!("N1MM {e}"));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        let msg = errors.join("; ");
        warn!("udp out: {msg}");
        Err(msg)
    }
}

fn hostname() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "DIDDLE".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A reader for our own QDataStream output, to check field order.
    struct R<'a>(&'a [u8]);
    impl R<'_> {
        fn u32(&mut self) -> u32 {
            let v = u32::from_be_bytes(self.0[..4].try_into().unwrap());
            self.0 = &self.0[4..];
            v
        }
        fn u64(&mut self) -> u64 {
            let v = u64::from_be_bytes(self.0[..8].try_into().unwrap());
            self.0 = &self.0[8..];
            v
        }
        fn utf8(&mut self) -> String {
            let n = self.u32() as usize;
            let s = String::from_utf8(self.0[..n].to_vec()).unwrap();
            self.0 = &self.0[n..];
            s
        }
        fn datetime(&mut self) -> (i64, u32, u8) {
            let jd = i64::from_be_bytes(self.0[..8].try_into().unwrap());
            self.0 = &self.0[8..];
            let ms = self.u32();
            let spec = self.0[0];
            self.0 = &self.0[1..];
            (jd, ms, spec)
        }
    }

    fn qso() -> UdpQso {
        UdpQso {
            id: "3f2a-91c0".into(),
            // 2026-10-16 19:35:12 UTC
            ts: 1_792_179_312_000,
            call: "DL1ABC".into(),
            freq_hz: 14_084_875,
            band: "20m".into(),
            mode: "RTTY".into(),
            rst_sent: "599".into(),
            rst_rcvd: "599".into(),
            exch_sent: "05".into(),
            exch_rcvd: "14".into(),
            serial_sent: 42,
            my_call: "WW2DX".into(),
            contest: "CQ-WW-RTTY".into(),
            country_prefix: "DL".into(),
            wpx_prefix: "DL1".into(),
            continent: "EU".into(),
            zone: 14,
            points: 3,
            is_mult: true,
            run: true,
            adif: "<EOH>\n<CALL:6>DL1ABC <EOR>\n".into(),
            ..Default::default()
        }
    }

    #[test]
    fn wsjtx_qso_logged_layout() {
        let b = wsjtx_qso_logged(&qso());
        let mut r = R(&b);
        assert_eq!(r.u32(), 0xADBC_CBDA);
        assert_eq!(r.u32(), 2);
        assert_eq!(r.u32(), 5);
        assert_eq!(r.utf8(), "Diddle");
        // 2026-10-16 = JD 2461330; 19:35:12 = 70 512 000 ms; UTC.
        assert_eq!(r.datetime(), (2_461_330, 70_512_000, 1));
        assert_eq!(r.utf8(), "DL1ABC");
        assert_eq!(r.utf8(), ""); // grid
        assert_eq!(r.u64(), 14_084_875);
        assert_eq!(r.utf8(), "RTTY");
        assert_eq!(r.utf8(), "599");
        assert_eq!(r.utf8(), "599");
        for _ in 0..3 {
            r.utf8(); // power, comments, name
        }
        assert_eq!(r.datetime(), (2_461_330, 70_512_000, 1));
        assert_eq!(r.utf8(), ""); // operator
        assert_eq!(r.utf8(), "WW2DX");
        assert_eq!(r.utf8(), ""); // my grid
        assert_eq!(r.utf8(), "05");
        assert_eq!(r.utf8(), "14");
        assert_eq!(r.utf8(), ""); // propagation
        assert!(r.0.is_empty(), "{} bytes left over", r.0.len());
    }

    #[test]
    fn wsjtx_adif_and_heartbeat() {
        let b = wsjtx_logged_adif(&qso());
        let mut r = R(&b);
        assert_eq!((r.u32(), r.u32(), r.u32()), (0xADBC_CBDA, 2, 12));
        assert_eq!(r.utf8(), "Diddle");
        assert_eq!(r.utf8(), "<EOH>\n<CALL:6>DL1ABC <EOR>\n");
        let h = wsjtx_heartbeat();
        let mut r = R(&h);
        assert_eq!((r.u32(), r.u32(), r.u32()), (0xADBC_CBDA, 2, 0));
        assert_eq!(r.utf8(), "Diddle");
        assert_eq!(r.u32(), 3);
    }

    #[test]
    fn n1mm_contactinfo_and_delete() {
        let x = n1mm_xml(QsoEvent::Add, &qso(), "SHACK");
        assert!(x.contains("<contactinfo>"));
        assert!(x.contains("<timestamp>2026-10-16 19:35:12</timestamp>"));
        assert!(x.contains("<band>14</band>"));
        assert!(x.contains("<rxfreq>1408487</rxfreq>")); // tens of Hz
        assert!(x.contains("<call>DL1ABC</call>"));
        assert!(x.contains("<zone>14</zone>"));
        assert!(x.contains("<ismultiplier1>1</ismultiplier1>"));
        assert!(x.contains("<ID>3f2a91c0</ID>"));
        let r = n1mm_xml(QsoEvent::Replace, &qso(), "SHACK");
        assert!(r.contains("<contactreplace>") && r.trim_end().ends_with("</contactreplace>"));
        let d = n1mm_xml(QsoEvent::Delete, &qso(), "SHACK");
        assert!(d.contains("<contactdelete>") && d.contains("<ID>3f2a91c0</ID>"));
    }

    #[test]
    fn sends_to_a_listener() {
        let rx = UdpSocket::bind("127.0.0.1:0").unwrap();
        rx.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let port = rx.local_addr().unwrap().port();
        set_config(UdpConfig {
            wsjtx_enabled: true,
            wsjtx_addrs: format!("127.0.0.1:{port}"),
            ..Default::default()
        });
        let mut buf = [0u8; 2048];
        // The heartbeat that set_config sends, then the QSO's two messages.
        let n = rx.recv(&mut buf).unwrap();
        assert_eq!(&buf[8..12], &0u32.to_be_bytes());
        send_qso(QsoEvent::Add, &qso()).unwrap();
        let n5 = rx.recv(&mut buf).unwrap();
        assert_eq!(&buf[8..12], &5u32.to_be_bytes());
        let _ = (n, n5);
        let _ = rx.recv(&mut buf).unwrap();
        assert_eq!(&buf[8..12], &12u32.to_be_bytes());
        // Edits don't go to WSJT-X.
        send_qso(QsoEvent::Replace, &qso()).unwrap();
        assert!(rx.recv(&mut buf).is_err());
        set_config(UdpConfig::default());
    }
}
