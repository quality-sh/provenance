//! Implementation aids: these process tests check distribution from one source.
//! They do not define or approve a runtime installation Rule.

#[path = "cli_dictionary/support.rs"]
#[allow(dead_code)]
mod dictionary_support;

fn run(case: &str) {
    let temporary = tempfile::tempdir().unwrap();
    let pdf = temporary.path().join("dictionary.pdf");
    std::fs::write(&pdf, dictionary_support::dictionary_pdf()).unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap().parent().unwrap();
    let status = std::process::Command::new("node")
        .arg(root.join("crates/provenance-cli/tests/support/skills_single_source/run.mjs"))
        .args([case, root.to_str().unwrap(), env!("CARGO")])
        .arg(assert_cmd::cargo::cargo_bin!("provenance"))
        .arg(&pdf)
        .status().unwrap();
    assert!(status.success(), "single-source process test failed: {case}");
}

#[test]
fn one_source_edit_reaches_all_distribution_surfaces() {
    run("distribution");
}

#[test]
fn init_leaves_prepared_checkout_clean() {
    run("clean");
}
