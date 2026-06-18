# Claude Usage (menu bar, macOS)

Лёгкий нативный индикатor в строке меню: показывает реальные лимиты Claude
(сессионный 5ч и недельный) и время сброса — то же, что `/usage` в Claude Code,
плюс локальный расход токенов из логов как фолбэк.

Панель: кольцо-прогрессбар по сессионному (5ч) лимиту (зелёный→оранжевый→красный)
+ процент без подписи, напр. `◔ 13%`.
Меню по клику: сессия / неделя / модельный лимит с временем сброса, локальный
расход за сегодня и неделю, «Обновить», «Открыть claude.ai/usage», «Выход».

## Установка

Через Homebrew:

```sh
brew install --cask ogolknev/tap/claude-usage
```

Или вручную: скачай `.dmg` из [релизов](https://github.com/ogolknev/claude-usage/releases),
открой, перетащи `Claude Usage.app` в Applications.

При первом запуске один раз разреши доступ к Keychain-итему `Claude Code-credentials`
(«Всегда разрешать» / Touch ID). Автозапуск при входе: `bash bundle/autostart.sh on`.

> Аппка подписана ad-hoc (без нотаризации Apple). Через cask карантин снимается сам;
> при ручной установке из `.dmg`, если Gatekeeper ругнётся:
> `xattr -dr com.apple.quarantine "/Applications/Claude Usage.app"`.

Требования: Apple Silicon, установленный и авторизованный Claude Code.

## Как это работает

- **Лимиты** — `GET https://api.anthropic.com/api/oauth/usage` с OAuth-токеном
  Claude Code. Токен читается из macOS Keychain (item `Claude Code-credentials`),
  заголовок `anthropic-beta: oauth-2025-04-20`. Парсится массив `limits[]`
  (`session` / `weekly_all` / `weekly_scoped`). См. `src/limits.rs`.
- **Локальный расход** — суммирование `message.usage` из `~/.claude/projects/*/*.jsonl`
  по окнам 5ч / сегодня / 7 дней (фильтр по mtime). Сетью не пользуется. См. `src/local.rs`.
- Опрос лимитов раз в 180с с бэкоффом при 429 (уважаем `Retry-After`). Последние
  удачные лимиты кэшируются на диск (`~/Library/Caches/com.local.claude-usage`),
  поэтому кольцо показывается сразу при старте и переживает 429/перезапуск. При
  полной недоступности — деградация к локальным токенам.

v1 **не рефрешит** OAuth-токен сам, чтобы не сломать креды Claude Code — если
токен истёк, ждёт, пока Claude Code обновит его при следующем запуске.

## Сборка из исходников

```sh
bash bundle/build-app.sh          # release + упаковка в «Claude Usage.app» + ad-hoc подпись
bash bundle/build-dmg.sh          # (опционально) DMG-инсталлятор
open "Claude Usage.app"
```

Токен читается через системный `/usr/bin/security`. Один раз macOS спросит доступ
к Keychain-итему `Claude Code-credentials` — нажми «Всегда разрешать» (на Mac с
Touch ID подтверждение можно приложить пальцем). Дальше промптов не будет — даже
после пересборки приложения, потому что доступ выдан стабильному системному
бинарю `security`, а не нашему ad-hoc-подписанному приложению.

Диагностика без GUI:

```sh
./target/release/claude-usage --probe
```

## Автозапуск

```sh
bash bundle/autostart.sh on    # поставить LaunchAgent и запустить сейчас
bash bundle/autostart.sh off   # убрать
```

Ставит `~/Library/LaunchAgents/com.local.claude-usage.plist` (RunAtLoad) — приложение
стартует при входе в систему. Если выйти через меню «Выход», обратно до следующего
входа не поднимется. Альтернатива вручную: System Settings → General → Login Items.

## Оценка стоимости ($)

По умолчанию выключена: точных тарифов на текущие модели нет, фейковые цифры не
зашиваются. Чтобы включить — впиши цены (USD за 1M токенов) в таблицу в
`src/pricing.rs`; тогда в меню появится оценка `~$…`.

## Структура

```
src/
  main.rs      событийный цикл tao + tray-icon, опрос, меню, --probe
  limits.rs    /api/oauth/usage + парсинг (единственная точка риска)
  keychain.rs  чтение Claude Code-credentials из Keychain
  local.rs     расход из JSONL-логов
  pricing.rs   редактируемая таблица цен
  tray.rs      форматирование строки и пунктов меню
  model.rs     общие структуры
bundle/        Info.plist (LSUIElement) + build-app.sh
```

## Замечания

- `/api/oauth/usage` — внутренний недокументированный эндпоинт Claude Code; может
  меняться между версиями. Изолирован в `src/limits.rs`; при сбое приложение
  продолжает работать на локальном расходе.
- Токен живёт только в памяти процесса и никуда не отправляется, кроме `api.anthropic.com`.
