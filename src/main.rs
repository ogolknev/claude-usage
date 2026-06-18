mod icon;
mod keychain;
mod limits;
mod local;
mod model;
mod pricing;
mod tray;

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use tao::event::Event;
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::TrayIconBuilder;

use model::{LocalUsage, UsageState};

const USAGE_PAGE: &str = "https://claude.ai/settings/usage";
const POLL_SECS: u64 = 60;
/// Локальный расход тяжелее опроса лимитов, поэтому считаем реже (раз в 5 циклов).
const LOCAL_EVERY: u64 = 5;

enum UserEvent {
    State(Box<UsageState>),
    Menu(MenuEvent),
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default())
}

fn main() {
    // Диагностика без GUI: один прогон пайплайна и выход.
    if std::env::args().any(|a| a == "--probe") {
        probe();
        return;
    }

    let mut event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    // Agent-приложение без иконки в доке (дублирует LSUIElement в Info.plist).
    event_loop.set_activation_policy(ActivationPolicy::Accessory);

    // Информационные строки (disabled) + действия.
    let i_session = MenuItem::new("Сессия (5ч): загрузка…", false, None);
    let i_weekly = MenuItem::new("Неделя: загрузка…", false, None);
    let i_scoped = MenuItem::new("Модельные лимиты: …", false, None);
    let i_local = MenuItem::new("Расход: …", false, None);
    let i_updated = MenuItem::new("Обновлено: —", false, None);
    let i_refresh = MenuItem::new("Обновить", true, None);
    let i_open = MenuItem::new("Открыть claude.ai/usage", true, None);
    let i_quit = MenuItem::new("Выход", true, None);

    let menu = Menu::new();
    let _ = menu.append(&i_session);
    let _ = menu.append(&i_weekly);
    let _ = menu.append(&i_scoped);
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&i_local);
    let _ = menu.append(&i_updated);
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&i_refresh);
    let _ = menu.append(&i_open);
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&i_quit);

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_title("Claude …")
        .with_tooltip("Claude Usage")
        .build()
        .expect("не удалось создать значок в панели");

    let id_refresh = i_refresh.id().clone();
    let id_open = i_open.id().clone();
    let id_quit = i_quit.id().clone();

    // Клики по меню приходят в глобальный канал — пробрасываем их в цикл tao.
    let proxy_menu = event_loop.create_proxy();
    MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
        let _ = proxy_menu.send_event(UserEvent::Menu(e));
    }));

    let (refresh_tx, refresh_rx) = mpsc::channel::<()>();
    let proxy_state = event_loop.create_proxy();
    thread::spawn(move || worker(proxy_state, refresh_rx));

    event_loop.run(move |event, _target, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::UserEvent(UserEvent::State(state)) => {
                match tray::session_percent(&state) {
                    Some(p) => {
                        let (rgba, w, h) = icon::ring_rgba(p);
                        if let Ok(ic) = tray_icon::Icon::from_rgba(rgba, w, h) {
                            let _ = tray.set_icon(Some(ic));
                        }
                    }
                    None => {
                        let _ = tray.set_icon(None);
                    }
                }
                let _ = tray.set_title(Some(tray::title_for(&state)));
                let _ = tray.set_tooltip(Some(tray::tooltip_for(&state)));
                tray::apply_menu(
                    &state,
                    &tray::MenuHandles {
                        session: &i_session,
                        weekly: &i_weekly,
                        scoped: &i_scoped,
                        local: &i_local,
                        updated: &i_updated,
                    },
                );
            }
            Event::UserEvent(UserEvent::Menu(e)) => {
                if e.id == id_quit {
                    *control_flow = ControlFlow::Exit;
                } else if e.id == id_refresh {
                    let _ = refresh_tx.send(());
                } else if e.id == id_open {
                    let _ = std::process::Command::new("open").arg(USAGE_PAGE).spawn();
                }
            }
            _ => {}
        }
    });
}

fn probe() {
    let client = limits::client();
    match limits::fetch(&client) {
        Ok(l) => {
            println!("limits OK:");
            for e in &l.entries {
                println!("  {:<16} {:>3}%  reset={:?}", e.kind, e.percent as i64, e.resets_at);
            }
        }
        Err(e) => println!("limits ERR: {e}"),
    }
    let u = local::compute(&home());
    println!(
        "local: today={} 5h={} week={} (tok)",
        u.today_tokens, u.window5h_tokens, u.week_tokens
    );
}

fn worker(proxy: EventLoopProxy<UserEvent>, refresh_rx: Receiver<()>) {
    let h = home();
    let client = limits::client();
    let mut tick: u64 = 0;
    let mut last_local = LocalUsage::default();
    let mut last_ok: Option<chrono::DateTime<chrono::Local>> = None;

    loop {
        let limits = limits::fetch(&client);
        if limits.is_ok() {
            last_ok = Some(chrono::Local::now());
        }
        if tick % LOCAL_EVERY == 0 {
            last_local = local::compute(&h);
        }
        let state = UsageState {
            limits: limits.as_ref().ok().cloned(),
            limits_err: limits.as_ref().err().cloned(),
            local: last_local.clone(),
            fetched_at: chrono::Local::now(),
            last_ok,
        };
        if proxy.send_event(UserEvent::State(Box::new(state))).is_err() {
            break; // цикл событий закрыт — выходим
        }
        tick += 1;

        match refresh_rx.recv_timeout(Duration::from_secs(POLL_SECS)) {
            Ok(()) => tick = 0, // ручное «Обновить» — пересчитать и локальный расход
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
}
