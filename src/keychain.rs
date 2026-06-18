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

/// Читаем токен через системный `/usr/bin/security`, а не напрямую из нашего
/// процесса. Это стабильный Apple-бинарь: доступ к итему выдаётся ему один раз
/// («Всегда разрешать»/Touch ID в диалоге) и сохраняется навсегда — даже после
/// пересборки приложения. Прямой доступ так не умеет: его ACL-запись привязана
/// к подписи бинаря и слетает при каждом ребилде ad-hoc, вызывая новый промпт.
fn read_secret() -> Result<Vec<u8>, String> {
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
