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

## Блокер (РАЗГАДАН по HAR)

Сам грант делает не Claude Code, а **браузер** (React-фронт claude.ai). По клику
Authorize фронт шлёт:

```
POST https://claude.ai/v1/oauth/{organization_uuid}/authorize   (Content-Type: application/json)
{
  "response_type":"code", "client_id":"9d1c250a-...",
  "organization_uuid":"...", "redirect_uri":"...",
  "scope":"user:profile user:inference user:sessions:claude_code user:mcp_servers user:file_upload",  // org:create_api_key фронт ВЫКИДЫВАЕТ
  "state":"...", "code_challenge":"...", "code_challenge_method":"S256",
  "arkose_session_token":"...|pk=EEA5F558-D6AC-4C03-B678-AABF639EE69A|..."   // Arkose Labs (FunCaptcha)
}
```

Ответ при наших попытках: `400 {"type":"invalid_request_error","message":"Invalid request format"}`.

**Грант огорожен Arkose Labs (FunCaptcha) — антибот-защитой.** `arkose_session_token`
генерит JS claude.ai в браузере. Claude Code и наше приложение делают одно и то
же: открывают браузер, пользователь авторизуется (браузер сам проходит Arkose),
приложение получает код. Копировать в коде Claude Code нечего — грант браузерный.

Тело запроса было полным и валидным, поэтому «Invalid request format» здесь —
это, скорее всего, **антибот зафлагал сессию/IP** после ~20 быстрых попыток.

## Вывод

- Флоу в принципе делегируется браузеру и **мог бы сработать** на чистой
  (не зафлаганной) сессии — приложение получает код после успешного гранта.
- Но это упирается в их антибот (Arkose) + серую зону ToS. Долбить = риск бана.
  Обходить капчу нельзя и не нужно.
- Практический вывод: в продукт логин не тащим. Кому нужно без Claude Code —
  путь только официальный (Claude Code `/login`).

## Важное

- Использование подписочного OAuth-токена вне официальных клиентов Anthropic —
  серая зона ToS. В публичный релиз тащить не стоит.
- Рабочий поддерживаемый путь для пользователя без Claude Code: поставить Claude
  Code (бесплатный, та же подписка) → `/login`.

## Ссылки

- https://github.com/anthropics/claude-code/issues (баги про OAuth подписки)
- https://gist.github.com/shubcodes/3c9c7ff813715aa47018bf22e7cf8cb5 (PKCE-скрипт)
- https://flopsstuff.github.io/coqu/claude-oauth/
