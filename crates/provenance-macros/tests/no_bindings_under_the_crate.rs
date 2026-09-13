//! The strict Rule Coverage command scans `crates/` and fails the build on
//! any binding citing a Rule the graph does not know. This crate carries no
//! repository bindings: its compile-path fixtures live in
//! `fixtures/provenance-macros/`, where their demonstration rule id cannot
//! enter the scan.

#[test]
fn the_macro_crate_carries_no_rule_bindings() {
    let manifest = camino::Utf8Path::from_path(std::path::Path::new(env!("CARGO_MANIFEST_DIR")))
        .expect("manifest path is UTF-8");
    let scans = provenance_scanner::scan_path(manifest).expect("crate scan");
    let bindings: Vec<_> = scans
        .into_iter()
        .filter(|file| !file.bindings.is_empty())
        .map(|file| (file.file_path.clone(), file.bindings))
        .collect();
    assert!(
        bindings.is_empty(),
        "crates/provenance-macros must carry no #[rule]/#[verifies] bindings; \
         marker-technology fixtures belong in fixtures/provenance-macros/: {bindings:?}"
    );
}
