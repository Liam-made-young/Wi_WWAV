#!/usr/bin/env bash
# Builds Wi_WWAV for macOS, signs it inside out with the hardened runtime,
# notarizes and staples it, and writes the updater's files (docs/SPEC.md 9.9).
#
#   tools/release/macos.sh
#
# Run it on a Mac with Xcode's command line tools, Rust with the
# aarch64-apple-darwin and x86_64-apple-darwin targets, Node, CMake, Ninja
# and the Tauri CLI (cargo install tauri-cli --version '^2' --locked), after
# tools/bootstrap.sh. It reads:
#
#   WI_SIGNING_IDENTITY  "Developer ID Application: <name> (<team id>)", in the
#                        login keychain
#   WI_NOTARY_PROFILE    a notarytool keychain profile, made once with
#                        xcrun notarytool store-credentials
#   TAURI_SIGNING_PRIVATE_KEY_PATH (or TAURI_SIGNING_PRIVATE_KEY) and
#   TAURI_SIGNING_PRIVATE_KEY_PASSWORD
#                        the updater's Ed25519 key (tools/release/README.md)
#   WI_DOWNLOAD_BASE     where the archive will be served (default
#                        https://mi-wwav.com/desktop/<version>)
#
# Everything is built in ~/Library/Developer/wi-wwav-build, outside iCloud
# Desktop and Documents, which stamp com.apple.FinderInfo onto files that
# codesign then refuses; and xattr -cr runs before signing anyway. The DMG,
# the updater archive, its signature and latest.json land in
# ~/Library/Developer/wi-wwav-build/release/<version>/.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
APP_SRC="$ROOT/app/src-tauri"
BUILD="$HOME/Library/Developer/wi-wwav-build"
TARGET=universal-apple-darwin

die() { echo "macos.sh: $*" >&2; exit 1; }
step() { echo; echo "== $*"; }
conf() { node -p "require(process.argv[1])$1" "$APP_SRC/tauri.conf.json"; }

[ "$(uname -s)" = Darwin ] || die "this builds the Mac app, so it runs on a Mac."
[ -n "${WI_SIGNING_IDENTITY:-}" ] || die "set WI_SIGNING_IDENTITY to your Developer ID Application identity."
[ -n "${WI_NOTARY_PROFILE:-}" ] || die "set WI_NOTARY_PROFILE to a notarytool keychain profile."
[ -n "${TAURI_SIGNING_PRIVATE_KEY:-}${TAURI_SIGNING_PRIVATE_KEY_PATH:-}" ] \
  || die "set TAURI_SIGNING_PRIVATE_KEY_PATH to the updater's private key."
[ -n "$(conf .plugins.updater.pubkey)" ] \
  || die "tauri.conf.json has no updater public key; make the key pair first (tools/release/README.md)."
[ -f "$ROOT/formats/prana/core/base/base.h" ] || die "formats/ is empty: run tools/bootstrap.sh."

VERSION="$(conf .version)"
OUT="$BUILD/release/$VERSION"
BASE="${WI_DOWNLOAD_BASE:-https://mi-wwav.com/desktop/$VERSION}"
mkdir -p "$OUT"

sign() { codesign --force --timestamp --options runtime --sign "$WI_SIGNING_IDENTITY" "$@"; }
universal() {
  case "$(lipo -archs "$1")" in
    *arm64*x86_64* | *x86_64*arm64*) ;;
    *) die "$1 is not universal: $(lipo -archs "$1")" ;;
  esac
}
notarize() {
  xcrun notarytool submit "$1" --keychain-profile "$WI_NOTARY_PROFILE" --wait \
    || die "notarization of $1 failed; xcrun notarytool log <id> --keychain-profile $WI_NOTARY_PROFILE says why."
}

step "The engine and the scanner, arm64 and x86_64"
bash "$ROOT/tools/fetch_juce.sh"
cmake -S "$ROOT/engine" -B "$BUILD/engine" -G Ninja -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_OSX_ARCHITECTURES="arm64;x86_64"
ninja -C "$BUILD/engine" wwav-engine wwav-scan

