use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

#[test]
fn cargo_injected_argument_initializes_one_package_at_the_cli_version() {
    let fixture = CargoFixture::new(&[("app", "Cargo.toml")]);

    fixture
        .command()
        .arg("provenance")
        .arg("init")
        .assert()
        .success();

    assert_eq!(fixture.scope_path_prefix(), ".");
    assert_eq!(fixture.gitignore(), ".provenance/cache/\n");
    assert!(fixture.root().join("AGENTS.md").exists());
    assert!(fixture
        .root()
        .join(".agents/skills/provenance-shaping/SKILL.md")
        .exists());
    assert!(fixture
        .cargo_calls()
        .contains("add provenance-sdk@0.2.2 --package app --exact"));
}

#[test]
fn initializer_is_idempotent() {
    let fixture = CargoFixture::new(&[("app", "Cargo.toml")]);
    fixture
        .command()
        .arg("provenance")
        .arg("init")
        .assert()
        .success();
    let manifest = fixture.manifest();
    let cargo_manifest = fixture.cargo_manifest();
    let cargo_lock = fixture.cargo_lock();

    fixture
        .command()
        .arg("provenance")
        .arg("init")
        .assert()
        .success();

    assert_eq!(fixture.manifest(), manifest);
    assert_eq!(fixture.gitignore(), ".provenance/cache/\n");
    assert_eq!(fixture.cargo_manifest(), cargo_manifest);
    assert_eq!(fixture.cargo_lock(), cargo_lock);
    assert_eq!(
        fixture
            .cargo_calls()
            .lines()
            .filter(|call| call.starts_with("add "))
            .count(),
        1
    );
}

#[test]
fn cargo_and_plain_init_write_identical_onboarding_files() {
    let fixture = CargoFixture::new(&[("app", "Cargo.toml")]);
    fixture
        .command()
        .args(["provenance", "init"])
        .assert()
        .success();
    let plain = fixture.temporary.path().join("plain");
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "init",
            "--path",
            plain.to_str().unwrap(),
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();

    for relative in [
        ".provenance/state/manifest.json",
        ".gitignore",
        "AGENTS.md",
        ".agents/skills/provenance-fork-tournament/SKILL.md",
        ".agents/skills/provenance-grounded-writing/SKILL.md",
        ".agents/skills/provenance-shaping/SKILL.md",
        ".agents/skills/provenance-swarm-backtrace/SKILL.md",
    ] {
        assert_eq!(
            std::fs::read(fixture.root().join(relative)).unwrap(),
            std::fs::read(plain.join(relative)).unwrap(),
            "onboarding output differs at {relative}"
        );
    }
}

#[test]
fn ambiguous_workspace_requires_a_package() {
    let fixture = CargoFixture::new(&[
        ("api", "crates/api/Cargo.toml"),
        ("worker", "crates/worker/Cargo.toml"),
    ]);

    fixture
        .command()
        .arg("provenance")
        .arg("init")
        .assert()
        .failure()
        .stderr(predicate::str::contains("more than one eligible package"))
        .stderr(predicate::str::contains("--package"))
        .stderr(predicate::str::contains("api"))
        .stderr(predicate::str::contains("worker"));

    assert!(!fixture.root().join(".provenance").exists());
    assert!(!fixture.cargo_calls().contains("add "));
}

#[test]
fn explicit_package_selects_its_manifest_directory_as_the_path_prefix() {
    let fixture = CargoFixture::new(&[
        ("api", "crates/api/Cargo.toml"),
        ("worker", "crates/worker/Cargo.toml"),
    ]);

    fixture
        .command()
        .args(["provenance", "init", "--package", "api"])
        .assert()
        .success();

    assert_eq!(fixture.scope_path_prefix(), "crates/api");
    assert!(fixture
        .cargo_calls()
        .contains("add provenance-sdk@0.2.2 --package api --exact"));
}

