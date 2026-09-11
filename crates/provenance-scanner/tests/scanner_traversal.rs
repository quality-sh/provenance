use camino::Utf8Path;
use provenance_scanner::scan_path;

fn write_source(root: &Utf8Path, relative: &str) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, "fn scanned() {}\n").unwrap();
}

fn scanned_relative_paths(root: &Utf8Path) -> Vec<String> {
    scan_path(root)
        .unwrap()
        .into_iter()
        .map(|scan| {
            scan.file_path
                .strip_prefix(root)
                .unwrap()
                .components()
                .map(|component| component.as_str())
                .collect::<Vec<_>>()
                .join("/")
        })
        .collect()
}

#[test]
fn repository_scan_excludes_dependency_build_and_metadata_trees() {
    let temp = tempfile::tempdir().unwrap();
    let root = Utf8Path::from_path(temp.path()).unwrap();
    write_source(root, "src/application.rs");
    write_source(root, "node_modules/package/dependency.rs");
    write_source(root, "target/debug/generated.rs");
    write_source(root, ".git/hooks/metadata.rs");

    assert_eq!(scanned_relative_paths(root), ["src/application.rs"]);
}

#[test]
fn repository_scan_excludes_nested_dependency_boundaries() {
    let temp = tempfile::tempdir().unwrap();
    let root = Utf8Path::from_path(temp.path()).unwrap();
    write_source(root, "packages/web/src/application.ts");
    write_source(root, "packages/web/node_modules/package/dependency.ts");
    write_source(root, "packages/api/target/generated.rs");

    assert_eq!(
        scanned_relative_paths(root),
        ["packages/web/src/application.ts"]
    );
}

#[test]
fn similarly_named_directories_remain_scannable() {
    let temp = tempfile::tempdir().unwrap();
    let root = Utf8Path::from_path(temp.path()).unwrap();
    write_source(root, "node_modules_cache/cached.js");
    write_source(root, "targeting/domain.rs");
    write_source(root, ".github/workflows/check.ts");

    assert_eq!(
        scanned_relative_paths(root),
        [
            ".github/workflows/check.ts",
            "node_modules_cache/cached.js",
            "targeting/domain.rs",
        ]
    );
}

#[test]
fn explicitly_selected_excluded_directory_remains_scannable() {
    let temp = tempfile::tempdir().unwrap();
    let root = Utf8Path::from_path(temp.path()).unwrap();
    let dependency_root = root.join("node_modules");
    let build_root = root.join("target");
    write_source(root, "node_modules/package/dependency.js");
    write_source(root, "target/debug/generated.rs");

    assert_eq!(
        scanned_relative_paths(&dependency_root),
        ["package/dependency.js"]
    );
    assert_eq!(scanned_relative_paths(&build_root), ["debug/generated.rs"]);
}

const GENERATED_ANNOTATION: &str =
    "// @provenance rule: rule_generated_output\nexport function generated() {}\n";

#[test]
fn repository_scan_excludes_ignored_generated_output() {
    let temp = tempfile::tempdir().unwrap();
    let root = Utf8Path::from_path(temp.path()).unwrap();
    write_source(root, "src/application.ts");
    std::fs::write(root.join(".gitignore"), "dist\n.test-dist\n").unwrap();
    std::fs::create_dir_all(root.join("dist")).unwrap();
    std::fs::write(root.join("dist/generated.js"), GENERATED_ANNOTATION).unwrap();
    std::fs::create_dir_all(root.join(".test-dist")).unwrap();
    std::fs::write(root.join(".test-dist/generated.js"), GENERATED_ANNOTATION).unwrap();

    assert_eq!(
        scanned_relative_paths(root),
        ["src/application.ts"],
        "ignored output trees never enter a repository scan"
    );
}

#[test]
fn similarly_named_unignored_output_directories_remain_scannable() {
    let temp = tempfile::tempdir().unwrap();
    let root = Utf8Path::from_path(temp.path()).unwrap();
    std::fs::write(root.join(".gitignore"), "build/\n").unwrap();
    write_source(root, "distribution/hand-written.ts");
    write_source(root, "dist/source.ts");
    write_source(root, "build/generated.rs");

    assert_eq!(
        scanned_relative_paths(root),
        ["dist/source.ts", "distribution/hand-written.ts"],
        "a basename alone is not proof that legitimate source is generated"
    );
}

#[test]
fn anchored_and_glob_ignore_patterns_prune_output_trees() {
    let temp = tempfile::tempdir().unwrap();
    let root = Utf8Path::from_path(temp.path()).unwrap();
    std::fs::write(root.join(".gitignore"), "/dist\n**/out\n*.cache/\n").unwrap();
    write_source(root, "src/application.rs");
    write_source(root, "dist/generated.js");
    write_source(root, "packages/web/src/page.ts");
    write_source(root, "packages/web/out/bundle.js");
    write_source(root, "docs/site.source.ts");
    write_source(root, "notes.cache/kept.ts");

    assert_eq!(
        scanned_relative_paths(root),
        [
            "docs/site.source.ts",
            "packages/web/src/page.ts",
            "src/application.rs",
        ]
    );
}

#[test]
fn negated_ignore_pattern_keeps_the_directory() {
    let temp = tempfile::tempdir().unwrap();
    let root = Utf8Path::from_path(temp.path()).unwrap();
    std::fs::write(root.join(".gitignore"), "tmp*\n!tmp_fixtures\n").unwrap();
    write_source(root, "tmp_scratch/generated.js");
    write_source(root, "tmp_fixtures/fixture.ts");

    assert_eq!(scanned_relative_paths(root), ["tmp_fixtures/fixture.ts"]);
}

#[test]
fn nested_gitignore_prunes_only_its_own_tree() {
    let temp = tempfile::tempdir().unwrap();
    let root = Utf8Path::from_path(temp.path()).unwrap();
    std::fs::create_dir_all(root.join("packages/web")).unwrap();
    std::fs::write(root.join("packages/web/.gitignore"), "dist/\n").unwrap();
    write_source(root, "packages/web/dist/generated.js");
    write_source(root, "packages/api/dist/hand-written.ts");

    assert_eq!(
        scanned_relative_paths(root),
        ["packages/api/dist/hand-written.ts"]
    );
}

#[test]
fn explicitly_requested_ignored_output_remains_scannable() {
    let temp = tempfile::tempdir().unwrap();
    let root = Utf8Path::from_path(temp.path()).unwrap();
    std::fs::write(root.join(".gitignore"), "dist\n").unwrap();
    write_source(root, "dist/generated.js");

    assert_eq!(
        scanned_relative_paths(&root.join("dist")),
        ["generated.js"],
        "an explicitly requested path overrides repository ignore rules"
    );
}
