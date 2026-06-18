//! Встроенный OAuth-логин (PKCE) — чтобы работать без Claude Code/Desktop.
//! Тот же флоу, что у Claude Code: открываем браузер на authorize, локальный
//! сервер ловит redirect с кодом, меняем код на токен. Свой токен храним в
//! `~/Library/Application Support/claude-usage/credentials.json` (формат как у
//! Claude Code: `{claudeAiOauth:{accessToken,refreshToken,expiresAt}}`) и сами
//! его рефрешим.

use crate::keychain::{Creds, Source};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::Deserialize;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
const AUTHORIZE_URL: &str = "https://platform.claude.com/oauth/authorize";
const TOKEN_URL: &str = "https://platform.claude.com/v1/oauth/token";
const SCOPE: &str = "org:create_api_key user:profile user:inference";

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

/// Полный интерактивный логин: браузер + локальный колбэк + обмен кода.
pub fn login(client: &reqwest::blocking::Client) -> Result<(), String> {
    let verifier = b64url(&rand_bytes(32));
    let state = b64url(&rand_bytes(16));
    let challenge = {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(verifier.as_bytes());
        b64url(&h.finalize())
    };

    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| format!("listen: {e}"))?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let redirect = format!("http://localhost:{port}/callback");

    let url = format!(
        "{AUTHORIZE_URL}?client_id={CLIENT_ID}&response_type=code&redirect_uri={}&scope={}&code_challenge={}&code_challenge_method=S256&state={}",
        enc(&redirect),
        enc(SCOPE),
        challenge,
        enc(&state),
    );
    std::process::Command::new("open")
        .arg(&url)
        .spawn()
        .map_err(|e| format!("открыть браузер: {e}"))?;

    let (code, got_state) = wait_callback(&listener)?;
    if got_state != state {
        return Err("state не совпал (возможна подмена)".into());
    }

    let body = serde_json::json!({
        "grant_type": "authorization_code",
        "code": code,
        "redirect_uri": redirect,
        "client_id": CLIENT_ID,
        "code_verifier": verifier,
        "state": state,
    });
    let resp = client
        .post(TOKEN_URL)
        .json(&body)
        .send()
        .map_err(|e| format!("обмен кода: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("обмен кода: HTTP {}", resp.status().as_u16()));
    }
    let t: TokenResp = resp.json().map_err(|e| format!("разбор токена: {e}"))?;
    save(&t.access_token, t.refresh_token.as_deref(), t.expires_in)?;
    Ok(())
}

/// Рефреш нашего токена (Claude Code тут нет, обновляем сами).
pub fn refresh(client: &reqwest::blocking::Client, refresh_token: &str) -> Result<Creds, String> {
    let body = serde_json::json!({
        "grant_type": "refresh_token",
        "refresh_token": refresh_token,
        "client_id": CLIENT_ID,
    });
    let resp = client
        .post(TOKEN_URL)
        .json(&body)
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

/// Ждём GET /callback?code=…&state=… на локальном сервере (до 3 минут).
fn wait_callback(listener: &TcpListener) -> Result<(String, String), String> {
    listener.set_nonblocking(true).ok();
    let deadline = Instant::now() + Duration::from_secs(180);
    loop {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let mut buf = [0u8; 4096];
                let n = stream.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]);
                let path = req
                    .lines()
                    .next()
                    .and_then(|l| l.split_whitespace().nth(1))
                    .unwrap_or("");
                let html = "<html><body style='font-family:sans-serif'>Готово — можно закрыть это окно и вернуться в Claude Usage.</body></html>";
                let _ = stream.write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        html.len(),
                        html
                    )
                    .as_bytes(),
                );
                let (mut code, mut state) = (None, None);
                if let Some(q) = path.split('?').nth(1) {
                    for kv in q.split('&') {
                        let mut it = kv.splitn(2, '=');
                        match (it.next().unwrap_or(""), it.next().unwrap_or("")) {
                            ("code", v) => code = Some(dec(v)),
                            ("state", v) => state = Some(dec(v)),
                            _ => {}
                        }
                    }
                }
                return match (code, state) {
                    (Some(c), Some(s)) => Ok((c, s)),
                    _ => Err("в колбэке нет code/state (вход отменён?)".into()),
                };
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() > deadline {
                    return Err("истекло время ожидания входа".into());
                }
                std::thread::sleep(Duration::from_millis(200));
            }
            Err(e) => return Err(format!("accept: {e}")),
        }
    }
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

/// Перкодирование значения query: всё, кроме unreserved, в %XX.
fn enc(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for &b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn dec(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
                if let Ok(v) = u8::from_str_radix(hex, 16) {
                    out.push(v);
                    i += 3;
                    continue;
                }
                out.push(b'%');
                i += 1;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}
