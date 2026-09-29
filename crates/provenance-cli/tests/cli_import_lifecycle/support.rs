use assert_cmd::Command;
use std::path::{Path, PathBuf};

pub fn provenance() -> Command {
    Command::cargo_bin("provenance").unwrap()
}

pub fn init_repo(repo: &Path, disposition_actor_id: Option<&str>) {
    init_repo_with_actors(repo, disposition_actor_id.as_slice());
}

pub fn init_repo_with_actors(repo: &Path, disposition_actor_ids: &[&str]) {
    let mut command = provenance();
    command.args([
        "init",
        "--path",
        repo.to_str().unwrap(),
        "--scope",
        "default",
    ]);
    for actor_id in disposition_actor_ids {
        command.args(["--disposition-actor-id", actor_id]);
    }
    command.assert().success();
}

pub fn create_system_source(repo: &Path) {
    provenance()
        .args([
            "sources",
            "create",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--id",
            "source_a",
            "--name",
            "Source A",
            "--source-type",
            "system_state",
            "--reference",
            "git:a@abc1234",
            "--commit-pin",
            "abc1234",
        ])
        .assert()
        .success();
}

pub fn export_scope(repo: &Path, output: &Path) -> assert_cmd::assert::Assert {
    crate::export_fixture_support::make_default_scope_portable(repo);
    provenance()
        .args([
            "export",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--format",
            "json",
            "--output",
            output.to_str().unwrap(),
        ])
        .assert()
}

pub fn import_scope(repo: &Path, input: &Path) -> assert_cmd::assert::Assert {
    provenance()
        .args([
            "import",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--input",
            input.to_str().unwrap(),
        ])
        .assert()
}

pub fn shipped_repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

pub fn write_json(path: &Path, value: &serde_json::Value) {
    std::fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
}
