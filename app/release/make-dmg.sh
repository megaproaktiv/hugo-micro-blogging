#!/bin/sh
# Baut das Mac-Release von hmb: hmb.app (mit µ-Symbol) und ein DMG zum Verteilen.
#
#   ./release/make-dmg.sh            ->  dist/hmb-<version>-macos-<arch>.dmg
#
# Universal (Apple Silicon + Intel) wird es automatisch, wenn beide Rust-Ziele installiert sind:
#   rustup target add aarch64-apple-darwin x86_64-apple-darwin
#
# Signieren: ohne Apple-Entwicklerzertifikat wird "ad hoc" signiert (läuft, macOS fragt beim
# ersten Start einmal nach). Mit Zertifikat:  SIGN_ID="Developer ID Application: …" ./release/make-dmg.sh
set -e
cd "$(dirname "$0")/.."

NAME=hmb
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
BUNDLE_ID=${BUNDLE_ID:-io.github.hmb}
SIGN_ID=${SIGN_ID:--}
DIST=dist
WORK=$DIST/work
APP=$WORK/$NAME.app

rm -rf "$WORK"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

# 1) build (universal if both targets are installed)
TARGETS=$(rustup target list --installed 2>/dev/null || true)
if echo "$TARGETS" | grep -q aarch64-apple-darwin && echo "$TARGETS" | grep -q x86_64-apple-darwin; then
  cargo build --release --target aarch64-apple-darwin
  cargo build --release --target x86_64-apple-darwin
  lipo -create -output "$APP/Contents/MacOS/$NAME" \
    target/aarch64-apple-darwin/release/$NAME target/x86_64-apple-darwin/release/$NAME
  ARCH=universal
else
  cargo build --release
  cp target/release/$NAME "$APP/Contents/MacOS/$NAME"
  ARCH=$(uname -m)
fi

# 2) icon
"$APP/Contents/MacOS/$NAME" --export-iconset "$WORK/$NAME.iconset"
iconutil -c icns "$WORK/$NAME.iconset" -o "$APP/Contents/Resources/$NAME.icns"

# 3) Info.plist
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleName</key><string>$NAME</string>
	<key>CFBundleDisplayName</key><string>hmb – Hugo Microblogger</string>
	<key>CFBundleIdentifier</key><string>$BUNDLE_ID</string>
	<key>CFBundleVersion</key><string>$VERSION</string>
	<key>CFBundleShortVersionString</key><string>$VERSION</string>
	<key>CFBundleExecutable</key><string>$NAME</string>
	<key>CFBundleIconFile</key><string>$NAME</string>
	<key>CFBundlePackageType</key><string>APPL</string>
	<key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
	<key>LSMinimumSystemVersion</key><string>11.0</string>
	<key>LSApplicationCategoryType</key><string>public.app-category.productivity</string>
	<key>NSHighResolutionCapable</key><true/>
	<key>NSHumanReadableCopyright</key><string>hmb – Hugo Microblogger</string>
</dict>
</plist>
PLIST

# 4) sign (ad hoc without certificate)
codesign --force --deep --options runtime --timestamp=none --sign "$SIGN_ID" "$APP"
codesign --verify --deep --strict "$APP"

# 5) DMG with a shortcut to /Applications
STAGE=$WORK/dmg
mkdir -p "$STAGE"
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Programme"
DMG=$DIST/$NAME-$VERSION-macos-$ARCH.dmg
rm -f "$DMG"
# Not via -srcfolder: its temporary volume is mounted visibly, Spotlight/XProtect scan it,
# and the unmount then fails with "Ressource ist belegt". So: blank image, mount it
# invisibly (-nobrowse), copy, detach, compress.
RW=$WORK/rw.dmg
MNT=$WORK/mnt
SIZE=$(( $(du -sm "$STAGE" | cut -f1) + 20 ))
hdiutil create -size "${SIZE}m" -fs HFS+ -volname "$NAME $VERSION" -ov "$RW" > /dev/null
mkdir -p "$MNT"
hdiutil attach "$RW" -nobrowse -noautoopen -mountpoint "$MNT" > /dev/null
ditto "$STAGE" "$MNT"
n=0
until hdiutil detach "$MNT" > /dev/null 2>&1; do
  n=$((n + 1))
  if [ $n -ge 5 ]; then hdiutil detach -force "$MNT" > /dev/null; break; fi
  sleep 1
done
hdiutil convert "$RW" -format UDZO -ov -o "$DMG" > /dev/null
rm -f "$RW"
shasum -a 256 "$DMG" | tee "$DMG.sha256"

echo "Fertig: $DMG"
