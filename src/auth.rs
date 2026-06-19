//! Встроенный OAuth-логин (PKCE) — чтобы работать без Claude Code/Desktop.
//! Тот же флоу, что у Claude Code: открываем браузер на authorize, локальный
//! сервер ловит redirect с кодом, меняем код на токен. Свой токен храним в
//! `~/Library/Application Support/claude-usage/credentials.json` (формат как у
//! Claude Code: `{claudeAiOauth:{accessToken,refreshToken,expiresAt}}`) и сами
//! его рефрешим.

use crate::keychain::{Creds, Source};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::Deserialize;
use std::io::Read;
use std::path::PathBuf;

const CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
const AUTHORIZE_URL: &str = "https://claude.ai/oauth/authorize";
const TOKEN_URL: &str = "https://platform.claude.com/v1/oauth/token";
// Ручной флоу подписки: после авторизации страница показывает код для вставки.
const MANUAL_REDIRECT: &str = "https://platform.claude.com/oauth/code/callback";
// Scope подписочного логина. ВАЖНО: без `org:create_api_key` — это org/console
// scope, и с ним claude.ai отвергает грант подписки («Invalid request format»).
const SCOPE: &str = "user:inference user:profile user:sessions:claude_code user:mcp_servers";

pub fn creds_path() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default())
        .join("Library/Application Support/claude-usage/credentials.json")
}

#[derive(Deserialize)]
struct TokenResp {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
}

/// Ручной флоу подписки (как у рабочих реализаций Claude Code OAuth): браузер →
/// страница показывает код `code#state` → пользователь вставляет его в диалог →
/// обмен на токен.
pub fn login(client: &reqwest::blocking::Client) -> Result<(), String> {
    let verifier = b64url(&rand_bytes(32));
    let state = b64url(&rand_bytes(16));
    let challenge = {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(verifier.as_bytes());
        b64url(&h.finalize())
    };

    // Ручной флоу: code=true + manual-redirect (страница покажет код).
    let url = format!(
        "{AUTHORIZE_URL}?code=true&client_id={CLIENT_ID}&response_type=code&redirect_uri={}&scope={}&code_challenge={}&code_challenge_method=S256&state={}",
        enc(MANUAL_REDIRECT),
        enc(SCOPE),
        challenge,
        enc(&state),
    );
    std::process::Command::new("open")
        .arg(&url)
        .spawn()
        .map_err(|e| format!("открыть браузер: {e}"))?;

    let pasted = prompt_code().ok_or("вход отменён")?;
    let pasted = pasted.trim();
    // Страница отдаёт код в формате `code#state`.
    let (code, ret_state) = match pasted.split_once('#') {
        Some((c, s)) => (c.to_string(), s.to_string()),
        None => (pasted.to_string(), state.clone()),
    };
    if ret_state != state {
        return Err("state не совпал (возможна подмена)".into());
    }

    // Обмен — application/x-www-form-urlencoded (JSON тут вызывает проблемы).
    let form = [
        ("grant_type", "authorization_code"),
        ("code", code.as_str()),
        ("redirect_uri", MANUAL_REDIRECT),
        ("client_id", CLIENT_ID),
        ("code_verifier", verifier.as_str()),
        ("state", state.as_str()),
    ];
    let resp = client
        .post(TOKEN_URL)
        .form(&form)
        .send()
        .map_err(|e| format!("обмен кода: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("обмен кода: HTTP {}", resp.status().as_u16()));
    }
    let t: TokenResp = resp.json().map_err(|e| format!("разбор токена: {e}"))?;
    save(&t.access_token, t.refresh_token.as_deref(), t.expires_in)?;
    Ok(())
}

/// Диалог для вставки кода (osascript). None — отмена.
fn prompt_code() -> Option<String> {
    let script = "display dialog \"Авторизуйся в браузере, затем вставь сюда показанный код:\" default answer \"\" with title \"Claude Usage — вход\"";
    let out = std::process::Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout);
    s.split("text returned:").nth(1).map(|x| x.trim().to_string())
}

/// Рефреш нашего токена (Claude Code тут нет, обновляем сами).
pub fn refresh(client: &reqwest::blocking::Client, refresh_token: &str) -> Result<Creds, String> {
    let form = [
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("client_id", CLIENT_ID),
    ];
    let resp = client
        .post(TOKEN_URL)
        .form(&form)
        .send()
        .map_err(|e| format!("рефреш: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("рефреш: HTTP {}", resp.status().as_u16()));
    }
    let t: TokenResp = resp.json().map_err(|e| format!("разбор токена: {e}"))?;
    // Сервер может не вернуть новый refresh_token — оставляем прежний.
    let rt = t.refresh_token.clone().unwrap_or_else(|| refresh_token.to_string());
    save(&t.access_token, Some(&rt), t.expires_in)?;
    Ok(Creds {
        access_token: t.access_token,
        refresh_token: Some(rt),
        expires_at_ms: t.expires_in.map(expires_at),
        source: Source::Ours,
    })
}

fn save(access: &str, refresh: Option<&str>, expires_in: Option<i64>) -> Result<(), String> {
    let blob = serde_json::json!({
        "claudeAiOauth": {
            "accessToken": access,
            "refreshToken": refresh,
            "expiresAt": expires_in.map(expires_at),
        }
    });
    let path = creds_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("mkdir: {e}"))?;
    }
    std::fs::write(&path, blob.to_string()).map_err(|e| format!("запись: {e}"))?;
    Ok(())
}

fn expires_at(seconds: i64) -> i64 {
    chrono::Utc::now().timestamp_millis() + seconds * 1000
}

fn rand_bytes(n: usize) -> Vec<u8> {
    let mut b = vec![0u8; n];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        let _ = f.read_exact(&mut b);
    }
    b
}

fn b64url(data: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(data)
}

/// Кодирование значения query как у URLSearchParams (пробел → `+`, остальное
/// не-unreserved → %XX). Важно совпасть с форматом Claude Code.
fn enc(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for &b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

