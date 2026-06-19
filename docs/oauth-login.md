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

## Доказано: наш запрос структурно корректен

Метаданные рабочего токена Claude Code (из keychain) совпадают с тем, что фронт
кладёт в наш грант:
- `subscriptionType: max`, `rateLimitTier: default_claude_max_5x` (личный Max 5x,
  не корпоратив → claude.ai-флоу правильный);
- `scopes: [user:file_upload, user:inference, user:mcp_servers, user:profile,
  user:sessions:claude_code]` — ровно те же 5 (без `org:create_api_key`), что в
  нашем гранте.

Два упавших гранта (platform-redirect и localhost-redirect) идентичны по телу,
оба `400 invalid_request_error / Invalid request format`.

## Стена: Arkose (FunCaptcha)

В `arkose_session_token` поле `rid` растёт по мере попыток (1 → 71): мы накрутили
антибот-счётчик с устройства, и Arkose стал отдавать токены, которые бэкенд
бракует. Claude Code в том же браузере работает (его вход так не затёрт).

## Вывод

- Код собран правильно (доказано). Упор — **не в код, а в Arkose-капчу**.
- Обходить капчу нельзя/не нужно; это серая зона ToS; долбёжка = риск аккаунта.
- В продукт логин **не идёт**. Без Claude Code — только официальный путь
  (поставить Claude Code, `/login`).

## Важное

- Использование подписочного OAuth-токена вне официальных клиентов Anthropic —
  серая зона ToS. В публичный релиз тащить не стоит.
- Рабочий поддерживаемый путь для пользователя без Claude Code: поставить Claude
  Code (бесплатный, та же подписка) → `/login`.

## Ссылки

- https://github.com/anthropics/claude-code/issues (баги про OAuth подписки)
- https://gist.github.com/shubcodes/3c9c7ff813715aa47018bf22e7cf8cb5 (PKCE-скрипт)
- https://flopsstuff.github.io/coqu/claude-oauth/
