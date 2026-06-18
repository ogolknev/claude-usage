//! Рендер кольца-прогрессбара для иконки в строке меню.
//! Дуга заполняется по проценту утилизации сессионного лимита, цвет — от зелёного
//! (свободно) к красному (близко к лимиту). tray-icon сам ужимает до высоты 18pt.

use std::f32::consts::TAU;

const N: i32 = 44; // сторона в пикселях; масштабируется системой до 18pt

pub fn ring_rgba(percent: f64) -> (Vec<u8>, u32, u32) {
    let nf = N as f32;
    let c = nf / 2.0;
    let r_out = nf * 0.42;
    let r_in = nf * 0.27;
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

fn progress_color(percent: f64) -> (u8, u8, u8) {
    if percent >= 80.0 {
        (255, 69, 58) // red
    } else if percent >= 50.0 {
        (255, 159, 10) // orange
    } else {
        (48, 209, 88) // green
    }
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
