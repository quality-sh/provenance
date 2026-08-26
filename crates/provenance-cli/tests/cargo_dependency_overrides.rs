use assert_cmd::Command;
use predicates::prelude::*;
use provenance_macros::verifies;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;

#[test]
#[verifies("rule_cargo_init_preserves_sdk_dependency", examples)]
fn registry_requirements_and_patch_leave_the_manifest_and_lock_unchanged() {
    for requirement in ["=0.2.2", "^0.2.0"] {
        let fixture = CargoProject::new("0.2.2");
        fixture.write_registry_dependency(requirement);
        fixture.generate_lockfile();
        let manifest = fixture.manifest();
        let lockfile = fixture.lockfile();

        fixture.init().success();

        assert_eq!(fixture.manifest(), manifest);
        assert_eq!(fixture.lockfile(), lockfile);
    }
}

#[test]
#[verifies("rule_cargo_init_preserves_sdk_dependency", examples)]
fn path_dependency_is_preserved_without_creating_a_lockfile() {
    let fixture = CargoProject::new("0.2.2");
    fixture.write_path_dependency();
    let manifest = fixture.manifest();

    fixture.init().success();

    assert_eq!(fixture.manifest(), manifest);
    assert!(!fixture.root.join("Cargo.lock").exists());
}

#[test]
#[verifies("rule_cargo_init_preserves_sdk_dependency", examples)]
fn git_dependency_is_preserved_without_creating_a_lockfile() {
    let fixture = CargoProject::new("0.2.2");
    fixture.commit_sdk();
    fixture.write_git_dependency();
    let manifest = fixture.manifest();

    fixture.init().success();

    assert_eq!(fixture.manifest(), manifest);
    assert!(!fixture.root.join("Cargo.lock").exists());
}

#[test]
#[verifies("rule_cargo_init_preserves_sdk_dependency", examples)]
fn incompatible_registry_requirement_fails_without_any_repository_mutation() {
    let fixture = CargoProject::new("0.1.9");
    fixture.write_registry_dependency("=0.1.9");
    let manifest = fixture.manifest();

    fixture
        .init()
        .failure()
        .stderr(predicate::str::contains("requires provenance-sdk =0.1.9"))
        .stderr(predicate::str::contains(
            "not compatible with provenance-cli 0.2.2",
        ));

    assert_eq!(fixture.manifest(), manifest);
    assert!(!fixture.root.join("Cargo.lock").exists());
    assert!(!fixture.root.join(".provenance").exists());
}

struct CargoProject {
    _temporary: tempfile::TempDir,
    root: PathBuf,
    sdk: PathBuf,
}

impl CargoProject {
    fn new(sdk_version: &str) -> Self {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("app");
        let sdk = temporary.path().join("provenance-sdk");
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::create_dir_all(sdk.join("src")).unwrap();
        std::fs::write(root.join("src/lib.rs"), "pub fn app() {}\n").unwrap();
        std::fs::write(sdk.join("src/lib.rs"), "pub fn sdk() {}\n").unwrap();
        std::fs::write(
            sdk.join("Cargo.toml"),
            format!(
                "[package]\nname = \"provenance-sdk\"\nversion = \"{sdk_version}\"\nedition = \"2021\"\n"
            ),
        )
        .unwrap();
        Self {
            _temporary: temporary,
            root,
            sdk,
        }
    }

    fn write_registry_dependency(&self, requirement: &str) {
        self.write_manifest(&format!(
            "provenance-sdk = \"{requirement}\"\n\n[patch.crates-io]\nprovenance-sdk = {{ path = \"{}\" }}\n",
            cargo_path(&self.sdk)
        ));
    }

    fn write_path_dependency(&self) {
        self.write_manifest(&format!(
            "provenance-sdk = {{ path = \"{}\" }}\n",
            cargo_path(&self.sdk)
        ));
    }

    fn write_git_dependency(&self) {
        self.write_manifest(&format!(
            "provenance-sdk = {{ git = \"{}\" }}\n",
            file_url(&self.sdk)
        ));
    }

    fn write_manifest(&self, dependency: &str) {
        std::fs::write(
            self.root.join("Cargo.toml"),
            format!(
                "[package]\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\n{dependency}"
            ),
        )
        .unwrap();
    }

    fn commit_sdk(&self) {
        run_git(&self.sdk, &["init", "-q"]);
        run_git(&self.sdk, &["add", "."]);
        run_git(
            &self.sdk,
            &[
                "-c",
                "user.name=Provenance Test",
                "-c",
                "user.email=test@example.invalid",
                "commit",
                "-qm",
                "local sdk",
            ],
        );
    }

    fn generate_lockfile(&self) {
        let output = StdCommand::new("cargo")
            .arg("generate-lockfile")
            .current_dir(&self.root)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "generate-lockfile failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn init(&self) -> assert_cmd::assert::Assert {
        let mut command = Command::cargo_bin("cargo-provenance").unwrap();
        command
            .current_dir(&self.root)
            .args(["provenance", "init"])
            .assert()
    }

    fn manifest(&self) -> Vec<u8> {
        std::fs::read(self.root.join("Cargo.toml")).unwrap()
    }

    fn lockfile(&self) -> Vec<u8> {
        std::fs::read(self.root.join("Cargo.lock")).unwrap()
    }
}

fn cargo_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn file_url(path: &Path) -> String {
    let path = cargo_path(path);
    if path.starts_with('/') {
        format!("file://{path}")
    } else {
        format!("file:///{path}")
    }
}

fn run_git(repo: &Path, arguments: &[&str]) {
    assert!(StdCommand::new("git")
        .args(arguments)
        .current_dir(repo)
        .status()
        .unwrap()
        .success());
}
