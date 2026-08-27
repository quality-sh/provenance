// @provenance rule: rule_cargo_install_prints_init_step
fn main() {
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
