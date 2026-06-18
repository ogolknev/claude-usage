#!/usr/bin/env bash
# Собирает DMG-инсталлятор (перетащи .app в Applications) из готового бандла.
# Использует create-dmg для оформленного окна; если его нет — чистый hdiutil.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

APP="Claude Usage.app"
VERSION="$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)".*/\1/')"
DMG="Claude-Usage-${VERSION}.dmg"

[ -d "$APP" ] || { echo "Сначала собери .app: bash bundle/build-app.sh"; exit 1; }
rm -f "$DMG"

if command -v create-dmg >/dev/null 2>&1; then
  # create-dmg иногда возвращает ненулевой код из-за стилизации окна — проверяем
  # факт создания файла отдельно.
  create-dmg \
    --volname "Claude Usage" \
    --window-pos 200 120 \
    --window-size 520 360 \
    --icon-size 100 \
    --icon "$APP" 140 185 \
    --app-drop-link 380 185 \
    --no-internet-enable \
    "$DMG" "$APP" || true
else
  echo "create-dmg не найден — собираю простой DMG через hdiutil"
  STAGE="$(mktemp -d)"
  cp -R "$APP" "$STAGE/"
  ln -s /Applications "$STAGE/Applications"
  hdiutil create -volname "Claude Usage" -srcfolder "$STAGE" -ov -format UDZO "$DMG" >/dev/null
  rm -rf "$STAGE"
fi

[ -f "$DMG" ] || { echo "DMG не создан"; exit 1; }
echo "Готово: $ROOT/$DMG"
