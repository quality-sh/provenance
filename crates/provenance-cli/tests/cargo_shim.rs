use assert_cmd::Command;
use std::path::{Path, PathBuf};

#[test]
fn direct_version_flag_names_the_cargo_subcommand_binary() {
    let expected = format!("cargo-provenance {}\n", env!("CARGO_PKG_VERSION"));

    for flag in ["--version", "-V"] {
        Command::cargo_bin("cargo-provenance")
            .unwrap()
            .arg(flag)
            .assert()
            .success()
            .stdout(expected.clone());
    }
}

#[test]
fn shim_forwards_to_the_sibling_provenance_and_preserves_exit_code() {
    let fixture = ShimFixture::new(true);

    fixture
        .command()
        .args(["provenance", "init", "--package", "api"])
        .assert()
        .code(23);

    assert_eq!(
        fixture.forwarded_arguments(),
        "__cargo-init --package api\n"
    );
}

#[test]
fn shim_refuses_a_path_binary_when_its_sibling_is_missing() {
    let fixture = ShimFixture::new(false);

    fixture
        .command()
        .args(["provenance", "init"])
        .assert()
        .code(1)
        .stderr(predicates::str::contains(
            "matching sibling provenance executable is missing",
        ));

    assert!(!fixture.call_log.exists());
}

struct ShimFixture {
    _temporary: tempfile::TempDir,
    shim: PathBuf,
    path_bin: PathBuf,
    call_log: PathBuf,
}

impl ShimFixture {
    fn new(sibling: bool) -> Self {
        let temporary = tempfile::tempdir().expect("create shim fixture");
        let shim_bin = temporary.path().join("shim-bin");
        let path_bin = temporary.path().join("path-bin");
        std::fs::create_dir_all(&shim_bin).unwrap();
        std::fs::create_dir_all(&path_bin).unwrap();

        let shim = shim_bin.join(executable("cargo-provenance"));
        std::fs::copy(assert_cmd::cargo::cargo_bin!("cargo-provenance"), &shim).unwrap();
        compile_fake_provenance(
            if sibling { &shim_bin } else { &path_bin },
            temporary.path(),
        );

        Self {
            _temporary: temporary,
            shim,
            path_bin,
            call_log: shim_bin.join("forwarded-arguments.log"),
        }
    }

    fn command(&self) -> Command {
        let path = std::env::join_paths(std::iter::once(self.path_bin.clone()).chain(
            std::env::split_paths(&std::env::var_os("PATH").expect("PATH is set")),
        ))
        .unwrap();
        let mut command = Command::new(&self.shim);
        command
            .env("PATH", path)
            .env("SHIM_CALL_LOG", &self.call_log);
        command
    }

    fn forwarded_arguments(&self) -> String {
        std::fs::read_to_string(&self.call_log).unwrap()
    }
}

fn compile_fake_provenance(output_directory: &Path, temporary: &Path) {
    let source = temporary.join(format!(
        "fake-provenance-{}.rs",
        output_directory.file_name().unwrap().to_string_lossy()
    ));
    std::fs::write(
        &source,
        r#"
use std::env;
use std::fs::OpenOptions;
use std::io::Write;

fn main() {
    let arguments: Vec<_> = env::args().skip(1).collect();
    let mut log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(env::var_os("SHIM_CALL_LOG").unwrap())
        .unwrap();
    writeln!(log, "{}", arguments.join(" ")).unwrap();
    std::process::exit(23);
}
"#,
    )
    .unwrap();
    let output =
        std::process::Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
            .args(["--edition=2021", "-o"])
            .arg(output_directory.join(executable("provenance")))
            .arg(source)
            .output()
            .expect("compile fake provenance");
    assert!(
        output.status.success(),
        "fake provenance did not compile: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn executable(name: &str) -> String {
    format!("{name}{}", std::env::consts::EXE_SUFFIX)
}
