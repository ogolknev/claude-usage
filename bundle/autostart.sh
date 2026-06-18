#!/usr/bin/env bash
# Включает/выключает автозапуск при входе в систему через LaunchAgent.
#   bash bundle/autostart.sh on    — поставить и запустить сейчас
#   bash bundle/autostart.sh off   — убрать
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
LABEL="com.local.claude-usage"
PLIST="$HOME/Library/LaunchAgents/$LABEL.plist"
UID_NUM="$(id -u)"

# Предпочитаем установленную копию (brew/DMG → /Applications), иначе локальную сборку.
APP_INSTALLED="/Applications/Claude Usage.app/Contents/MacOS/claude-usage"
APP_LOCAL="$ROOT/Claude Usage.app/Contents/MacOS/claude-usage"
if [ -x "$APP_INSTALLED" ]; then BIN="$APP_INSTALLED"; else BIN="$APP_LOCAL"; fi

case "${1:-on}" in
  on)
    [ -x "$BIN" ] || { echo "Сначала собери .app: bash bundle/build-app.sh"; exit 1; }
    # Убираем уже запущенные вручную копии, чтобы launchd держал ровно одну.
    pkill -f "Claude Usage.app/Contents/MacOS/claude-usage" 2>/dev/null || true
    mkdir -p "$HOME/Library/LaunchAgents"
    cat > "$PLIST" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>$LABEL</string>
  <key>ProgramArguments</key>
  <array><string>$BIN</string></array>
  <key>RunAtLoad</key><true/>
  <key>ProcessType</key><string>Interactive</string>
</dict>
</plist>
EOF
    launchctl bootout "gui/$UID_NUM/$LABEL" 2>/dev/null || true
    launchctl bootstrap "gui/$UID_NUM" "$PLIST"
    echo "Автозапуск включён: $PLIST"
    ;;
  off)
    launchctl bootout "gui/$UID_NUM/$LABEL" 2>/dev/null || true
    rm -f "$PLIST"
    echo "Автозапуск выключен"
    ;;
  *)
    echo "use: autostart.sh on|off"; exit 1
    ;;
esac
