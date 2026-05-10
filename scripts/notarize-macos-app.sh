#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROFILE="${MODELRACK_NOTARY_PROFILE:-}"
SIGN_IDENTITY="${MODELRACK_SIGN_IDENTITY:-}"
BUNDLE_ID="${MODELRACK_BUNDLE_ID:-dev.modelrack.ModelRack}"

usage() {
  cat >&2 <<USAGE
usage: $0 --keychain-profile <profile> [--sign-identity <developer-id-identity>]

Environment alternatives:
  MODELRACK_NOTARY_PROFILE=<profile>
  MODELRACK_SIGN_IDENTITY=<Developer ID Application: ...>
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --keychain-profile)
      PROFILE="${2:-}"
      shift 2
      ;;
    --sign-identity)
      SIGN_IDENTITY="${2:-}"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      usage
      exit 2
      ;;
  esac
done

if [[ -z "$PROFILE" ]]; then
  usage
  printf '\nMissing notarytool keychain profile. Create one with:\n' >&2
  printf '  xcrun notarytool store-credentials modelrack --apple-id <apple-id> --team-id <team-id> --password <app-specific-password>\n' >&2
  exit 2
fi

if [[ -z "$SIGN_IDENTITY" ]]; then
  SIGN_IDENTITY="$(security find-identity -v -p codesigning | sed -n 's/.*"\(Developer ID Application: .*\)"/\1/p' | head -1)"
fi
if [[ -z "$SIGN_IDENTITY" ]]; then
  printf 'No Developer ID Application signing identity found.\n' >&2
  exit 2
fi

VERSION="$(awk -F '"' '/^version =/ { print $2; exit }' "$ROOT/Cargo.toml")"
ARCH="$(uname -m)"
APP="$ROOT/build/ModelRack.app"
DIST="$ROOT/dist"
SUBMIT_ZIP="$DIST/ModelRack-v${VERSION}-macos-${ARCH}.notary-submit.zip"
FINAL_ZIP="$DIST/ModelRack-v${VERSION}-macos-${ARCH}.notarized.zip"
FINAL_SUM="$FINAL_ZIP.sha256"

mkdir -p "$DIST"
MODELRACK_SIGN_IDENTITY="$SIGN_IDENTITY" "$ROOT/scripts/build-macos-app.sh" --release

codesign --verify --deep --strict --verbose=2 "$APP"
codesign -dv --verbose=4 "$APP" 2>&1 | sed -n '1,80p'

rm -f "$SUBMIT_ZIP" "$FINAL_ZIP" "$FINAL_SUM"
ditto -c -k --keepParent "$APP" "$SUBMIT_ZIP"

xcrun notarytool submit "$SUBMIT_ZIP" \
  --keychain-profile "$PROFILE" \
  --wait

xcrun stapler staple "$APP"
xcrun stapler validate "$APP"
spctl --assess --type execute --verbose=4 "$APP"

ditto -c -k --keepParent "$APP" "$FINAL_ZIP"
shasum -a 256 "$FINAL_ZIP" > "$FINAL_SUM"

printf 'Notarized app: %s\n' "$APP"
printf 'Release asset: %s\n' "$FINAL_ZIP"
printf 'Checksum: '
cat "$FINAL_SUM"
