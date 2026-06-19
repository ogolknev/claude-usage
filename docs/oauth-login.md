# Встроенный OAuth-логин — статус (WIP, не работает)

Цель: логиниться в подписку Claude прямо из приложения, без Claude Code/Desktop,
чтобы получить свой OAuth-токен для `/api/oauth/usage`.

Реализовано в `src/auth.rs` (ручной флоу + PKCE), пункт меню «Войти в Claude…»,
флаг `--login` для теста. **В релизы не входит.**

## Что точно выяснено (проверено вживую)

- Подписочный («обычный Клод») authorize: `https://claude.ai/oauth/authorize`
  (Console/API — другой: `platform.claude.com/oauth/authorize`; client_id-URL
  метадокумента claude.ai отвергает: «client_id should be a valid UUID»).
- `client_id` = `9d1c250a-e61b-44d9-88ed-5944d1962f5e` (UUID).
- Token endpoint: `https://platform.claude.com/v1/oauth/token`, обмен —
  `application/x-www-form-urlencoded` (НЕ JSON).
- PKCE S256, `state`, redirect либо `http://localhost:PORT/callback` (localhost-флоу,
  без `code=true`), либо `https://platform.claude.com/oauth/code/callback`
  (manual-флоу, с `code=true`, страница показывает `code#state`).
- Scope без `org:create_api_key` (это org/console-scope). Полный набор Claude Code:
  `org:create_api_key user:profile user:inference user:sessions:claude_code
  user:mcp_servers user:file_upload`; для подписки `org:create_api_key` убирают.
- Консент рисуется корректно («Claude Code would like to connect to your Claude
  chat account», нужные скоупы).

## Блокер

На шаге **гранта** (после нажатия Authorize) claude.ai стабильно отвечает
`Invalid request format` (видно в консоли: `[REACT_QUERY_CLIENT] QueryClient error:
Error: Invalid request format`). Так падают ВСЕ комбинации, включая ту, что
используют рабочие сторонние реализации (manual + `code=true` + scope без
`org:create_api_key` + form-обмен). Наш authorize-URL побайтово совпадает с
URL, который генерирует сам Claude Code (сверено).

## Гипотезы

1. claude.ai недавно поменял подписочный OAuth — сторонние реализации сломались.
2. Soft-block/rate-limit после множества попыток (нельзя долбить — риск бана).

## Единственный надёжный следующий шаг

Перехватить **реальный успешный грант-запрос самого Claude Code** (`/login`):
DevTools → Network на POST, который шлёт страница консента при Authorize, ИЛИ
mitmproxy на трафике Claude Code. Снять точный URL, payload и заголовки гранта и
сдиффить с нашим. Без этого — гадание.

## Важное

- Использование подписочного OAuth-токена вне официальных клиентов Anthropic —
  серая зона ToS. В публичный релиз тащить не стоит.
- Рабочий поддерживаемый путь для пользователя без Claude Code: поставить Claude
  Code (бесплатный, та же подписка) → `/login`.

## Ссылки

- https://github.com/anthropics/claude-code/issues (баги про OAuth подписки)
- https://gist.github.com/shubcodes/3c9c7ff813715aa47018bf22e7cf8cb5 (PKCE-скрипт)
- https://flopsstuff.github.io/coqu/claude-oauth/
