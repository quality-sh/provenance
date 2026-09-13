#[path = "build/review_assets.rs"]
mod review_assets;

// @provenance rule: rule_cargo_install_prints_init_step
fn main() {
    println!("cargo:rerun-if-env-changed=PROVENANCE_REVIEW_ASSETS_DIR");
    let root = std::env::var_os("PROVENANCE_REVIEW_ASSETS_DIR").map_or_else(
        || {
            let generated = std::path::PathBuf::from("review-assets-generated");
            println!("cargo:rerun-if-changed=review-assets-generated");
            if generated.is_dir() {
                generated
            } else {
                std::path::PathBuf::from("review-assets")
            }
        },
        std::path::PathBuf::from,
    );
    let output =
        std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo output directory"));
    review_assets::generate(&root, &output.join("review_assets.rs"))
        .expect("valid review asset bundle");
    println!("cargo:warning=Next step: run cargo provenance init in your project.");

    // The generated Clap parser exceeds Windows' 1 MiB main-thread stack in
    // debug builds. Reserve the same headroom available on supported Unix
    // hosts for the full CLI, without changing the lightweight Cargo shim.
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os == "windows" && target_env == "msvc" {
        println!("cargo:rustc-link-arg-bin=provenance=/STACK:8388608");
    }
}
