//! Стоимость локального расхода по официальным тарифам Anthropic (API-эквивалент;
//! на подписке ты это не платишь — это «сколько стоило бы по API»).
//!
//! Хранятся только input/output $/1M по моделям. Кэш-цены выводятся по формуле
//! Anthropic от input: write-5мин ×1.25, write-1час ×2, read ×0.1.
//!
//! Актуальность: базовые цены вшиты как фолбэк, но раз в сутки обновляются из
//! LiteLLM (тот же источник, что у ccusage; зеркалит офф-цены Anthropic) в
//! `~/Library/Caches/com.local.claude-usage/prices.json`. См. [`refresh`].

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct Price {
    pub input: f64,  // $/1M
    pub output: f64, // $/1M
}

pub type Prices = HashMap<String, Price>;

// Множители кэша от цены input (официальная схема Anthropic).
const CW_5M: f64 = 1.25;
const CW_1H: f64 = 2.0;
const CR: f64 = 0.1;

// jsdelivr-зеркало того же файла LiteLLM: быстрый CDN (raw.githubusercontent
// для 1.5MB регулярно отваливается по таймауту).
const LITELLM_URL: &str =
    "https://cdn.jsdelivr.net/gh/BerriAI/litellm@main/model_prices_and_context_window.json";

/// Вшитые официальные тарифы (фолбэк, если ещё не обновлялись из сети).
fn defaults() -> Prices {
    let opus45 = Price { input: 5.0, output: 25.0 };
    let opus41 = Price { input: 15.0, output: 75.0 };
    let sonnet = Price { input: 3.0, output: 15.0 };
    let mut m = Prices::new();
    for k in [
        "claude-opus-4-8",
        "claude-opus-4-7",
        "claude-opus-4-6",
        "claude-opus-4-5",
    ] {
        m.insert(k.into(), opus45);
    }
    m.insert("claude-opus-4-1".into(), opus41);
    m.insert("claude-opus-4".into(), opus41);
    m.insert("claude-sonnet-4-6".into(), sonnet);
    m.insert("claude-sonnet-4-5".into(), sonnet);
    m.insert("claude-haiku-4-5".into(), Price { input: 1.0, output: 5.0 });
    m.insert("claude-fable-5".into(), Price { input: 10.0, output: 50.0 });
    m
}

fn cache_path() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default())
        .join("Library/Caches/com.local.claude-usage/prices.json")
}

/// Актуальные цены: обновлённые из сети (если есть) поверх вшитых.
pub fn load() -> Prices {
    let mut p = defaults();
    if let Ok(s) = std::fs::read_to_string(cache_path()) {
        if let Ok(net) = serde_json::from_str::<Prices>(&s) {
            p.extend(net);
        }
    }
    p
}

fn lookup<'a>(prices: &'a Prices, model: &str) -> Option<&'a Price> {
    if let Some(p) = prices.get(model) {
        return Some(p);
    }
    // подстрочный матч на случай префиксов вроде "anthropic/claude-…"
    prices
        .iter()
        .find(|(k, _)| model.contains(k.as_str()))
        .map(|(_, p)| p)
}

/// Стоимость одного сообщения в USD. `c5m`/`c1h` — запись кэша 5мин/1час.
pub fn cost(prices: &Prices, model: &str, inp: u64, out: u64, c5m: u64, c1h: u64, cr: u64) -> f64 {
    let Some(p) = lookup(prices, model) else {
        return 0.0;
    };
    (inp as f64 * p.input
        + out as f64 * p.output
        + c5m as f64 * p.input * CW_5M
        + c1h as f64 * p.input * CW_1H
        + cr as f64 * p.input * CR)
        / 1_000_000.0
}

/// Обновляет `prices.json` из LiteLLM (вызывается воркером раз в сутки).
/// Тихо ничего не делает при сетевой ошибке — остаются прежние/вшитые цены.
/// Свой клиент с увеличенным таймаутом: файл ~1.5MB.
pub fn refresh() {
    let Ok(client) = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .user_agent("claude-usage-menubar/0.1")
        .build()
    else {
        return;
    };
    let Ok(resp) = client.get(LITELLM_URL).send() else {
        return;
    };
    if !resp.status().is_success() {
        return;
    }
    let Ok(json) = resp.json::<serde_json::Value>() else {
        return;
    };
    let Some(obj) = json.as_object() else {
        return;
    };
    let mut out = Prices::new();
    for (k, v) in obj {
        let name = k.strip_prefix("anthropic/").unwrap_or(k);
        if !name.starts_with("claude-") {
            continue;
        }
        let inp = v.get("input_cost_per_token").and_then(|x| x.as_f64());
        let outp = v.get("output_cost_per_token").and_then(|x| x.as_f64());
        if let (Some(i), Some(o)) = (inp, outp) {
            out.insert(
                name.to_string(),
                Price {
                    input: i * 1_000_000.0,
                    output: o * 1_000_000.0,
                },
            );
        }
    }
    if out.is_empty() {
        return;
    }
    let path = cache_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(s) = serde_json::to_string(&out) {
        let _ = std::fs::write(&path, s);
    }
}
