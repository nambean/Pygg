fn main() {
    cli_version();
    // The window's build steps (icons, capabilities, the Windows resource) only with the `app` feature; the console
    // programs build without them (Cargo.toml, ADR-037).
    #[cfg(feature = "app")]
    tauri_build::build()
}

/// `pygg-cli` is published on npm, whose versions carry a pre-release tag (`0.1.0-dev.1`) that the crate's own version
/// cannot: the Windows installer takes numbers only. So `pygg --version` reports the npm launcher's version, read from
/// its package.json, and falls back to the crate's when the file is not there (a crate built outside this repository).
fn cli_version() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../packages/npm/pygg/package.json");
    println!("cargo:rerun-if-changed={}", manifest.display());
    let version = std::fs::read_to_string(&manifest)
        .ok()
        .and_then(|text| json_string(&text, "version"))
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());
    println!("cargo:rustc-env=PYGG_CLI_VERSION={version}");
}

/// The first top-level `"key": "value"` string in a small JSON file, without a JSON parser in the build script.
fn json_string(text: &str, key: &str) -> Option<String> {
    let rest = &text[text.find(&format!("\"{key}\""))? + key.len() + 2..];
    let rest = rest.trim_start().strip_prefix(':')?.trim_start().strip_prefix('"')?;
    let value = &rest[..rest.find('"')?];
    let plain = value.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+'));
    (plain && !value.is_empty()).then(|| value.to_string())
}
