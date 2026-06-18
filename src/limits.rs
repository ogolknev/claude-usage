use crate::keychain;
use crate::model::{LimitEntry, Limits};
use chrono::{DateTime, Utc};
use serde::Deserialize;

// Эндпоинт и константы подтверждены из бинаря Claude Code (см. план).
const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const BETA: &str = "oauth-2025-04-20";

pub fn client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .user_agent("claude-usage-menubar/0.1")
        .build()
        .expect("http client")
}

#[derive(Deserialize)]
struct Resp {
    #[serde(default)]
    limits: Vec<RawLimit>,
}

#[derive(Deserialize)]
struct RawLimit {
    kind: String,
    percent: f64,
    #[serde(default)]
    severity: String,
    #[serde(default)]
    resets_at: Option<String>,
    #[serde(default)]
    is_active: bool,
    #[serde(default)]
    scope: Option<Scope>,
}

#[derive(Deserialize)]
struct Scope {
    #[serde(default)]
    model: Option<ModelScope>,
}

#[derive(Deserialize)]
struct ModelScope {
    #[serde(default)]
    display_name: Option<String>,
}

pub fn fetch(client: &reqwest::blocking::Client) -> Result<Limits, String> {
    let creds = keychain::read_creds()?;
    if creds.is_expired() {
        return Err("токен истёк (обновится при следующем запуске Claude Code)".into());
    }
    let resp = client
        .get(USAGE_URL)
        .bearer_auth(&creds.access_token)
        .header("anthropic-beta", BETA)
        .header("content-type", "application/json")
        .send()
        .map_err(|e| format!("request: {}", err_chain(&e)))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(format!("HTTP {}", status.as_u16()));
    }
    let body = resp.text().map_err(|e| format!("body: {e}"))?;
    parse(&body)
}

pub fn parse(body: &str) -> Result<Limits, String> {
    let r: Resp = serde_json::from_str(body).map_err(|e| format!("json: {e}"))?;
    Ok(into_limits(r))
}

fn into_limits(r: Resp) -> Limits {
    let mut out = Limits::default();
    for rl in r.limits {
        let label = match rl.kind.as_str() {
            "session" => "Сессия (5ч)".to_string(),
            "weekly_all" => "Неделя".to_string(),
            "weekly_scoped" => {
                let m = rl
                    .scope
                    .as_ref()
                    .and_then(|s| s.model.as_ref())
                    .and_then(|m| m.display_name.clone())
                    .unwrap_or_else(|| "scoped".into());
                format!("Неделя — {m}")
            }
            other => other.to_string(),
        };
        let entry = LimitEntry {
            kind: rl.kind.clone(),
            label,
            percent: rl.percent,
            resets_at: rl.resets_at.as_deref().and_then(parse_ts),
            is_active: rl.is_active,
            severity: rl.severity.clone(),
        };
        match rl.kind.as_str() {
            "session" => out.session = Some(entry.clone()),
            "weekly_all" => out.weekly = Some(entry.clone()),
            "weekly_scoped" => out.scoped.push(entry.clone()),
            _ => {}
        }
        out.entries.push(entry);
    }
    out
}

fn err_chain(e: &dyn std::error::Error) -> String {
    let mut out = e.to_string();
    let mut src = e.source();
    while let Some(s) = src {
        out.push_str(" -> ");
        out.push_str(&s.to_string());
        src = s.source();
    }
    out
}

fn parse_ts(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|d| d.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Реальная форма ответа /api/oauth/usage (срез live-проверки).
    const SAMPLE: &str = r#"{
      "five_hour": {"utilization": 6.0, "resets_at": "2026-06-18T18:09:59.785207+00:00"},
      "seven_day": {"utilization": 8.0, "resets_at": "2026-06-22T09:59:59.785230+00:00"},
      "limits": [
        {"kind":"session","group":"session","percent":6,"severity":"normal","resets_at":"2026-06-18T18:09:59.785207+00:00","scope":null,"is_active":false},
        {"kind":"weekly_all","group":"weekly","percent":8,"severity":"normal","resets_at":"2026-06-22T09:59:59.785230+00:00","scope":null,"is_active":true},
        {"kind":"weekly_scoped","group":"weekly","percent":0,"severity":"normal","resets_at":"2026-06-22T09:59:59.785239+00:00","scope":{"model":{"id":null,"display_name":"Sonnet"},"surface":null},"is_active":false}
      ]
    }"#;

    #[test]
    fn parses_live_shape() {
        let l = parse(SAMPLE).unwrap();
        assert_eq!(l.session.as_ref().unwrap().percent, 6.0);
        assert_eq!(l.weekly.as_ref().unwrap().percent, 8.0);
        assert!(l.weekly.as_ref().unwrap().is_active);
        assert_eq!(l.scoped.len(), 1);
        assert_eq!(l.scoped[0].label, "Неделя — Sonnet");
        assert!(l.session.as_ref().unwrap().resets_at.is_some());
    }
}
