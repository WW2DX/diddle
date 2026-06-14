// TCI text protocol parser.
//
// TCI messages are ASCII, terminated by ';'. Format:
//   name;                    (no args)
//   name:arg1,arg2,...;      (comma-separated args)
//
// Examples:
//   start;
//   vfo:0,0,14200000;
//   mode:0,digu;
//   trx:0,true;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub name: String,
    pub args: Vec<String>,
}

impl Message {
    pub fn parse(raw: &str) -> Option<Self> {
        let line = raw.trim().trim_end_matches(';');
        if line.is_empty() {
            return None;
        }
        let (name, args) = match line.split_once(':') {
            Some((n, rest)) => (
                n.trim().to_ascii_lowercase(),
                rest.split(',').map(|s| s.trim().to_string()).collect(),
            ),
            None => (line.trim().to_ascii_lowercase(), Vec::new()),
        };
        if name.is_empty() {
            return None;
        }
        Some(Self { name, args })
    }

    pub fn arg_u8(&self, idx: usize) -> Option<u8> {
        self.args.get(idx)?.parse().ok()
    }

    pub fn arg_u64(&self, idx: usize) -> Option<u64> {
        self.args.get(idx)?.parse().ok()
    }

    pub fn arg_bool(&self, idx: usize) -> Option<bool> {
        match self.args.get(idx)?.to_ascii_lowercase().as_str() {
            "true" | "1" => Some(true),
            "false" | "0" => Some(false),
            _ => None,
        }
    }

    pub fn arg_str(&self, idx: usize) -> Option<&str> {
        self.args.get(idx).map(|s| s.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bare_command() {
        let m = Message::parse("start;").unwrap();
        assert_eq!(m.name, "start");
        assert!(m.args.is_empty());
    }

    #[test]
    fn parses_command_with_args() {
        let m = Message::parse("vfo:0,0,14200000;").unwrap();
        assert_eq!(m.name, "vfo");
        assert_eq!(m.args, vec!["0", "0", "14200000"]);
        assert_eq!(m.arg_u8(0), Some(0));
        assert_eq!(m.arg_u64(2), Some(14200000));
    }

    #[test]
    fn parses_bool_arg() {
        let m = Message::parse("trx:0,true;").unwrap();
        assert_eq!(m.arg_bool(1), Some(true));
    }

    #[test]
    fn ignores_empty() {
        assert!(Message::parse("").is_none());
        assert!(Message::parse(";").is_none());
        assert!(Message::parse("   ").is_none());
    }

    #[test]
    fn lowercases_name() {
        let m = Message::parse("VFO:0,0,14200000;").unwrap();
        assert_eq!(m.name, "vfo");
    }
}
