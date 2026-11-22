#!/bin/bash
# Build SkiFree.app (macOS arm64) into dist/.
#
# Assembles the bundle from the release binary and the extracted assets
# (run tools/extract_assets.py first; build.rs enforces this), generates an
# .icns from the original's 32x32 icon, and ad-hoc signs the result.
set -euo pipefail
cd "$(dirname "$0")/.."

APP="dist/SkiFree.app"
BIN="target/release/skifree"

cargo build --release --bin skifree

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN" "$APP/Contents/MacOS/SkiFree"
cp -R assets "$APP/Contents/Resources/assets"

# Icon: the original ships 32x32 only; let iconutil scale from that.
ICONSET="$(mktemp -d)/skifree.iconset"
mkdir -p "$ICONSET"
SRC="assets/icons/icon_1.png"
for size in 16 32 128 256 512; do
  sips -z "$size" "$size" "$SRC" --out "$ICONSET/icon_${size}x${size}.png" > /dev/null
  sips -z "$((size * 2))" "$((size * 2))" "$SRC" --out "$ICONSET/icon_${size}x${size}@2x.png" > /dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/SkiFree.icns"

cat > "$APP/Contents/Info.plist" << 'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>SkiFree</string>
    <key>CFBundleDisplayName</key>
    <string>SkiFree</string>
    <key>CFBundleIdentifier</key>
    <string>dev.nivaldomcj.skifree</string>
    <key>CFBundleVersion</key>
    <string>0.1.0</string>
    <key>CFBundleShortVersionString</key>
    <string>0.1.0</string>
    <key>CFBundleExecutable</key>
    <string>SkiFree</string>
    <key>CFBundleIconFile</key>
    <string>SkiFree</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>LSMinimumSystemVersion</key>
    <string>11.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
EOF

codesign --force --deep --sign - "$APP" > /dev/null
echo "built $APP"