step "Staging the helpers where the bundle takes them (tauri.macos.conf.json)"
HELPERS="$APP_SRC/helpers"
rm -rf "$HELPERS"
mkdir -p "$HELPERS"
ditto "$BUILD/engine/wwav-engine_artefacts/Release/wwav-engine.app" "$HELPERS/wwav-engine.app"
# Our Info.plist (LSUIElement, the microphone sentence) over JUCE's.
cp "$ROOT/engine/macos/Info.plist" "$HELPERS/wwav-engine.app/Contents/Info.plist"
plutil -replace CFBundleShortVersionString -string "$VERSION" "$HELPERS/wwav-engine.app/Contents/Info.plist"
plutil -replace CFBundleVersion -string "$VERSION" "$HELPERS/wwav-engine.app/Contents/Info.plist"
ditto "$BUILD/engine/wwav-scan" "$HELPERS/wwav-scan"

step "The app, arm64 and x86_64"
npm --prefix "$ROOT/app/ui" ci
(cd "$APP_SRC" && CARGO_TARGET_DIR="$BUILD/target" cargo tauri build --ci --target "$TARGET" --bundles app --no-sign)
APP="$BUILD/target/$TARGET/release/bundle/macos/Wi_WWAV.app"
ENGINE_APP="$APP/Contents/Helpers/wwav-engine.app"
SCAN="$APP/Contents/Helpers/wwav-scan"
for bin in "$APP/Contents/MacOS/wi-wwav" "$ENGINE_APP/Contents/MacOS/wwav-engine" "$SCAN"; do
  universal "$bin"
done

step "Clearing extended attributes"
xattr -cr "$APP"

step "Signing inside out: libraries, then the helpers, then the app"
if [ -d "$APP/Contents/Frameworks" ]; then
  # FFmpeg's shared libraries (9.10), when the video work brings them.
  find "$APP/Contents/Frameworks" -type f -name '*.dylib' -print0 | while IFS= read -r -d '' lib; do sign "$lib"; done
fi
sign --identifier com.mi-wwav.wi-wwav.scan --entitlements "$ROOT/engine/macos/wwav-scan.entitlements" "$SCAN"
sign --entitlements "$ROOT/engine/macos/wwav-engine.entitlements" "$ENGINE_APP"
sign --entitlements "$APP_SRC/entitlements.plist" "$APP"
codesign --verify --deep --strict --verbose=2 "$APP"

step "Notarizing the app, so it opens offline too"
ZIP="$BUILD/Wi_WWAV-notarize.zip"
ditto -c -k --keepParent "$APP" "$ZIP"
notarize "$ZIP"
xcrun stapler staple "$APP"

step "The disk image"
DMG="$OUT/Wi_WWAV_${VERSION}_universal.dmg"
STAGE="$BUILD/dmg"
rm -rf "$STAGE"
mkdir -p "$STAGE"
ditto "$APP" "$STAGE/Wi_WWAV.app"
ln -s /Applications "$STAGE/Applications"
hdiutil create -volname Wi_WWAV -srcfolder "$STAGE" -format UDZO -ov "$DMG"
codesign --force --timestamp --sign "$WI_SIGNING_IDENTITY" "$DMG"
notarize "$DMG"
xcrun stapler staple "$DMG"

step "Checking what a downloaded copy will meet"
xcrun stapler validate "$DMG"
spctl --assess --type open --context context:primary-signature --verbose=2 "$DMG"
spctl --assess --type execute --verbose=2 "$APP"
codesign --verify --deep --strict --verbose=2 "$APP"

step "The updater's archive and manifest"
ARCHIVE="$OUT/Wi_WWAV.app.tar.gz"
COPYFILE_DISABLE=1 tar -czf "$ARCHIVE" -C "$(dirname "$APP")" Wi_WWAV.app
(cd "$APP_SRC" && cargo tauri signer sign --app-version "$VERSION" "$ARCHIVE")
# The universal archive serves both kinds of Mac.
node - "$VERSION" "$BASE/Wi_WWAV.app.tar.gz" "$ARCHIVE.sig" "$OUT/latest.json" <<'EOF'
const [version, url, sigPath, out] = process.argv.slice(2);
const signature = require('fs').readFileSync(sigPath, 'utf8').trim();
const mac = { signature, url };
const manifest = {
  version,
  pub_date: new Date().toISOString().replace(/\.\d+Z$/, 'Z'),
  platforms: { 'darwin-aarch64': mac, 'darwin-x86_64': mac },
};
require('fs').writeFileSync(out, JSON.stringify(manifest, null, 2) + '\n');
EOF

echo
echo "Done: $OUT"
echo "Upload Wi_WWAV.app.tar.gz to $BASE/, then latest.json to https://mi-wwav.com/desktop/latest.json."
