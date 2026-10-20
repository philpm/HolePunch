fn main() {
    // Package the same offline locale runtime and logo used by the web console.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    for asset in ["i18n.js", "locales/catalogs.js", "logo.png"] {
        let source = root.join("web").join(asset);
        let target = root.join("apps/desktop/ui").join(asset);
        println!("cargo:rerun-if-changed={}", source.display());
        std::fs::create_dir_all(target.parent().unwrap()).expect("create shared asset directory");
        std::fs::copy(source, target).expect("copy shared UI asset");
    }
    tauri_build::build()
}
