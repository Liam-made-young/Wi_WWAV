"""Checks the Mac release's configuration and script without a Mac (S0.7's
preparation, docs/PLAN.md). Signing itself stays open until macos.sh runs on
a Mac with the Developer ID.

    python3 -m unittest discover -s tools/release -v

What a fail looks like, written before the checks:

- A plist or entitlements file isn't a well-formed property list (what
  plutil -lint refuses), or an entitlement is missing, extra, or relaxes
  library validation in the app.
- The engine's Info.plist isn't a faceless helper (LSUIElement), or the
  bundle doesn't put it in Contents/Helpers.
- The app's document types disagree with tauri.conf.json's file
  associations, or a session isn't a package.
- macos.sh has a syntax error; builds outside ~/Library/Developer/
  wi-wwav-build; isn't universal; signs before xattr -cr; signs the app
  before its helpers, without the hardened runtime, or with --deep; staples
  before notarytool finishes; skips spctl --assess or codesign --verify
  --deep --strict; or signs anything when a helper isn't universal.
"""

import json
import os
import plistlib
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
APP_SRC = ROOT / "app/src-tauri"
MACOS = ROOT / "engine/macos"

PLISTS = [
    APP_SRC / "Info.plist",
    APP_SRC / "entitlements.plist",
    MACOS / "Info.plist",
    MACOS / "wwav-engine.entitlements",
    MACOS / "wwav-scan.entitlements",
]


def plist(path):
    with open(path, "rb") as f:
        return plistlib.load(f, fmt=plistlib.FMT_XML)


class Plists(unittest.TestCase):
    def test_every_plist_is_well_formed(self):
        for path in PLISTS:
            with self.subTest(path.name):
                self.assertIsInstance(plist(path), dict)

    def test_the_engine_may_load_others_plugins_and_record(self):
        self.assertEqual(
            plist(MACOS / "wwav-engine.entitlements"),
            {
                "com.apple.security.cs.disable-library-validation": True,
                "com.apple.security.device.audio-input": True,
            },
        )

    def test_the_scanner_may_load_others_plugins_and_nothing_else(self):
        self.assertEqual(
            plist(MACOS / "wwav-scan.entitlements"),
            {"com.apple.security.cs.disable-library-validation": True},
        )

    def test_the_app_has_the_camera_and_keeps_library_validation(self):
        self.assertEqual(plist(APP_SRC / "entitlements.plist"), {"com.apple.security.device.camera": True})

    def test_the_engine_is_a_faceless_helper(self):
        info = plist(MACOS / "Info.plist")
        self.assertIs(info["LSUIElement"], True)
        self.assertEqual(info["CFBundleExecutable"], "wwav-engine")
        self.assertEqual(info["CFBundlePackageType"], "APPL")
        self.assertEqual(info["LSMinimumSystemVersion"], "13.0")
        self.assertIn("NSMicrophoneUsageDescription", info)

    def test_the_bundle_puts_the_helpers_in_contents_helpers(self):
        mac = json.loads((APP_SRC / "tauri.macos.conf.json").read_text())["bundle"]["macOS"]
        self.assertEqual(
            mac["files"],
            {"Helpers/wwav-engine.app": "helpers/wwav-engine.app", "Helpers/wwav-scan": "helpers/wwav-scan"},
        )
        self.assertIs(mac["hardenedRuntime"], True)
        self.assertEqual(mac["entitlements"], "entitlements.plist")
        self.assertEqual(mac["minimumSystemVersion"], "13.0")

    def test_elsewhere_the_engine_sits_beside_the_app(self):
        linux = json.loads((APP_SRC / "tauri.linux.conf.json").read_text())["bundle"]["linux"]
        self.assertEqual(linux["deb"]["files"]["/usr/bin/wwav-engine"], "helpers/wwav-engine")
        windows = json.loads((APP_SRC / "tauri.windows.conf.json").read_text())["bundle"]
        self.assertEqual(windows["externalBin"], ["helpers/wwav-engine"])

    def test_document_types_match_the_file_associations(self):
        info = plist(APP_SRC / "Info.plist")
        assoc = json.loads((APP_SRC / "tauri.conf.json").read_text())["bundle"]["fileAssociations"]
        types = {t["UTTypeIdentifier"]: t for t in info["UTExportedTypeDeclarations"]}
        docs = {d["LSItemContentTypes"][0]: d for d in info["CFBundleDocumentTypes"]}
        self.assertEqual(set(types), {a["exportedType"]["identifier"] for a in assoc})
        self.assertEqual(set(docs), set(types))
        for a in assoc:
            uti = a["exportedType"]["identifier"]
            with self.subTest(uti):
                tags = types[uti]["UTTypeTagSpecification"]
                self.assertEqual(tags["public.filename-extension"], a["ext"])
                self.assertEqual(tags["public.mime-type"], a["mimeType"])
                self.assertEqual(types[uti]["UTTypeConformsTo"], a["exportedType"]["conformsTo"])
                self.assertEqual(types[uti]["UTTypeDescription"], a["name"])
                self.assertEqual(docs[uti]["CFBundleTypeName"], a["name"])
                self.assertEqual(docs[uti]["CFBundleTypeRole"], a["role"])
                self.assertEqual(docs[uti]["LSHandlerRank"], a["rank"])

    def test_a_session_is_a_package(self):
        info = plist(APP_SRC / "Info.plist")
        doc = next(d for d in info["CFBundleDocumentTypes"] if d["LSItemContentTypes"] == ["com.mi-wwav.wwavsession"])
        self.assertIs(doc["LSTypeIsPackage"], True)
        uti = next(t for t in info["UTExportedTypeDeclarations"] if t["UTTypeIdentifier"] == "com.mi-wwav.wwavsession")
        self.assertIn("com.apple.package", uti["UTTypeConformsTo"])
        for d in info["CFBundleDocumentTypes"]:
            if d is not doc:
                self.assertNotIn("LSTypeIsPackage", d)

    def test_the_linux_mime_types_are_well_formed(self):
        subprocess.run(["xmllint", "--noout", str(APP_SRC / "linux/wi-wwav.xml")], check=True)


