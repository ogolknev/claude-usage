# Встроенный OAuth-логин — РАБОТАЕТ

Цель: логиниться в подписку Claude прямо из приложения, без Claude Code/Desktop,
чтобы получить свой OAuth-токен для `/api/oauth/usage`.

Реализовано в `src/auth.rs` (localhost-флоу + PKCE), пункт меню «Войти в Claude…»,
флаг `--login` для теста, `--check` — диагностика источника токена.

## Флоу (как у Claude Code)

1. Открываем браузер на `https://claude.com/cai/oauth/authorize?code=true&…`
   (307 → `claude.ai/oauth/authorize`); client_id `9d1c250a-…`, scope U68
   (`org:create_api_key` + 5 `user:*`), PKCE S256, `state`, redirect
   `http://localhost:PORT/callback`.
2. Пользователь жмёт Authorize. **Грант делает сам фронт claude.ai** (POST
   `claude.ai/v1/oauth/{org}/authorize`, он же проходит Arkose/FunCaptcha
   прозрачно). Копировать в коде Claude Code тут нечего — этот POST браузерный,
   CC его не делает (только `/v1/oauth/hello` и `/v1/oauth/token`).
3. Браузер редиректит на `localhost:PORT/callback?code=…&state=…`; локальный
   сервер ловит код.
4. Обмен кода на токен: POST `platform.claude.com/v1/oauth/token`,
   `application/x-www-form-urlencoded`. Токен кладём в Keychain.

## Что было не так (6 дней тупика) — два бага

Симптом: POST `.../authorize` отдавал `400 invalid_request_error /
Invalid request format`. Разгадка пришла из побайтового диффа нашего упавшего
тела против **успешного** гранта Claude Code (оба с claude.ai-вкладки):

1. **`state` 16 байт.** Все 9 полей совпадали с CC, кроме `state`: у нас 16 байт
   (22 симв.), у CC — 32 байта (43 симв.). Бэкенд бракует короткий `state` как
   `Invalid request format`. Фикс: `rand_bytes(16)` → `rand_bytes(32)`.
2. **gzip token-ответа.** После фикса (1) грант стал проходить, но обмен кода
   падал с «error decoding response body»: `token`-эндпоинт отдаёт
   `application/json` **в gzip**, а reqwest без фичи распаковки возвращал сырые
   байты. Фикс: features `gzip,brotli,deflate,zstd`.

Сигналы, что дело не в антиботе (вопреки прежнему выводу): ответ нёс `request_id`
и стандартный конверт API → это валидация схемы, а не WAF/капча; и фейл был
**постоянным** (`rid` шёл 1 → 71 → 38 непоследовательно, одинаковый 400 уже на
первой попытке) — никакого «накрутили счётчик».

## Хранение токена

`src/keychain.rs` читает токен по приоритету:
1. Keychain Claude Code (`Claude Code-credentials`, через `/usr/bin/security`);
2. файл Claude Code `~/.claude/.credentials.json`;
3. **наш Keychain** (`claude-usage-credentials`, Security framework `SecItem*`);
4. наш старый плейнтекст-файл (миграция со старых версий, удаляется при `save()`).

Наш токен пишем/читаем через крейт `security-framework` (нативный API: без
argv-засветки, без лимита 128). `refresh()` (`grant_type=refresh_token`) делаем
сами и тоже сохраняем в Keychain. Наш токен — fallback: если есть Claude Code,
берётся его токен (CC сам его рефрешит).

## Подпись (чтобы keychain-ACL пережил апдейты)

Наш токен в Keychain имеет app-bound ACL, привязанный к designated requirement
подписи. У ad-hoc-подписи (`codesign --sign -`) DR завязан на cdhash и меняется
каждую сборку → после апдейта приложение теряет доступ (промпт/перелогин).
Поэтому `build-app.sh` подписывает **стабильным самоподписанным сертификатом**
`Claude Usage Signing` (DR = `identifier + cert leaf`, неизменный).

Создать сертификат один раз (Keychain Access → Ассистент сертификатов → Создать
→ тип «Подпись кода»), либо через CLI (LibreSSL `/usr/bin/openssl` — даёт
Apple-совместимый p12):

```sh
/usr/bin/openssl req -x509 -newkey rsa:2048 -keyout k.pem -out c.pem -days 3650 \
  -nodes -subj "/CN=Claude Usage Signing" \
  -addext "extendedKeyUsage=critical,codeSigning" -addext "keyUsage=critical,digitalSignature"
/usr/bin/openssl pkcs12 -export -inkey k.pem -in c.pem -out c.p12 -passout pass:PW -name "Claude Usage Signing"
security import c.p12 -k ~/Library/Keychains/login.keychain-db -P PW -T /usr/bin/codesign -A
rm -f k.pem c.pem c.p12
```

`CSSMERR_TP_NOT_TRUSTED` в `find-identity` не мешает — codesign подписывает, а
ACL сверяет DR. Сертификат **нельзя терять**: новый серт = новый DR = разовый
перелогин у пользователей. Бэкап — экспорт `.p12` из Keychain Access.

## Важное (ToS)

Использование подписочного OAuth-токена вне официальных клиентов Anthropic —
серая зона ToS. Поддерживаемый путь для пользователя без Claude Code остаётся:
поставить Claude Code (бесплатный, та же подписка) → `/login`.

## Ссылки

- https://gist.github.com/shubcodes/3c9c7ff813715aa47018bf22e7cf8cb5 (PKCE-скрипт)
- https://flopsstuff.github.io/coqu/claude-oauth/
