use assert_cmd::cargo::cargo_bin;
use provenance_macros::verifies;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

#[test]
#[verifies("rule_nested_rust_implementation_path", examples)]
fn nested_workspace_package_applies_package_relative_implementation_after_init() {
    let fixture = NestedWorkspace::new();
    let root_manifest = std::fs::read(fixture.root().join("Cargo.toml")).unwrap();
    let package_manifest = std::fs::read(fixture.root().join("packages/app/Cargo.toml")).unwrap();
    assert!(!fixture.root().join("Cargo.lock").exists());

    let status = Command::new("cargo")
        .args(["provenance", "init", "--package", "app"])
        .current_dir(fixture.root())
        .env("PATH", NestedWorkspace::path_with_cli())
        .status()
        .expect("run cargo provenance init");
    assert!(status.success());
    assert_eq!(
        std::fs::read(fixture.root().join("Cargo.toml")).unwrap(),
        root_manifest
    );
    assert_eq!(
        std::fs::read(fixture.root().join("packages/app/Cargo.toml")).unwrap(),
        package_manifest
    );
    assert!(!fixture.root().join("Cargo.lock").exists());

    let output = Command::new("cargo")
        .args(["run", "--quiet", "--package", "app"])
        .current_dir(fixture.root())
        .env("CARGO_TARGET_DIR", NestedWorkspace::target_dir())
        .output()
        .expect("build and run nested SDK consumer");
    assert!(
        output.status.success(),
        "consumer failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let binding = std::fs::read_to_string(
        fixture
            .root()
            .join(".provenance/state/scopes/default/implementations/binding.jsonl"),
    )
    .expect("implementation binding");
    let binding: Value = serde_json::from_str(binding.trim()).unwrap();
    assert_eq!(binding["file"], "packages/app/src/main.rs");
    assert_eq!(binding["symbol"], "start_workflow");
}

struct NestedWorkspace {
    _temporary: tempfile::TempDir,
    root: PathBuf,
}

impl NestedWorkspace {
    fn new() -> Self {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("workspace");
        let package = root.join("packages/app");
        std::fs::create_dir_all(package.join("src")).unwrap();
        let sdk = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../provenance-sdk")
            .to_string_lossy()
            .replace('\\', "/");
        std::fs::write(
            root.join("Cargo.toml"),
            format!(
                "[workspace]\nmembers = [\"packages/app\"]\nresolver = \"2\"\n\n[workspace.dependencies]\nprovenance-sdk = {{ path = \"{sdk}\" }}\n"
            ),
        )
        .unwrap();
        std::fs::write(
            package.join("Cargo.toml"),
            "[package]\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\nprovenance-sdk.workspace = true\n",
        )
        .unwrap();
        std::fs::write(
            package.join("src/main.rs"),
            r#"use provenance_sdk::{implemented_by, operations, requirement, rule, spec, Settings};

fn start_workflow() {}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    start_workflow();
    let settings = Settings::from_env();
    let document = spec("nested-workspace")
        .requirements([requirement("workflow")
            .statement("Accepted workflows start")
            .rules([implemented_by!(
                rule("start").statement("A workflow starts through start_workflow"),
                "src/main.rs",
                start_workflow
            )])])
        .build()?;
    operations::apply(
        settings.repository.clone(),
        &settings.scope_id()?,
        document.materialize(settings.owner),
    )?;
    Ok(())
}
"#,
        )
        .unwrap();
        Self {
            _temporary: temporary,
            root,
        }
    }

    fn root(&self) -> &Path {
        &self.root
    }

    fn path_with_cli() -> std::ffi::OsString {
        let cli = cargo_bin("cargo-provenance");
        std::env::join_paths(std::iter::once(cli.parent().unwrap().to_path_buf()).chain(
            std::env::split_paths(&std::env::var_os("PATH").expect("PATH is set")),
        ))
        .unwrap()
    }

    fn target_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/cargo-nested-sdk")
    }
}
