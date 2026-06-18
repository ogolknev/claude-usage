use serde::Deserialize;

/// OAuth-креды Claude Code из macOS Keychain (item `Claude Code-credentials`).
pub struct Creds {
    pub access_token: String,
    #[allow(dead_code)]
    pub refresh_token: Option<String>,
    pub expires_at_ms: Option<i64>,
}

impl Creds {
    /// Истёк ли токен (с запасом 30с). v1 сам токен не рефрешит — это делает
    /// сам Claude Code при следующем запуске, чтобы не сломать его refresh-токен.
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
    let raw = read_secret()?;
    let blob: Blob = serde_json::from_slice(&raw).map_err(|e| format!("parse creds: {e}"))?;
    Ok(Creds {
        access_token: blob.oauth.access_token,
        refresh_token: blob.oauth.refresh_token,
        expires_at_ms: blob.oauth.expires_at,
    })
}

const SERVICE: &str = "Claude Code-credentials";

/// Читаем токен: сначала из macOS Keychain через системный `/usr/bin/security`
/// (стабильный Apple-бинарь — доступ выдаётся ему один раз и навсегда), затем
/// фолбэк на файл `~/.claude/.credentials.json` (Claude Code хранит креды там,
/// если keychain недоступен). Если нет ни того, ни другого — значит в Claude
/// Code нет OAuth-токена (не залогинен через подписку).
fn read_secret() -> Result<Vec<u8>, String> {
    // 1) Keychain.
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
                return Ok(s);
            }
        }
    }
    // 2) Файловый фолбэк.
    let home = std::env::var("HOME").unwrap_or_default();
    let file = std::path::Path::new(&home).join(".claude/.credentials.json");
    if let Ok(data) = std::fs::read(&file) {
        if !data.is_empty() {
            return Ok(data);
        }
    }
    Err("креды Claude Code не найдены — залогинься в Claude Code".into())
}
