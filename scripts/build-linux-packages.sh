#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="$(awk -F '"' '/^version =/ { print $2; exit }' "$ROOT/Cargo.toml")"
if [[ -z "$VERSION" ]]; then
  echo "failed to read package version from Cargo.toml" >&2
  exit 1
fi

RAW_ARCH="$(uname -m)"
case "$RAW_ARCH" in
  x86_64|amd64) ASSET_ARCH="x86_64"; DEB_ARCH="amd64" ;;
  aarch64|arm64) ASSET_ARCH="arm64"; DEB_ARCH="arm64" ;;
  *) ASSET_ARCH="$RAW_ARCH"; DEB_ARCH="$RAW_ARCH" ;;
esac

DIST="$ROOT/dist"
WORK="$ROOT/build/linux-package"
BIN="$ROOT/target/release/modelrack"
APP_ID="dev.modelrack.ModelRack"
PORTABLE_DIR="$WORK/ModelRack"
TAR="$DIST/ModelRack-v${VERSION}-linux-${ASSET_ARCH}.tar.gz"
DEB_ROOT="$WORK/deb-root"
DEB="$DIST/modelrack_${VERSION}_${DEB_ARCH}.deb"

mkdir -p "$DIST" "$WORK"
cargo build --release --manifest-path "$ROOT/Cargo.toml"
if [[ ! -x "$BIN" ]]; then
  echo "missing release binary: $BIN" >&2
  exit 1
fi

rm -rf "$PORTABLE_DIR" "$TAR" "$TAR.sha256"
mkdir -p "$PORTABLE_DIR/bin" "$PORTABLE_DIR/share/applications" "$PORTABLE_DIR/share/icons/hicolor/256x256/apps"
cp "$BIN" "$PORTABLE_DIR/bin/modelrack"
cp "$ROOT/README.md" "$PORTABLE_DIR/README.md"
cp "$ROOT/assets/AppIcon.iconset/icon_256x256.png" "$PORTABLE_DIR/share/icons/hicolor/256x256/apps/${APP_ID}.png"
cat > "$PORTABLE_DIR/share/applications/${APP_ID}.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=ModelRack
Comment=Desktop-native 3D model library manager for makers
Exec=modelrack %F
Icon=${APP_ID}
Categories=Graphics;Engineering;Utility;
Terminal=false
MimeType=model/stl;model/3mf;application/sla;
DESKTOP

tar -C "$WORK" -czf "$TAR" ModelRack
sha256sum "$TAR" > "$TAR.sha256"

rm -rf "$DEB_ROOT" "$DEB" "$DEB.sha256"
install -Dm755 "$BIN" "$DEB_ROOT/usr/bin/modelrack"
install -Dm644 "$ROOT/assets/AppIcon.iconset/icon_256x256.png" "$DEB_ROOT/usr/share/icons/hicolor/256x256/apps/${APP_ID}.png"
install -Dm644 "$PORTABLE_DIR/share/applications/${APP_ID}.desktop" "$DEB_ROOT/usr/share/applications/${APP_ID}.desktop"
mkdir -p "$DEB_ROOT/DEBIAN"
INSTALLED_SIZE="$(du -sk "$DEB_ROOT/usr" | awk '{print $1}')"
cat > "$DEB_ROOT/DEBIAN/control" <<CONTROL
Package: modelrack
Version: ${VERSION}
Section: graphics
Priority: optional
Architecture: ${DEB_ARCH}
Maintainer: ModelRack <hello@modelrack.dev>
Installed-Size: ${INSTALLED_SIZE}
Depends: libc6, libfontconfig1, libxkbcommon0, libxcb1, libwayland-client0
Homepage: https://github.com/stormixus/ModelRack
Description: Desktop-native 3D model library manager for makers
 ModelRack helps organize, preview, tag, and open 3D printing model libraries.
CONTROL

dpkg-deb --build --root-owner-group "$DEB_ROOT" "$DEB"
sha256sum "$DEB" > "$DEB.sha256"

echo "Linux packages written to $DIST"
