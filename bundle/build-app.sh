#!/usr/bin/env bash
# Собирает release-бинарь и упаковывает в «Claude Usage.app» (menu-bar agent).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

CARGO="${CARGO:-$HOME/.cargo/bin/cargo}"
APP="Claude Usage.app"

"$CARGO" build --release

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS"
cp "target/release/claude-usage" "$APP/Contents/MacOS/claude-usage"
cp "bundle/Info.plist" "$APP/Contents/Info.plist"

# Ad-hoc подпись. Замечание: cdhash меняется при каждой пересборке, поэтому
# разрешение Keychain «Always Allow» придётся выдавать заново после ребилда.
# Для постоянного ACL подпиши self-signed identity вместо «-».
codesign --force --sign - --identifier com.local.claude-usage "$APP" || true

echo "Готово: $ROOT/$APP"
echo "Запуск:  open \"$ROOT/$APP\""
