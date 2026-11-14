fn main() {
    // The window's build steps (icons, capabilities, the Windows resource) only with the `app` feature; the console
    // programs build without them (Cargo.toml, ADR-037).
    #[cfg(feature = "app")]
    tauri_build::build()
}
