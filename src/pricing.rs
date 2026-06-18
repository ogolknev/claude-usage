//! Оценка стоимости локального расхода.
//!
//! ВНИМАНИЕ: цены НЕ зашиты. Актуальные тарифы Anthropic на модели Claude Code
//! (opus-4-8/4-7, sonnet-4-6, haiku-4-5, fable-5) меняются и не входят в публичный
//! слепок, поэтому по умолчанию таблица пустая и стоимость = 0 (в UI скрывается).
//!
//! Чтобы включить оценку $, впиши цены в USD за 1M токенов в таблицу ниже.
//! Поля: (input, output, cache_write_5m, cache_read). Подстрока матчится по имени модели.

pub struct Price {
    pub input: f64,
    pub output: f64,
    pub cache_write: f64,
    pub cache_read: f64,
}

/// Заполни цены здесь (USD за 1M токенов). Пустая таблица => стоимость не считается.
fn table() -> &'static [(&'static str, Price)] {
    &[
        // Пример (раскомментируй и впиши реальные тарифы):
        // ("opus",   Price { input: 0.0, output: 0.0, cache_write: 0.0, cache_read: 0.0 }),
        // ("sonnet", Price { input: 0.0, output: 0.0, cache_write: 0.0, cache_read: 0.0 }),
        // ("haiku",  Price { input: 0.0, output: 0.0, cache_write: 0.0, cache_read: 0.0 }),
    ]
}

pub fn cost(model: &str, inp: u64, out: u64, cache_write: u64, cache_read: u64) -> f64 {
    let m = model.to_lowercase();
    let Some((_, p)) = table().iter().find(|(name, _)| m.contains(name)) else {
        return 0.0;
    };
    (inp as f64 * p.input
        + out as f64 * p.output
        + cache_write as f64 * p.cache_write
        + cache_read as f64 * p.cache_read)
        / 1_000_000.0
}

/// Есть ли вообще заполненные цены — UI по этому решает, показывать ли «$».
pub fn enabled() -> bool {
    !table().is_empty()
}
