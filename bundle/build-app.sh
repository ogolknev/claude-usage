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

# Ad-hoc подпись. Доступ к Keychain не зависит от неё: токен читается через
# системный /usr/bin/security, которому доступ выдаётся один раз и навсегда.
codesign --force --sign - --identifier com.local.claude-usage "$APP" || true

echo "Готово: $ROOT/$APP"
echo "Запуск:  open \"$ROOT/$APP\""
