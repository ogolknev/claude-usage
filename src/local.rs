use crate::model::LocalUsage;
use crate::pricing;
use chrono::{DateTime, Duration, Local, TimeZone, Utc};
use std::collections::HashSet;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Считает расход токенов за окна 5ч / сегодня / 7д из JSONL-логов Claude Code.
/// Сетью не пользуется. Файлы фильтруются по mtime.
///
/// Важно: «вх+вых» (input+output) — это осмысленный расход. cache_read (чтение
/// закэшированного контекста) считается отдельно: его на порядки больше, и в
/// «расход» он не входит. Записи дедуплицируются по `message.id`, иначе одни и
/// те же сообщения из нескольких файлов (resume/форк сессий) считаются повторно.
pub fn compute(home: &Path) -> LocalUsage {
    let projects = home.join(".claude/projects");
    let now = Utc::now();
    let w5h = now - Duration::hours(5);
    let w7d = now - Duration::days(7);
    let today_start = start_of_today_utc();

    let cutoff = SystemTime::now() - std::time::Duration::from_secs(8 * 24 * 3600);
    let mut files = Vec::new();
    collect_jsonl(&projects, cutoff, &mut files);

    let mut u = LocalUsage::default();
    let mut seen: HashSet<String> = HashSet::new();
    for f in files {
        accumulate_file(&f, w5h, w7d, today_start, &mut u, &mut seen);
    }
    u
}

fn accumulate_file(
    path: &Path,
    w5h: DateTime<Utc>,
    w7d: DateTime<Utc>,
    today_start: DateTime<Utc>,
    u: &mut LocalUsage,
    seen: &mut HashSet<String>,
) {
    let Ok(file) = fs::File::open(path) else {
        return;
    };
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        if !line.contains("\"usage\"") {
            continue;
        }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        let Some(ts) = v
            .get("timestamp")
            .and_then(|t| t.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|d| d.with_timezone(&Utc))
        else {
            continue;
        };
        if ts < w7d {
            continue;
        }
        let message = v.get("message");
        let Some(usage) = message.and_then(|m| m.get("usage")) else {
            continue;
        };

        // Дедуп: одно и то же сообщение встречается в нескольких файлах.
        if let Some(id) = message.and_then(|m| m.get("id")).and_then(|x| x.as_str()) {
            if !seen.insert(id.to_string()) {
                continue;
            }
        }

        let inp = field(usage, "input_tokens");
        let out = field(usage, "output_tokens");
        let cw = field(usage, "cache_creation_input_tokens");
        let cr = field(usage, "cache_read_input_tokens");
        let io = inp + out;
        let cache = cw + cr;
        let model = message
            .and_then(|m| m.get("model"))
            .and_then(|x| x.as_str())
            .or_else(|| v.get("model").and_then(|x| x.as_str()))
            .unwrap_or("");
        let cost = pricing::cost(model, inp, out, cw, cr);

        u.week_io += io;
        u.week_cache += cache;
        u.week_cost += cost;
        if ts >= today_start {
            u.today_io += io;
            u.today_cache += cache;
            u.today_cost += cost;
        }
        if ts >= w5h {
            u.window5h_io += io;
        }
    }
}

fn field(usage: &serde_json::Value, key: &str) -> u64 {
    usage.get(key).and_then(|x| x.as_u64()).unwrap_or(0)
}

fn start_of_today_utc() -> DateTime<Utc> {
    let local = Local::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .expect("midnight");
    Local
        .from_local_datetime(&local)
        .single()
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or_else(Utc::now)
}

fn collect_jsonl(dir: &Path, cutoff: SystemTime, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect_jsonl(&p, cutoff, out);
        } else if p.extension().and_then(|x| x.to_str()) == Some("jsonl") {
            if let Ok(m) = e.metadata().and_then(|md| md.modified()) {
                if m >= cutoff {
                    out.push(p);
                }
            }
        }
    }
}
