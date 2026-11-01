#[path = "build/review_assets.rs"]
mod review_assets;

/// Embeds the review page bundle. `PROVENANCE_REVIEW_ASSETS_DIR` names an explicit
/// asset directory; otherwise the composed `review-assets-generated` directory is
/// used when it exists, and the committed unavailable page when it does not.
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
}
