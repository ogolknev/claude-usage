#!/usr/bin/env bash
# Собирает release-бинарь и упаковывает в «Claude Usage.app» (menu-bar agent).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

CARGO="${CARGO:-$HOME/.cargo/bin/cargo}"
APP="Claude Usage.app"
VERSION="$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)".*/\1/')"

"$CARGO" build --release

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS"
cp "target/release/claude-usage" "$APP/Contents/MacOS/claude-usage"
cp "bundle/Info.plist" "$APP/Contents/Info.plist"

# Подставляем реальную версию из Cargo.toml (в шаблоне Info.plist зашит плейсхолдер).
/usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $VERSION" "$APP/Contents/Info.plist" >/dev/null 2>&1 || true
/usr/libexec/PlistBuddy -c "Set :CFBundleVersion $VERSION" "$APP/Contents/Info.plist" >/dev/null 2>&1 || true

# Подписываем стабильным самоподписанным сертификатом (не ad-hoc): его designated
# requirement = identifier + cert leaf, неизменный между сборками. Это нужно, чтобы
# Keychain-ACL нашего OAuth-токена (security-framework, app-bound) переживал
# обновления приложения без повторных промптов. Сертификат «Claude Usage Signing»
# должен лежать в login-keychain сборочной машины (как создать — docs/oauth-login.md).
# Если его нет — фолбэк на ad-hoc (тогда после апдейта будет разовый промпт).
SIGN_ID="${SIGN_ID:-Claude Usage Signing}"
if security find-identity -p codesigning 2>/dev/null | grep -q "$SIGN_ID"; then
  codesign --force --sign "$SIGN_ID" --identifier com.local.claude-usage "$APP"
else
  echo "⚠ нет сертификата «$SIGN_ID» — подпись ad-hoc (keychain-ACL не переживёт апдейт)"
  codesign --force --sign - --identifier com.local.claude-usage "$APP" || true
fi

echo "Готово: $ROOT/$APP"
echo "Запуск:  open \"$ROOT/$APP\""
