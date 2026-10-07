fn main() {
    // Declaring the app's one command makes Tauri check it against each
    // window's capability, so a window with no capability for it (the
    // checkout) can't call it at all (docs/SPEC.md 9.8).
    let manifest = tauri_build::AppManifest::new().commands(&["core"]);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest))
        .expect("tauri-build failed");
}
