use super::bounded_process::{provenance, Bounded};
use super::support::{create_source, init_repo, write_run_dir};

fn land(repo: &str, run_dir: &std::path::Path) -> assert_cmd::assert::Assert {
    let run_dir = run_dir.to_string_lossy().to_string();
    provenance()
        .args([
            "swarm-backtrace",
            "land",
            "--repo",
            repo,
            "--scope",
            "default",
            "--run-dir",
            &run_dir,
            "--format",
            "json",
        ])
        .bounded_assert()
}

#[test]
fn swarm_backtrace_land_refuses_merge_output_under_another_name() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo").to_string_lossy().to_string();
    let run_dir = dir.path().join("run");
    init_repo(&repo);
    create_source(&repo);
    write_run_dir(&run_dir, "Publishing is guarded by worker assignment.");
    std::fs::rename(
        run_dir.join("merge/merged.json"),
        run_dir.join("merge/merge.json"),
    )
    .unwrap();
    let expected = run_dir.join("merge").join("merged.json");

    land(&repo, &run_dir)
        .failure()
        .stderr(predicates::str::contains("merge.json"))
        .stderr(predicates::str::contains(
            expected.to_string_lossy().as_ref(),
        ));
}

#[test]
fn swarm_backtrace_land_names_the_read_paths_when_the_run_is_empty() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo").to_string_lossy().to_string();
    let run_dir = dir.path().join("run");
    init_repo(&repo);
    create_source(&repo);
    std::fs::create_dir_all(&run_dir).unwrap();
    std::fs::write(run_dir.join("notes.json"), r#"{"scratch": true}"#).unwrap();

    land(&repo, &run_dir)
        .failure()
        .stderr(predicates::str::contains(
            "swarm run contains no landable records",
        ))
        .stderr(predicates::str::contains("extractors/"))
        .stderr(predicates::str::contains("refuters/"))
        .stderr(predicates::str::contains("contributions/"))
        .stderr(predicates::str::contains("merge/merged.json"));
}
