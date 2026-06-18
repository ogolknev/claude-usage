use serde::Deserialize;

/// Откуда взят токен — определяет, можем ли мы его рефрешить.
#[derive(Clone, Copy, PartialEq)]
pub enum Source {
    /// Креды Claude Code (keychain или ~/.claude). Рефрешит сам Claude Code.
    ClaudeCode,
    /// Наш токен (логин внутри приложения). Рефрешим сами.
    Ours,
}

/// OAuth-креды для вызова `/api/oauth/usage`.
pub struct Creds {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at_ms: Option<i64>,
    pub source: Source,
}

impl Creds {
    /// Истёк ли токен (с запасом 30с).
    pub fn is_expired(&self) -> bool {
        match self.expires_at_ms {
            Some(ms) => ms <= chrono::Utc::now().timestamp_millis() + 30_000,
            None => false,
        }
    }
}

#[derive(Deserialize)]
struct Blob {
    #[serde(rename = "claudeAiOauth")]
    oauth: OAuth,
}

#[derive(Deserialize)]
struct OAuth {
    #[serde(rename = "accessToken")]
    access_token: String,
    #[serde(rename = "refreshToken")]
    refresh_token: Option<String>,
    #[serde(rename = "expiresAt")]
    expires_at: Option<i64>,
}

pub fn read_creds() -> Result<Creds, String> {
    let (raw, source) = read_secret()?;
    let blob: Blob = serde_json::from_slice(&raw).map_err(|e| format!("parse creds: {e}"))?;
    Ok(Creds {
        access_token: blob.oauth.access_token,
        refresh_token: blob.oauth.refresh_token,
        expires_at_ms: blob.oauth.expires_at,
        source,
    })
}

const SERVICE: &str = "Claude Code-credentials";

/// Ищем токен по приоритету:
/// 1) Keychain Claude Code (через системный `/usr/bin/security`);
/// 2) файл Claude Code `~/.claude/.credentials.json`;
/// 3) наш файл (логин внутри приложения).
fn read_secret() -> Result<(Vec<u8>, Source), String> {
    if let Ok(out) = std::process::Command::new("/usr/bin/security")
        .args(["find-generic-password", "-s", SERVICE, "-w"])
        .output()
    {
        if out.status.success() {
            let mut s = out.stdout;
            while s.last() == Some(&b'\n') {
                s.pop();
            }
            if !s.is_empty() {
                return Ok((s, Source::ClaudeCode));
            }
        }
    }

    let home = std::env::var("HOME").unwrap_or_default();
    let cc_file = std::path::Path::new(&home).join(".claude/.credentials.json");
    if let Ok(data) = std::fs::read(&cc_file) {
        if !data.is_empty() {
            return Ok((data, Source::ClaudeCode));
        }
    }

    if let Ok(data) = std::fs::read(crate::auth::creds_path()) {
        if !data.is_empty() {
            return Ok((data, Source::Ours));
        }
    }

    Err("нет токена — «Войти в Claude…» в меню, либо поставь Claude Code (/login)".into())
}
