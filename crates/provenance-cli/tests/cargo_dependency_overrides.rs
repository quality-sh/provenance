use assert_cmd::Command;
use predicates::prelude::*;
use provenance_macros::verifies;
use sha2::{Digest, Sha256};
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
#[verifies("rule_cargo_init_adds_exact_sdk", examples)]
fn missing_dependency_is_added_with_an_exact_requirement_by_real_cargo() {
    let fixture = CargoProject::new("0.2.2");
    fixture.write_missing_dependency();

    fixture.init().success();

    let manifest = String::from_utf8(fixture.manifest()).unwrap();
    assert!(
        manifest.contains("provenance-sdk = \"=0.2.2\""),
        "{manifest}"
    );
}

#[test]
#[verifies("rule_cargo_init_preserves_sdk_dependency", examples)]
fn onboarding_conflict_leaves_missing_dependency_and_repository_state_unchanged() {
    let fixture = CargoProject::new("0.2.2");
    fixture.write_missing_dependency();
    let skill = fixture
        .root
        .join(".agents/skills/provenance-shaping/SKILL.md");
    std::fs::create_dir_all(skill.parent().unwrap()).unwrap();
    std::fs::write(&skill, "user-owned skill\n").unwrap();
    let manifest = fixture.manifest();

    fixture
        .init()
        .failure()
        .stderr(predicate::str::contains("exists and differs"));

    assert_eq!(fixture.manifest(), manifest);
    assert_eq!(
        std::fs::read_to_string(skill).unwrap(),
        "user-owned skill\n"
    );
    assert!(!fixture.root.join("Cargo.lock").exists());
    assert!(!fixture.root.join(".provenance").exists());
    assert!(!fixture.root.join(".claude").exists());
    assert!(!fixture.root.join("AGENTS.md").exists());
    assert!(!fixture.root.join(".gitignore").exists());
}

#[test]
#[verifies("rule_init_validates_planned_repository", examples)]
fn planned_repository_validation_failure_precedes_cargo_and_onboarding_writes() {
    let fixture = CargoProject::new("0.2.2");
    fixture.write_missing_dependency();
    let manifest = fixture.manifest();
    let original_scope = fixture.root.join(".provenance/state/scopes/unexpected");
    std::fs::create_dir_all(&original_scope).unwrap();

    fixture
        .init()
        .failure()
        .stderr(predicate::str::contains("scope directory unexpected"));

    assert_eq!(fixture.manifest(), manifest);
    assert!(!fixture.root.join("Cargo.lock").exists());
    assert!(original_scope.is_dir());
    assert!(!fixture.root.join(".provenance/manifest.json").exists());
    assert!(!fixture.root.join(".provenance/cache").exists());
    assert!(!fixture.root.join(".agents").exists());
    assert!(!fixture.root.join(".claude").exists());
    assert!(!fixture.root.join("AGENTS.md").exists());
    assert!(!fixture.root.join(".gitignore").exists());
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

    fn write_missing_dependency(&self) {
        self.write_manifest("");
        let registry = self.root.join("registry");
        let archive = b"fixture crate archive";
        std::fs::create_dir_all(registry.join("index/pr/ov")).unwrap();
        std::fs::write(registry.join("provenance-sdk-0.2.2.crate"), archive).unwrap();
        std::fs::write(
            registry.join("index/pr/ov/provenance-sdk"),
            format!(
                "{{\"name\":\"provenance-sdk\",\"vers\":\"0.2.2\",\"deps\":[],\"cksum\":\"{:x}\",\"features\":{{}},\"yanked\":false}}\n",
                Sha256::digest(archive)
            ),
        )
        .unwrap();
        std::fs::create_dir_all(self.root.join(".cargo")).unwrap();
        std::fs::write(
            self.root.join(".cargo/config.toml"),
            format!(
                "[source.crates-io]\nreplace-with = \"fixture\"\n\n[source.fixture]\nlocal-registry = \"{}\"\n",
                cargo_path(&registry)
            ),
        )
        .unwrap();
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