# ---- macos.sh, run against stand-ins for the Mac's tools ----------------------

SHIM = r"""#!/usr/bin/env bash
# Records the call, then does the little the script needs from it.
name="$(basename "$0")"
{ printf '%s' "$name"; for a in "$@"; do printf '\x1f%s' "$a"; done; printf '\n'; } >> "$SHIM_LOG"
last="${!#}"
case "$name" in
  uname) echo Darwin ;;
  lipo) echo "${SHIM_ARCHS:-x86_64 arm64}" ;;
  ninja)
    out="$2"
    mkdir -p "$out/wwav-engine_artefacts/Release/wwav-engine.app/Contents/MacOS"
    : > "$out/wwav-engine_artefacts/Release/wwav-engine.app/Contents/MacOS/wwav-engine"
    : > "$out/wwav-engine_artefacts/Release/wwav-engine.app/Contents/Info.plist"
    : > "$out/wwav-scan" ;;
  ditto)
    if [ "$1" = -c ]; then : > "$last"; else mkdir -p "$(dirname "$last")"; cp -R "$1" "$last"; fi ;;
  hdiutil) : > "$last" ;;
  cargo)
    echo "cwd=$PWD CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-}" >> "$SHIM_LOG"
    if [ "$2" = build ]; then
      app="$CARGO_TARGET_DIR/universal-apple-darwin/release/bundle/macos/Wi_WWAV.app/Contents"
      mkdir -p "$app/MacOS" "$app/Helpers"
      : > "$app/MacOS/wi-wwav"
      cp -R helpers/wwav-engine.app helpers/wwav-scan "$app/Helpers/"
    elif [ "$2" = signer ]; then
      echo SIG > "$last.sig"
    fi ;;
esac
exit 0
"""

TOOLS = ["uname", "cmake", "ninja", "npm", "cargo", "lipo", "xattr", "codesign", "xcrun", "spctl", "hdiutil", "ditto", "plutil"]


