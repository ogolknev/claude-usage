//! Кэш последних удачных лимитов на диске. Нужен, чтобы кольцо показывалось
//! сразу при запуске (пусть устаревшее) и переживало перезапуски/429, пока не
//! придёт свежий удачный ответ.

use crate::model::Limits;
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize)]
struct Cache {
    limits: Limits,
    at: DateTime<Local>,
}

fn path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join("Library/Caches/com.local.claude-usage/limits.json")
}

pub fn save(limits: &Limits, at: DateTime<Local>) {
    save_at(&path(), limits, at);
}

pub fn load() -> Option<(Limits, DateTime<Local>)> {
    load_at(&path())
}

fn save_at(p: &std::path::Path, limits: &Limits, at: DateTime<Local>) {
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(s) = serde_json::to_string(&Cache {
        limits: limits.clone(),
        at,
    }) {
        let _ = std::fs::write(p, s);
    }
}

fn load_at(p: &std::path::Path) -> Option<(Limits, DateTime<Local>)> {
    let s = std::fs::read_to_string(p).ok()?;
    let c: Cache = serde_json::from_str(&s).ok()?;
    Some((c.limits, c.at))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::LimitEntry;

    #[test]
    fn roundtrip() {
        let entry = LimitEntry {
            kind: "session".into(),
            label: "Сессия (5ч)".into(),
            percent: 16.0,
            resets_at: Some(chrono::Utc::now()),
            is_active: true,
            severity: "normal".into(),
        };
        let mut limits = Limits::default();
        limits.session = Some(entry.clone());
        limits.entries.push(entry);
        let at = chrono::Local::now();

        let p = std::env::temp_dir().join("claude-usage-cache-test.json");
        save_at(&p, &limits, at);
        let (loaded, loaded_at) = load_at(&p).expect("loads");
        assert_eq!(loaded.session.unwrap().percent, 16.0);
        assert_eq!(loaded_at.timestamp(), at.timestamp());
        let _ = std::fs::remove_file(&p);
    }
}
