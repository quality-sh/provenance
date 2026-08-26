use assert_cmd::Command;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub struct CargoFixture {
    pub temporary: tempfile::TempDir,
    workspace_root: PathBuf,
    fake_bin: PathBuf,
    call_log: PathBuf,
    pub cargo_manifest: PathBuf,
}

impl CargoFixture {
    #[allow(clippy::needless_raw_string_hashes, clippy::too_many_lines)]
    pub fn new(packages: &[(&str, &str)]) -> Self {
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
            r##"
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::process::Command;

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
            let workspace_path = env::var_os("FAKE_CARGO_WORKSPACE").unwrap();
            let workspace = std::path::Path::new(&workspace_path);
            let mut manifest = fs::read_to_string(&manifest_path).unwrap();
            if !manifest.contains("provenance-sdk") {
                manifest.push_str("\n[dependencies]\nprovenance-sdk = \"=0.2.2\"\n");
                fs::write(&manifest_path, manifest).unwrap();
            }
            fs::write(env::var_os("FAKE_CARGO_LOCK").unwrap(), "version = 4\n").unwrap();
            fs::write(env::var_os("FAKE_CARGO_DEPENDENCY_STATE").unwrap(), "installed\n").unwrap();
            if env::var_os("FAKE_CARGO_BREAK_LOCK_OBSERVATION").is_some() {
                let lock = env::var_os("FAKE_CARGO_LOCK").unwrap();
                fs::remove_file(&lock).unwrap();
                fs::create_dir(&lock).unwrap();
            }
            if env::var_os("FAKE_CARGO_FAIL_AFTER_WRITE").is_some() {
                std::process::exit(42);
            }
            if env::var_os("FAKE_CARGO_STALE_INIT_PLAN").is_some() {
                fs::write(workspace.join("AGENTS.md"), "concurrent instructions\n").unwrap();
            }
            if env::var_os("FAKE_CARGO_ROLLBACK_SECOND_RESTORE_FAIL").is_some() {
                fs::write(workspace.join("AGENTS.md"), "concurrent instructions\n").unwrap();
                assert!(Command::new("chmod").args(["555"]).arg(workspace).status().unwrap().success());
            }
            if env::var_os("FAKE_CARGO_ROLLBACK_FIRST_RESTORE_FAIL").is_some() {
                fs::write(workspace.join("AGENTS.md"), "concurrent instructions\n").unwrap();
                let package = std::path::Path::new(&manifest_path).parent().unwrap();
                assert!(Command::new("chmod").args(["555"]).arg(package).status().unwrap().success());
            }
        }
        _ => std::process::exit(64),
    }
}
"##,
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

    pub fn command(&self) -> Command {
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
            .env("FAKE_CARGO_WORKSPACE", &self.workspace_root)
            .env(
                "FAKE_CARGO_DEPENDENCY_STATE",
                self.temporary.path().join("dependency-installed"),
            );
        command
    }

    pub fn root(&self) -> &Path {
        &self.workspace_root
    }

    pub fn manifest(&self) -> Value {
        serde_json::from_slice(
            &std::fs::read(self.root().join(".provenance/state/manifest.json")).unwrap(),
        )
        .unwrap()
    }

    pub fn scope_path_prefix(&self) -> String {
        self.manifest()["scopes"][0]["path_prefix"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    pub fn gitignore(&self) -> String {
        std::fs::read_to_string(self.root().join(".gitignore")).unwrap()
    }

    pub fn cargo_calls(&self) -> String {
        std::fs::read_to_string(&self.call_log).unwrap_or_default()
    }

    pub fn cargo_manifest(&self) -> String {
        std::fs::read_to_string(&self.cargo_manifest).unwrap()
    }

    pub fn cargo_lock(&self) -> String {
        std::fs::read_to_string(self.root().join("Cargo.lock")).unwrap()
    }
}
