#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_DIR="${1:-$ROOT/.omx/artifacts/runtime}"
mkdir -p "$OUT_DIR"

APP="$ROOT/build/ModelRack.app"
SHOT="$OUT_DIR/modelrack-smoke-$(date -u +%Y%m%dT%H%M%SZ).png"

if [[ ! -d "$APP" ]]; then
  echo "Missing app bundle: $APP" >&2
  exit 1
fi

pkill -x modelrack >/dev/null 2>&1 || true
open -n "$APP"
sleep 2
screencapture -x "$SHOT"

if [[ ! -s "$SHOT" ]]; then
  echo "Screenshot capture failed: $SHOT" >&2
  exit 1
fi

echo "$SHOT"
