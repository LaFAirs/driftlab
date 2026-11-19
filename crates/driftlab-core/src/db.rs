//! Controller database loaded from `assets/controllers.toml`.

use crate::model::Family;
use serde::Deserialize;

const BUILTIN: &str = include_str!("../../../assets/controllers.toml");

#[derive(Debug, Clone, Deserialize)]
pub struct Entry {
    pub name: String,
    pub family: Family,
    pub vid: u16,
    pub pids: Vec<u16>,
    pub theme: String,
    pub model: String,
}

#[derive(Debug, Deserialize)]
struct File {
    controller: Vec<Entry>,
}

#[derive(Debug, Clone)]
pub struct ControllerDb {
    pub entries: Vec<Entry>,
}

impl ControllerDb {
    pub fn builtin() -> Self {
        Self::parse(BUILTIN).unwrap_or(Self { entries: Vec::new() })
    }

    pub fn parse(text: &str) -> Result<Self, toml::de::Error> {
        let f: File = toml::from_str(text)?;
        Ok(Self { entries: f.controller })
    }

    pub fn lookup(&self, vid: u16, pid: u16) -> Option<&Entry> {
        self.entries.iter().find(|e| e.vid == vid && e.pids.contains(&pid))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_parses_and_finds() {
        let db = ControllerDb::builtin();
        assert!(db.entries.len() >= 7);
        assert_eq!(db.lookup(0x054C, 0x0CE6).map(|e| e.family), Some(Family::DualSense));
        assert_eq!(db.lookup(0x054C, 0x0DF2).map(|e| e.family), Some(Family::DualSenseEdge));
        assert_eq!(db.lookup(0x054C, 0x09CC).map(|e| e.family), Some(Family::DualShock4));
        assert_eq!(db.lookup(0x045E, 0x0B12).map(|e| e.family), Some(Family::Xbox));
        assert!(db.lookup(0x1234, 0x5678).is_none());
    }

    #[test]
    fn no_duplicate_ids() {
        let db = ControllerDb::builtin();
        let mut seen = std::collections::HashSet::new();
        for e in &db.entries {
            for p in &e.pids {
                assert!(seen.insert((e.vid, *p)), "duplicate {:04x}:{:04x}", e.vid, p);
            }
        }
    }
}