class MacosScript(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.tmp)
        # A copy of the repository's parts the script reads, so the dry run
        # touches nothing real (fetch_juce.sh here only records the call).
        repo = self.tmp / "repo"
        for rel in ["tools/release/macos.sh", "engine/macos", "app/src-tauri/entitlements.plist", "app/src-tauri/tauri.conf.json"]:
            src, dst = ROOT / rel, repo / rel
            dst.parent.mkdir(parents=True, exist_ok=True)
            (shutil.copytree if src.is_dir() else shutil.copy)(src, dst)
        conf = json.loads((repo / "app/src-tauri/tauri.conf.json").read_text())
        conf["plugins"]["updater"]["pubkey"] = "dW50cnVzdGVkIGNvbW1lbnQ6IHRlc3Q="
        (repo / "app/src-tauri/tauri.conf.json").write_text(json.dumps(conf))
        (repo / "formats/prana/core/base").mkdir(parents=True)
        (repo / "formats/prana/core/base/base.h").touch()
        (repo / "tools/fetch_juce.sh").write_text('echo "fetch_juce" >> "$SHIM_LOG"\n')
        bin_dir = self.tmp / "bin"
        bin_dir.mkdir()
        for tool in TOOLS:
            (bin_dir / tool).write_text(SHIM)
            (bin_dir / tool).chmod(0o755)
        self.repo, self.home, self.log = repo, self.tmp / "home", self.tmp / "calls"
        self.home.mkdir()
        self.env = {
            "PATH": f"{bin_dir}:{os.environ['PATH']}",
            "HOME": str(self.home),
            "SHIM_LOG": str(self.log),
            "WI_SIGNING_IDENTITY": "Developer ID Application: Test (TEAM123456)",
            "WI_NOTARY_PROFILE": "wi-notary",
            "TAURI_SIGNING_PRIVATE_KEY_PATH": "/keys/wi-wwav.key",
        }

    def run_script(self, **env):
        return subprocess.run(
            ["bash", str(self.repo / "tools/release/macos.sh")],
            env={**self.env, **env},
            capture_output=True,
            text=True,
        )

    def calls(self):
        if not self.log.exists():
            return []
        return [line.split("\x1f") for line in self.log.read_text().splitlines()]

    def index(self, calls, pred, what):
        for i, c in enumerate(calls):
            if pred(c):
                return i
        self.fail(f"never ran: {what}")

    def test_bash_and_shellcheck_accept_it(self):
        subprocess.run(["bash", "-n", str(ROOT / "tools/release/macos.sh")], check=True)
        if shutil.which("shellcheck"):
            subprocess.run(["shellcheck", str(ROOT / "tools/release/macos.sh")], check=True)

    def test_a_dry_run_builds_signs_notarizes_and_checks_in_order(self):
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stderr)
        calls = self.calls()
        build = self.home / "Library/Developer/wi-wwav-build"
        app = str(build / "target/universal-apple-darwin/release/bundle/macos/Wi_WWAV.app")
        out = build / "release/0.1.0"
        dmg = str(out / "Wi_WWAV_0.1.0_universal.dmg")

        cmake = next(c for c in calls if c[0] == "cmake")
        self.assertIn(str(build / "engine"), cmake)
        self.assertIn("-DCMAKE_OSX_ARCHITECTURES=arm64;x86_64", cmake)
        tauri = next(c for c in calls if c[:3] == ["cargo", "tauri", "build"])
        self.assertEqual(tauri[tauri.index("--target") + 1], "universal-apple-darwin")
        self.assertIn("--no-sign", tauri)
        self.assertIn(f"CARGO_TARGET_DIR={build}/target", self.log.read_text())

        signs = [c for c in calls if c[0] == "codesign" and "--sign" in c]
        self.assertEqual(
            [s[-1] for s in signs],
            [f"{app}/Contents/Helpers/wwav-scan", f"{app}/Contents/Helpers/wwav-engine.app", app, dmg],
        )
        entitlements = {s[-1]: s[s.index("--entitlements") + 1] for s in signs if "--entitlements" in s}
        self.assertTrue(entitlements[f"{app}/Contents/Helpers/wwav-scan"].endswith("engine/macos/wwav-scan.entitlements"))
        self.assertTrue(entitlements[f"{app}/Contents/Helpers/wwav-engine.app"].endswith("engine/macos/wwav-engine.entitlements"))
        self.assertTrue(entitlements[app].endswith("app/src-tauri/entitlements.plist"))
        for s in signs[:3]:
            self.assertEqual(s[s.index("--options") + 1], "runtime", s[-1])
            self.assertIn("--timestamp", s)
            self.assertNotIn("--deep", s)

        clear = self.index(calls, lambda c: c[:3] == ["xattr", "-cr", app], "xattr -cr on the app")
        first_sign = calls.index(signs[0])
        self.assertLess(clear, first_sign, "xattr -cr must come before signing")

        submit = self.index(
            calls,
            lambda c: c[:3] == ["xcrun", "notarytool", "submit"] and c[3] == dmg and "--wait" in c,
            "notarytool submit --wait on the DMG",
        )
        staple = self.index(calls, lambda c: c[:4] == ["xcrun", "stapler", "staple", dmg], "stapler staple on the DMG")
        self.assertLess(submit, staple)
        assess = self.index(calls, lambda c: c[:2] == ["spctl", "--assess"], "spctl --assess")
        self.assertLess(staple, assess)
        verify = [i for i, c in enumerate(calls) if c[0] == "codesign" and c[1:4] == ["--verify", "--deep", "--strict"]]
        self.assertTrue(verify and verify[-1] > staple, "codesign --verify --deep --strict after stapling")

        manifest = json.loads((out / "latest.json").read_text())
        self.assertEqual(manifest["version"], "0.1.0")
        for platform in ["darwin-aarch64", "darwin-x86_64"]:
            self.assertEqual(
                manifest["platforms"][platform],
                {"signature": "SIG", "url": "https://mi-wwav.com/desktop/0.1.0/Wi_WWAV.app.tar.gz"},
            )
        signer = next(c for c in calls if c[:3] == ["cargo", "tauri", "signer"])
        self.assertEqual(signer[signer.index("--app-version") + 1], "0.1.0")

    def test_a_helper_that_isnt_universal_stops_it_before_signing(self):
        result = self.run_script(SHIM_ARCHS="arm64")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("is not universal", result.stderr)
        self.assertFalse([c for c in self.calls() if c[0] == "codesign"])

    def test_it_refuses_without_an_identity(self):
        result = self.run_script(WI_SIGNING_IDENTITY="")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("WI_SIGNING_IDENTITY", result.stderr)
        self.assertEqual([c[0] for c in self.calls()], ["uname"])

    def test_it_refuses_without_the_updater_key(self):
        conf = self.repo / "app/src-tauri/tauri.conf.json"
        c = json.loads(conf.read_text())
        c["plugins"]["updater"]["pubkey"] = ""
        conf.write_text(json.dumps(c))
        result = self.run_script()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("no updater public key", result.stderr)


if __name__ == "__main__":
    unittest.main()