#[test]
fn rerun_refuses_to_change_the_existing_target_package() {
    let fixture = CargoFixture::new(&[
        ("api", "crates/api/Cargo.toml"),
        ("worker", "crates/worker/Cargo.toml"),
    ]);
    fixture
        .command()
        .args(["provenance", "init", "--package", "api"])
        .assert()
        .success();

    fixture
        .command()
        .args(["provenance", "init", "--package", "worker"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already targets Cargo package"))
        .stderr(predicate::str::contains("api"))
        .stderr(predicate::str::contains("worker"));

    assert_eq!(fixture.scope_path_prefix(), "crates/api");
    assert!(!fixture
        .cargo_calls()
        .contains("add provenance-sdk@0.2.2 --package worker --exact"));
}

#[test]
fn rerun_accepts_an_equivalent_existing_path_prefix() {
    let fixture = CargoFixture::new(&[("api", "crates/api/Cargo.toml")]);
    fixture
        .command()
        .args(["provenance", "init"])
        .assert()
        .success();
    let mut manifest = fixture.manifest();
    manifest["scopes"][0]["path_prefix"] = json!("./crates/api");
    std::fs::write(
        fixture.root().join(".provenance/state/manifest.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();

    fixture
        .command()
        .args(["provenance", "init"])
        .assert()
        .success();

    assert_eq!(fixture.scope_path_prefix(), "crates/api");
}

struct CargoFixture {
    temporary: tempfile::TempDir,
    workspace_root: PathBuf,
    fake_bin: PathBuf,
    call_log: PathBuf,
    cargo_manifest: PathBuf,
}

impl CargoFixture {
    fn new(packages: &[(&str, &str)]) -> Self {
        let temporary = tempfile::tempdir().expect("create fixture directory");
        let workspace_root = temporary.path().join("workspace");
        let fake_bin = temporary.path().join("bin");
        std::fs::create_dir_all(workspace_root.join("nested")).unwrap();
        std::fs::create_dir_all(&fake_bin).unwrap();

        let metadata_packages: Vec<_> = packages
            .iter()
            .map(|(name, manifest)| {
                let manifest_path = workspace_root.join(manifest);
                std::fs::create_dir_all(manifest_path.parent().unwrap()).unwrap();
                std::fs::write(
                    &manifest_path,
                    format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\n"),
                )
                .unwrap();
                json!({
                    "id": format!("path+file://fixture/{name}#0.1.0"),
                    "name": name,
                    "manifest_path": manifest_path,
                    "dependencies": [],
                })
            })
            .collect();
        let cargo_manifest = workspace_root.join(packages[0].1);
        let metadata = json!({
            "workspace_root": workspace_root,
            "workspace_members": metadata_packages.iter().map(|package| package["id"].clone()).collect::<Vec<_>>(),
            "packages": metadata_packages,
        });
        let metadata_path = temporary.path().join("metadata.json");
        std::fs::write(&metadata_path, serde_json::to_vec(&metadata).unwrap()).unwrap();

        let call_log = temporary.path().join("cargo-calls.log");
        let helper_source = temporary.path().join("fake-cargo.rs");
        std::fs::write(
            &helper_source,
            r#"
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;

fn main() {
    let arguments: Vec<_> = env::args().skip(1).collect();
    let mut log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(env::var_os("FAKE_CARGO_CALL_LOG").unwrap())
        .unwrap();
    writeln!(log, "{}", arguments.join(" ")).unwrap();
    match arguments.first().map(String::as_str) {
        Some("metadata") => {
            let mut metadata = fs::read_to_string(env::var_os("FAKE_CARGO_METADATA").unwrap()).unwrap();
            if std::path::Path::new(&env::var_os("FAKE_CARGO_DEPENDENCY_STATE").unwrap()).exists() {
                metadata = metadata.replace(
                    "\"dependencies\":[]",
                    "\"dependencies\":[{\"name\":\"provenance-sdk\",\"req\":\"=0.2.2\"}]",
                );
            }
            print!("{metadata}");
        }
        Some("add") => {
            let manifest_path = env::var_os("FAKE_CARGO_MANIFEST").unwrap();
            let mut manifest = fs::read_to_string(&manifest_path).unwrap();
            if !manifest.contains("provenance-sdk") {
                manifest.push_str("\n[dependencies]\nprovenance-sdk = \"=0.2.2\"\n");
                fs::write(manifest_path, manifest).unwrap();
            }
            fs::write(env::var_os("FAKE_CARGO_LOCK").unwrap(), "version = 4\n").unwrap();
            fs::write(env::var_os("FAKE_CARGO_DEPENDENCY_STATE").unwrap(), "installed\n").unwrap();
        }
        _ => std::process::exit(64),
    }
}
"#,
        )
        .unwrap();
        let cargo = fake_bin.join(format!("cargo{}", std::env::consts::EXE_SUFFIX));
        let output =
            std::process::Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
                .args(["--edition=2021", "-o"])
                .arg(&cargo)
                .arg(&helper_source)
                .output()
                .expect("compile fake cargo");
        assert!(
            output.status.success(),
            "fake cargo did not compile: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        Self {
            temporary,
            workspace_root,
            fake_bin,
            call_log,
            cargo_manifest,
        }
    }

    fn command(&self) -> Command {
        let mut command = Command::cargo_bin("cargo-provenance").unwrap();
        let path = std::env::join_paths(std::iter::once(self.fake_bin.clone()).chain(
            std::env::split_paths(&std::env::var_os("PATH").expect("PATH is set")),
        ))
        .unwrap();
        command
            .current_dir(self.workspace_root.join("nested"))
            .env("PATH", path)
            .env(
                "FAKE_CARGO_METADATA",
                self.temporary.path().join("metadata.json"),
            )
            .env("FAKE_CARGO_CALL_LOG", &self.call_log)
            .env("FAKE_CARGO_MANIFEST", &self.cargo_manifest)
            .env("FAKE_CARGO_LOCK", self.workspace_root.join("Cargo.lock"))
            .env(
                "FAKE_CARGO_DEPENDENCY_STATE",
                self.temporary.path().join("dependency-installed"),
            );
        command
    }

    fn root(&self) -> &Path {
        &self.workspace_root
    }

    fn manifest(&self) -> Value {
        serde_json::from_slice(
            &std::fs::read(self.root().join(".provenance/state/manifest.json")).unwrap(),
        )
        .unwrap()
    }

    fn scope_path_prefix(&self) -> String {
        self.manifest()["scopes"][0]["path_prefix"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    fn gitignore(&self) -> String {
        std::fs::read_to_string(self.root().join(".gitignore")).unwrap()
    }

    fn cargo_calls(&self) -> String {
        std::fs::read_to_string(&self.call_log).unwrap_or_default()
    }

    fn cargo_manifest(&self) -> String {
        std::fs::read_to_string(&self.cargo_manifest).unwrap()
    }

    fn cargo_lock(&self) -> String {
        std::fs::read_to_string(self.root().join("Cargo.lock")).unwrap()
    }
}
