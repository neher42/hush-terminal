#!/bin/sh
# Builds Hush.app from the native version into target/release/bundle.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
version=$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/hush-gpui/Cargo.toml" | head -n 1)
app="$root/target/release/bundle/Hush.app"

cargo build --release -p hush-gpui --manifest-path "$root/Cargo.toml"

rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$root/target/release/hush-gpui" "$app/Contents/MacOS/Hush"
cp "$root/src-tauri/icons/icon.icns" "$app/Contents/Resources/Hush.icns"
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Hush</string>
  <key>CFBundleDisplayName</key><string>Hush</string>
  <key>CFBundleIdentifier</key><string>dev.hush.native</string>
  <key>CFBundleExecutable</key><string>Hush</string>
  <key>CFBundleIconFile</key><string>Hush</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.developer-tools</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST
codesign --force --sign - "$app"
echo "$app"
