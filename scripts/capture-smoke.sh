#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_DIR="${1:-$ROOT/.omx/artifacts/runtime}"
mkdir -p "$OUT_DIR"

APP="$ROOT/build/ModelRack.app"
RUN_ID="modelrack-smoke-$(date -u +%Y%m%dT%H%M%SZ)"
SHOT="$OUT_DIR/$RUN_ID.png"
VISUAL_QA_DIR="$ROOT/.omx/artifacts/visual-qa"
PREFS_PATH="${MODELRACK_PREFS_PATH:-$OUT_DIR/$RUN_ID-prefs.json}"

if [[ ! -d "$APP" ]]; then
  echo "Missing app bundle: $APP" >&2
  exit 1
fi

pkill -x modelrack >/dev/null 2>&1 || true
if [[ -z "${MODELRACK_PREFS_PATH:-}" ]]; then
  cat > "$PREFS_PATH" <<'JSON'
{
  "density": "medium",
  "view_mode": "grid"
}
JSON
fi

open -n "$APP" --env "MODELRACK_PREFS_PATH=$PREFS_PATH"
sleep 2

CAPTURE_CMD="screencapture"
if osascript -e 'tell application "modelrack" to activate' >/dev/null 2>&1 \
  && osascript -e 'tell application "System Events" to set frontmost of process "modelrack" to true' >/dev/null 2>&1; then
  sleep 0.8
fi

if BOUNDS="$(osascript -e 'tell application "System Events" to tell process "modelrack" to get position of window 1 & size of window 1' 2>/dev/null)"; then
  IFS=', ' read -r X Y W H <<< "$BOUNDS"
  screencapture -x -R "$X,$Y,$W,$H" "$SHOT"
  CAPTURE_CMD="scripts/capture-smoke.sh osascript-frontmost screencapture -R $X,$Y,$W,$H"
else
  screencapture -x "$SHOT"
  CAPTURE_CMD="scripts/capture-smoke.sh fullscreen-fallback"
fi

if [[ ! -s "$SHOT" ]]; then
  echo "Screenshot capture failed: $SHOT" >&2
  exit 1
fi

python3 "$ROOT/scripts/visual-qa-artifacts.py" \
  --root "$ROOT" \
  --out-dir "$VISUAL_QA_DIR" \
  --run-id "$RUN_ID" \
  --current "$SHOT" \
  --current-command "$CAPTURE_CMD" \
  --allow-missing-reference >/dev/null

echo "$SHOT"
echo "$VISUAL_QA_DIR/$RUN_ID/current/report.json"
