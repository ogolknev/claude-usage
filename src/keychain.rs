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

fn read_secret() -> Result<Vec<u8>, String> {
    // Прямое чтение из Keychain: ACL привязывается к нашему подписанному бинарю.
    let account = std::env::var("USER").unwrap_or_default();
    if !account.is_empty() {
        if let Ok(v) = security_framework::passwords::get_generic_password(SERVICE, &account) {
            return Ok(v);
        }
    }
    // Фолбэк: `security` ищет item по одному service (без точного account).
    let out = std::process::Command::new("/usr/bin/security")
        .args(["find-generic-password", "-s", SERVICE, "-w"])
        .output()
        .map_err(|e| format!("security: {e}"))?;
    if !out.status.success() {
        return Err("keychain item недоступен".into());
    }
    let mut s = out.stdout;
    while s.last() == Some(&b'\n') {
        s.pop();
    }
    Ok(s)
}
