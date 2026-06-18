use crate::model::{LimitEntry, LocalUsage, UsageState};
use chrono::Utc;
use tray_icon::menu::MenuItem;

/// Процент сессионного (5ч) лимита — для кольца-иконки. None, если API недоступен.
pub fn session_percent(s: &UsageState) -> Option<f64> {
    s.limits.as_ref().and_then(|l| l.session.as_ref()).map(|e| e.percent)
}

/// Текст рядом с кольцом — только процент 5-часового лимита, без подписи
/// (что это 5ч — ясно из контекста). Недельный лимит живёт в выпадающем меню.
/// При недоступности API — деградирует к токенам за 5ч из локальных логов.
pub fn title_for(s: &UsageState) -> String {
    match session_percent(s) {
        // Формат как у индикатора заряда батареи в строке меню: «NN %».
        Some(p) => format!("{} %", round(p)),
        None => format!("⌁ {}", human_tokens(s.local.window5h_io)),
    }
}

pub fn tooltip_for(s: &UsageState) -> String {
    match &s.limits_err {
        Some(e) => format!("Claude Usage — лимиты недоступны: {e}"),
        None => format!("Claude Usage — обновлено {}", s.fetched_at.format("%H:%M:%S")),
    }
}

pub struct MenuHandles<'a> {
    pub session: &'a MenuItem,
    pub weekly: &'a MenuItem,
    pub scoped: &'a MenuItem,
    pub local: &'a MenuItem,
    pub updated: &'a MenuItem,
}

pub fn apply_menu(s: &UsageState, h: &MenuHandles) {
    match &s.limits {
        Some(l) => {
            h.session.set_text(line_for(l.session.as_ref(), "Сессия (5ч)"));
            h.weekly.set_text(line_for(l.weekly.as_ref(), "Неделя"));
            if let Some(sc) = l.scoped.iter().find(|e| e.percent > 0.0).or(l.scoped.first()) {
                h.scoped.set_text(line_for(Some(sc), &sc.label));
            } else {
                h.scoped.set_text("Модельные лимиты: —");
            }
        }
        None => {
            let err = s.limits_err.as_deref().unwrap_or("недоступно");
            h.session.set_text(format!("Лимиты: {err}"));
            h.weekly.set_text("");
            h.scoped.set_text("");
        }
    }
    h.local.set_text(local_line(&s.local));
    h.updated.set_text(match (&s.last_ok, &s.limits_err) {
        (Some(t), Some(err)) => format!("Обновлено: {} · {err}", t.format("%H:%M:%S")),
        (Some(t), None) => format!("Обновлено: {}", t.format("%H:%M:%S")),
        (None, Some(err)) => format!("Ошибка: {err}"),
        (None, None) => "Обновлено: —".to_string(),
    });
}

fn line_for(e: Option<&LimitEntry>, label: &str) -> String {
    match e {
        Some(e) => {
            let dot = if e.is_active { "● " } else { "" };
            let bang = if is_alarming(e) { " ⚠" } else { "" };
            format!("{dot}{label}: {}%{bang} · {}", round(e.percent), reset_str(e))
        }
        None => format!("{label}: —"),
    }
}

/// Лимит близок к исчерпанию: по severity из API (не "normal") или по проценту.
fn is_alarming(e: &LimitEntry) -> bool {
    (!e.severity.is_empty() && e.severity != "normal") || e.percent >= 80.0
}

fn reset_str(e: &LimitEntry) -> String {
    let Some(reset) = e.resets_at else {
        return "сброс ?".into();
    };
    let now = Utc::now();
    let local = reset.with_timezone(&chrono::Local);
    if reset <= now {
        return format!("сброс {}", local.format("%H:%M"));
    }
    let mins = (reset - now).num_minutes();
    let when = if mins >= 60 * 24 {
        local.format("%a %H:%M").to_string()
    } else {
        local.format("%H:%M").to_string()
    };
    format!("сброс через {} ({when})", dur_str(mins))
}

fn dur_str(mins: i64) -> String {
    if mins >= 60 {
        let h = mins / 60;
        let m = mins % 60;
        if m == 0 {
            format!("{h}ч")
        } else {
            format!("{h}ч {m}м")
        }
    } else {
        format!("{mins}м")
    }
}

fn local_line(u: &LocalUsage) -> String {
    let cost = if crate::pricing::enabled() {
        format!(" (~${:.2})", u.today_cost)
    } else {
        String::new()
    };
    // Основное — input+output; чтение кэша на порядки больше и показывается отдельно.
    format!(
        "Расход вх+вых: сегодня {}{cost} · неделя {} · кэш сегодня {}",
        human_tokens(u.today_io),
        human_tokens(u.week_io),
        human_tokens(u.today_cache),
    )
}

fn round(p: f64) -> i64 {
    p.round() as i64
}

fn human_tokens(t: u64) -> String {
    if t >= 1_000_000 {
        format!("{:.1}M", t as f64 / 1_000_000.0)
    } else if t >= 1_000 {
        format!("{:.0}k", t as f64 / 1_000.0)
    } else {
        format!("{t}")
    }
}
