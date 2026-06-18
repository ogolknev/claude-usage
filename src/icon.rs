//! Рендер кольца-прогрессбара для иконки в строке меню.
//! Дуга заполняется по проценту утилизации сессионного лимита, цвет — от зелёного
//! (свободно) к красному (близко к лимиту). tray-icon сам ужимает до высоты 18pt.

use std::f32::consts::TAU;

const N: i32 = 44; // сторона в пикселях; масштабируется системой до 18pt

pub fn ring_rgba(percent: f64) -> (Vec<u8>, u32, u32) {
    let nf = N as f32;
    let c = nf / 2.0;
    // Кольцо почти во всю высоту слота (~18pt у tray-icon) и потолще.
    let r_out = nf * 0.47;
    let r_in = nf * 0.31;
    let frac = (percent.clamp(0.0, 100.0) as f32) / 100.0;

    let (pr, pg, pb) = progress_color(percent);
    let (tr, tg, tb, ta) = (130u8, 130, 130, 70); // дорожка под прогрессом

    let mut buf = vec![0u8; (N * N * 4) as usize];
    for y in 0..N {
        for x in 0..N {
            let dx = x as f32 - c + 0.5;
            let dy = y as f32 - c + 0.5;
            let r = (dx * dx + dy * dy).sqrt();
            // Кольцевая полоса с мягкими краями (~1px антиалиасинг по радиусу).
            let cov = smoothstep(r_in - 0.8, r_in + 0.8, r)
                * (1.0 - smoothstep(r_out - 0.8, r_out + 0.8, r));
            if cov <= 0.003 {
                continue;
            }
            // Угол от 12 часов по часовой стрелке, [0,1).
            let mut a = dx.atan2(-dy);
            if a < 0.0 {
                a += TAU;
            }
            let af = a / TAU;

            let (cr, cg, cb, base_a) = if af <= frac {
                (pr, pg, pb, 240u8)
            } else {
                (tr, tg, tb, ta)
            };
            let alpha = (base_a as f32 * cov).round() as u8;
            let i = ((y * N + x) * 4) as usize;
            buf[i] = cr;
            buf[i + 1] = cg;
            buf[i + 2] = cb;
            buf[i + 3] = alpha;
        }
    }
    (buf, N as u32, N as u32)
}

/// Плавный градиент по заполнению: зелёный (0%) → жёлтый → красный (100%).
fn progress_color(percent: f64) -> (u8, u8, u8) {
    let t = (percent.clamp(0.0, 100.0) / 100.0) as f32;
    let hue = 130.0 * (1.0 - t); // 130° зелёный → 0° красный
    hsv_to_rgb(hue, 0.85, 0.95)
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let c = v * s;
    let hp = h / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    (
        ((r + m) * 255.0).round() as u8,
        ((g + m) * 255.0).round() as u8,
        ((b + m) * 255.0).round() as u8,
    )
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
