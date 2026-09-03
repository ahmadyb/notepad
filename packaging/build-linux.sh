#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)";VERSION=1.0.2;ARCH="$(dpkg --print-architecture 2>/dev/null||printf amd64)";STAGE="$ROOT/.package/notepad-pro";OUT="$ROOT/artifacts"
cargo build --release --manifest-path "$ROOT/Cargo.toml" -p notepad-pro
rm -rf "$STAGE";mkdir -p "$STAGE/usr/bin" "$STAGE/usr/share/applications" "$STAGE/usr/share/icons/hicolor/scalable/apps" "$STAGE/DEBIAN";install -m755 "$ROOT/target/release/notepad-pro" "$STAGE/usr/bin/notepad-pro";install -m644 "$ROOT/packaging/notepad-pro.desktop" "$STAGE/usr/share/applications/notepad-pro.desktop";install -m644 "$ROOT/packaging/notepad-pro.svg" "$STAGE/usr/share/icons/hicolor/scalable/apps/notepad-pro.svg"
cat > "$STAGE/DEBIAN/control" <<EOF
Package: notepad-pro
Version: $VERSION
Section: editors
Priority: optional
Architecture: $ARCH
Maintainer: NotePad Pro Contributors
Description: NotePad Pro native note editor
 A fast native text editor and note system.
EOF
mkdir -p "$OUT";dpkg-deb --build --root-owner-group "$STAGE" "$OUT/notepad-pro_${VERSION}_${ARCH}.deb";tar -C "$STAGE" -czf "$OUT/notepad-pro-${VERSION}-linux-${ARCH}.tar.gz" .;echo "Created packages in $OUT"
