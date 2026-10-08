# Local Mac Build

From the repository root:

```sh
npm --prefix app/ui ci
bash tools/build-local-macos.sh
```

Needs Rust 1.88 or newer, Node, Xcode command-line tools and Tauri CLI 2.
The native-architecture debug bundle is ad-hoc signed, not notarized, at
`~/Library/Developer/wi-wwav-build/target-space/debug/bundle/macos/Wi_WWAV.app`.
`WI_WWAV_BUILD_DIR` and `CARGO_TARGET_DIR` can relocate its build artifacts.
This command does not install or replace an app, clear a Library, or quit one.

It bundles the new Rust engine, Learn MCP server and OCR helper. The old JUCE
plugin scanner is not built or shipped in this local build. The production
release script is still the legacy JUCE/universal/notarization workflow and
needs a separate migration before distribution.

The app normally opens `~/Music/Wi_WWAV`. A separate development instance may
use both `WI_WWAV_BESIDE=1` and `WI_WWAV_LIBRARY=/path/to/test-library`; neither
is needed for ordinary testing. Quit the old app before opening a new bundle
on the same Library, so the single-instance rule cannot redirect to an older
binary.
