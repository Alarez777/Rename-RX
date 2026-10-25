#!/usr/bin/env bash
#
# Build a Rename RX AppImage.
#
# By default this builds a release binary for the host architecture. Set
# RENAME_RX_ARCH=aarch64 to package an ARM64 binary, and use
# RENAME_RX_SKIP_BUILD=1 / RENAME_RX_BINARY=/path/to/rename-rx to package a
# binary built elsewhere (for example in an older container for wider glibc
# compatibility).

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST="$ROOT/dist"
APPDIR="$DIST/Rename_RX.AppDir"
TARGET_ARCH="${RENAME_RX_ARCH:-x86_64}"
OUTPUT="$DIST/Rename_RX-${TARGET_ARCH}.AppImage"
UPDATE_INFORMATION="${RENAME_RX_UPDATE_INFORMATION:-gh-releases-zsync|Alarez777|Rename-RX|latest|Rename_RX-${TARGET_ARCH}.AppImage.zsync}"
TOOLS="$DIST/.tools"
case "$(uname -m)" in
    x86_64)
        TOOL_ARCH="x86_64"
        ;;
    aarch64|arm64)
        TOOL_ARCH="aarch64"
        ;;
    *)
        echo "error: unsupported host architecture: $(uname -m)" >&2
        exit 1
        ;;
esac
APPIMAGETOOL="$TOOLS/appimagetool-${TOOL_ARCH}.AppImage"
APPIMAGETOOL_URL="${APPIMAGETOOL_URL:-https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-${TOOL_ARCH}.AppImage}"

case "$TARGET_ARCH" in
    x86_64|aarch64) ;;
    *)
        echo "error: unsupported architecture: $TARGET_ARCH (use x86_64 or aarch64)" >&2
        exit 1
        ;;
esac

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
install -m 0644 "$ROOT/packaging/io.github.alarez777.RenameRX.desktop" \
    "$APPDIR/io.github.alarez777.RenameRX.desktop"
install -m 0644 "$ROOT/assets/rename-rx.svg" \
    "$APPDIR/io.github.alarez777.RenameRX.svg"
cp "$APPDIR/io.github.alarez777.RenameRX.svg" "$APPDIR/.DirIcon"

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

rm -f "$OUTPUT" "$OUTPUT.zsync"
ARCH="$TARGET_ARCH" "$APPIMAGETOOL" --appimage-extract-and-run \
    -u "$UPDATE_INFORMATION" "$APPDIR" "$OUTPUT"
chmod 0755 "$OUTPUT"

# appimagetool/zsyncmake writes the zsync file in the current directory,
# while the AppImage goes to $DIST. Keep both release assets together.
ZSYNC_GENERATED="$PWD/$(basename "$OUTPUT").zsync"
if [[ ! -f "$OUTPUT.zsync" && -f "$ZSYNC_GENERATED" ]]; then
    mv "$ZSYNC_GENERATED" "$OUTPUT.zsync"
fi
if [[ ! -f "$OUTPUT.zsync" ]]; then
    echo "error: appimagetool did not generate $(basename "$OUTPUT").zsync" >&2
    exit 1
fi

echo
echo "AppImage: $OUTPUT"
echo "zsync:   $OUTPUT.zsync"
ls -lh "$OUTPUT" "$OUTPUT.zsync"
