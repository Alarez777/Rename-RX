#!/usr/bin/env bash
#
# Build a test AppImage for Rename RX.
#
# By default this builds a release binary on the host. Set
# RENAME_RX_SKIP_BUILD=1 and RENAME_RX_BINARY=/path/to/rename-rx to package a
# binary built elsewhere (for example in an older container for wider glibc
# compatibility).

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST="$ROOT/dist"
APPDIR="$DIST/Rename_RX.AppDir"
OUTPUT="$DIST/Rename_RX-x86_64.AppImage"
TOOLS="$DIST/.tools"
APPIMAGETOOL="$TOOLS/appimagetool-x86_64.AppImage"
APPIMAGETOOL_URL="${APPIMAGETOOL_URL:-https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage}"

if [[ -z "${RENAME_RX_SKIP_BUILD:-}" ]]; then
    cargo build --locked --release -p rename-rx --manifest-path "$ROOT/Cargo.toml"
fi

BINARY="${RENAME_RX_BINARY:-$ROOT/target/release/rename-rx}"
if [[ ! -x "$BINARY" ]]; then
    echo "error: binary not found or not executable: $BINARY" >&2
    exit 1
fi

rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin"
install -m 0755 "$BINARY" "$APPDIR/usr/bin/rename-rx"
install -m 0644 "$ROOT/packaging/rename-rx.desktop" "$APPDIR/rename-rx.desktop"
install -m 0644 "$ROOT/assets/rename-rx.svg" "$APPDIR/rename-rx.svg"
cp "$APPDIR/rename-rx.svg" "$APPDIR/.DirIcon"

cat > "$APPDIR/AppRun" <<'EOF'
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
exec "$HERE/usr/bin/rename-rx" "$@"
EOF
chmod 0755 "$APPDIR/AppRun"

mkdir -p "$TOOLS"
if [[ ! -x "$APPIMAGETOOL" ]]; then
    echo "Downloading appimagetool..."
    curl --fail --location --silent --show-error "$APPIMAGETOOL_URL" -o "$APPIMAGETOOL"
    chmod 0755 "$APPIMAGETOOL"
fi

rm -f "$OUTPUT"
ARCH=x86_64 "$APPIMAGETOOL" --appimage-extract-and-run "$APPDIR" "$OUTPUT"
chmod 0755 "$OUTPUT"

echo
echo "AppImage: $OUTPUT"
ls -lh "$OUTPUT"
