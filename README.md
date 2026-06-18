# Claude Usage (menu bar, macOS)

Лёгкий нативный индикатor в строке меню: показывает реальные лимиты Claude
(сессионный 5ч и недельный) и время сброса — то же, что `/usage` в Claude Code,
плюс локальный расход токенов из логов как фолбэк.

Панель: `5h 11% · 7d 9%` (при высокой утилизации — `⚠`).
Меню по клику: сессия / неделя / модельный лимит с временем сброса, локальный
расход за сегодня и неделю, «Обновить», «Открыть claude.ai/usage», «Выход».

## Как это работает

- **Лимиты** — `GET https://api.anthropic.com/api/oauth/usage` с OAuth-токеном
  Claude Code. Токен читается из macOS Keychain (item `Claude Code-credentials`),
  заголовок `anthropic-beta: oauth-2025-04-20`. Парсится массив `limits[]`
  (`session` / `weekly_all` / `weekly_scoped`). См. `src/limits.rs`.
- **Локальный расход** — суммирование `message.usage` из `~/.claude/projects/*/*.jsonl`
  по окнам 5ч / сегодня / 7 дней (фильтр по mtime). Сетью не пользуется. См. `src/local.rs`.
- Опрос лимитов раз в 60с, локальный расход — раз в 5 мин. При сетевой ошибке
  панель деградирует к локальным токенам и не падает.

v1 **не рефрешит** OAuth-токен сам, чтобы не сломать креды Claude Code — если
токен истёк, ждёт, пока Claude Code обновит его при следующем запуске.

## Сборка и запуск

```sh
bash bundle/build-app.sh          # release + упаковка в «Claude Usage.app» + ad-hoc подпись
open "Claude Usage.app"
```

При **первом запуске** macOS спросит доступ к Keychain-итему `Claude Code-credentials`
— нажми «Always Allow». (Из-за ad-hoc подписи запрос повторится после каждой
пересборки; для постоянного доступа подпиши self-signed identity вместо `-` в
`bundle/build-app.sh`.)

Диагностика без GUI:

```sh
./target/release/claude-usage --probe
```

## Автозапуск (по желанию)

System Settings → General → Login Items → добавить `Claude Usage.app`.

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
