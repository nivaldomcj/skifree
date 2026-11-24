#!/bin/bash
# Build SkiFree.app (macOS arm64) into dist/.
#
# Assembles the bundle from the release binary and the extracted assets
# (run tools/extract_assets.py first; build.rs enforces this), generates an
# .icns from the original's 32x32 icon, and ad-hoc signs the result.
set -euo pipefail
cd "$(dirname "$0")/.."

APP="dist/SkiFree.app"

cargo build --locked --release --bin skifree
# Where cargo put it: CARGO_TARGET_DIR and build.target-dir move target/.
BIN="$(tools/target_dir.sh)/release/skifree"

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN" "$APP/Contents/MacOS/SkiFree"
cp -R assets "$APP/Contents/Resources/assets"

# Icon: composed at build time from the extracted assets (Big Sur-style
# white squircle + the original's colorful skier glyph). No derived icon
# file is ever committed — see tools/make_icon.py.
ICONSET="$(mktemp -d)/skifree.iconset"
python3 tools/make_icon.py "$ICONSET" > /dev/null
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
