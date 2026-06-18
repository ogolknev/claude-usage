use chrono::{DateTime, Local, Utc};
use serde::{Deserialize, Serialize};

/// Одна строка лимита из ответа `/api/oauth/usage` (поле `limits[]`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LimitEntry {
    /// Тип лимита из API (session/weekly_all/weekly_scoped) — для отладки/группировки.
    #[allow(dead_code)]
    pub kind: String,
    pub label: String,
    /// Утилизация в процентах 0..100 (API отдаёт уже проценты).
    pub percent: f64,
    pub resets_at: Option<DateTime<Utc>>,
    pub is_active: bool,
    pub severity: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Limits {
    pub entries: Vec<LimitEntry>,
    pub session: Option<LimitEntry>,
    pub weekly: Option<LimitEntry>,
    /// weekly_scoped — лимиты по конкретным моделям (Opus/Sonnet).
    pub scoped: Vec<LimitEntry>,
}

/// Локальный расход из JSONL-логов Claude Code (фолбэк, считается без сети).
#[derive(Clone, Debug, Default)]
pub struct LocalUsage {
    pub window5h_tokens: u64,
    pub today_tokens: u64,
    pub week_tokens: u64,
    /// Оценка $ — ненулевая только если в pricing.rs заполнены цены.
    pub today_cost: f64,
    pub week_cost: f64,
}

#[derive(Clone, Debug)]
pub struct UsageState {
    pub limits: Option<Limits>,
    pub limits_err: Option<String>,
    pub local: LocalUsage,
    pub fetched_at: DateTime<Local>,
    /// Время последнего УСПЕШНОГО получения лимитов (None — ещё ни разу).
    pub last_ok: Option<DateTime<Local>>,
}
