# Releasing Wi_WWAV

A direct download, signed with a Developer ID, notarized, and updated by
Tauri's updater (`docs/SPEC.md` 9.9). **None of this has run on a Mac yet**:
`test_release.py` checks the configuration and runs `macos.sh` against
stand-ins for Apple's tools, which proves the order of the steps but not that
Apple accepts the result. S0.7 stays open until a release is downloaded
through a browser onto a Mac that never built it.

```
python3 -m unittest discover -s tools/release -v    # anywhere
tools/release/macos.sh                              # on a Mac
```

## Once, before the first release

1. **The Developer ID.** Join the Apple Developer Program, make a *Developer
   ID Application* certificate in Xcode or on developer.apple.com, and keep it
   in the login keychain. `security find-identity -v -p codesigning` shows
   its name: `Developer ID Application: <name> (<team id>)`.
2. **The notary's password.** Make an app-specific password at
   appleid.apple.com, then store it as a keychain profile:
   `xcrun notarytool store-credentials wi-notary --apple-id <email> --team-id <team id>`.
3. **The updater's key.** `cargo tauri signer generate -w ~/.tauri/wi-wwav.key`
   makes an Ed25519 key pair. Keep the private key and its password out of
   the repository (a password manager, and nowhere else: whoever holds it can
   ship an update to every copy). Put the public key, the contents of
   `~/.tauri/wi-wwav.key.pub`, in `app/src-tauri/tauri.conf.json` at
   `plugins.updater.pubkey` and commit it. Until it is there, builds never
   look for updates and `macos.sh` refuses to run.

## Each release

1. Raise `version` in `app/src-tauri/tauri.conf.json`.
2. On the Mac, with the toolchain `macos.sh` lists:

   ```
   export WI_SIGNING_IDENTITY="Developer ID Application: <name> (<team id>)"
   export WI_NOTARY_PROFILE=wi-notary
   export TAURI_SIGNING_PRIVATE_KEY_PATH=~/.tauri/wi-wwav.key
   export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=…
   tools/release/macos.sh
   ```

3. Upload `Wi_WWAV.app.tar.gz` from
   `~/Library/Developer/wi-wwav-build/release/<version>/` to R2 where
   `WI_DOWNLOAD_BASE` points (by default `https://mi-wwav.com/desktop/<version>/`),
   then `latest.json` to `https://mi-wwav.com/desktop/latest.json`, and the
   DMG wherever the download link points.

What `macos.sh` does, in order: builds the engine and scanner and the app
for arm64 and x86_64 in `~/Library/Developer/wi-wwav-build` (never under
iCloud Desktop or Documents, whose `com.apple.FinderInfo` codesign refuses);
puts `engine/macos/Info.plist` (a faceless helper, `LSUIElement`) into
`wwav-engine.app` and stages it for `Contents/Helpers`; checks every binary is
universal; clears extended attributes; signs inside out with the hardened
runtime (libraries, then `wwav-scan` and `wwav-engine.app` with their
entitlements in `engine/macos/`, then the app with
`app/src-tauri/entitlements.plist`); verifies; notarizes and staples the app,
so it opens offline too; builds, signs, notarizes and staples the DMG; checks
it with `stapler validate`, `spctl --assess` and `codesign --verify --deep
--strict`; and signs the updater's archive, bound to its version, and writes
`latest.json` for both kinds of Mac.

## Updates

The app reads `latest.json` once at launch and once a day after. A newer
version downloads in the background, the status bar says "Update ready ·
installs when you quit", and it installs only as the app quits, never
mid-playback. The manifest itself isn't signed, so the app requires the
archive's signature to carry the version it was signed for
(`requireSignedVersion`): an old, genuinely signed archive can't be passed
off as a new version.

## Windows and Linux

Windows comes after the Mac (Stage 6). `tauri.windows.conf.json` already
puts the engine beside the app, as a sidecar staged at
`app/src-tauri/helpers/wwav-engine-x86_64-pc-windows-msvc.exe`. On Linux the
`.deb` puts it at `/usr/bin/wwav-engine` from `app/src-tauri/helpers/`, with
the MIME types for `.wwav`, `.swav` and `.wwavsession`.
