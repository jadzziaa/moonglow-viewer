#!/bin/sh
# Builds "Moonglow Viewer.app" and MoonglowViewer-<version>-macos.dmg in
# target/dist/, for Apple silicon and Intel Macs (one universal binary).
# Run on macOS with both Rust targets installed:
#   rustup target add aarch64-apple-darwin x86_64-apple-darwin
# The app is signed ad hoc, so Gatekeeper refuses its first start until the
# user allows it (System Settings > Privacy & Security > Open Anyway); to
# pass Gatekeeper, sign it with a Developer ID (CODESIGN_IDENTITY) and
# notarize the dmg. The command-line tool is in the app:
# "Moonglow Viewer.app/Contents/MacOS/mgv".
set -eu
cd "$(dirname "$0")/../.."
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
OUT=target/dist
APP="$OUT/Moonglow Viewer.app"
export MACOSX_DEPLOYMENT_TARGET=11.0

for t in aarch64-apple-darwin x86_64-apple-darwin; do
    cargo build --profile dist --locked -p moonglow-viewer -p mgv --target $t
done
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
for b in moonglow-viewer mgv; do
    lipo -create -output "$APP/Contents/MacOS/$b" \
        target/aarch64-apple-darwin/dist/$b target/x86_64-apple-darwin/dist/$b
done
sed "s/@VERSION@/$VERSION/g" packaging/macos/Info.plist > "$APP/Contents/Info.plist"
cp packaging/icons/moonglow-viewer.icns "$APP/Contents/Resources/"
cp LICENSE "$APP/Contents/Resources/"
python3 packaging/third_party_licenses.py --target aarch64-apple-darwin \
    --output "$APP/Contents/Resources/THIRD-PARTY-LICENSES.txt"
cp -R docs/manual "$APP/Contents/Resources/manual"
codesign --force --deep --options runtime --sign "${CODESIGN_IDENTITY:--}" "$APP"

DMG="$OUT/MoonglowViewer-$VERSION-macos.dmg"
STAGE="$OUT/dmg"
rm -rf "$STAGE" "$DMG"
mkdir -p "$STAGE"
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
hdiutil create -volname "Moonglow Viewer" -srcfolder "$STAGE" -ov -format UDZO "$DMG"
rm -rf "$STAGE"
echo "$DMG"
