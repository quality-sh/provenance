use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn edited_skill_error_names_recovery_in_the_target_project() {
    let temporary = tempfile::tempdir().unwrap();
    let caller = temporary.path().join("caller");
    let project = temporary.path().join("target project; literal");
    std::fs::create_dir(&caller).unwrap();
    let path = project.to_str().unwrap();
    let init_args = ["init", "--path", path, "--scope", "default", "--path-prefix", "."];

    Command::cargo_bin("provenance")
        .unwrap()
        .current_dir(&caller)
        .args(init_args)
        .assert()
        .success();
    let skill = project.join(".agents/skills/provenance-shaping/SKILL.md");
    let original = std::fs::read(&skill).unwrap();
    let edited = [original.as_slice(), b"\nUser edit.\n"].concat();
    std::fs::write(&skill, &edited).unwrap();
    std::fs::write(project.join("unrelated.txt"), "keep\n").unwrap();

    Command::cargo_bin("provenance")
        .unwrap()
        .current_dir(&caller)
        .args(init_args)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "run `provenance skills install --force`",
        ))
        .stderr(predicate::str::contains(format!(
            "working directory: {}",
            project.display()
        )));
    assert_eq!(std::fs::read(&skill).unwrap(), edited);

    Command::cargo_bin("provenance")
        .unwrap()
        .current_dir(&project)
        .args(["skills", "install", "--force"])
        .assert()
        .success();
    assert_eq!(std::fs::read(&skill).unwrap(), original);
    assert_eq!(
        std::fs::read_to_string(project.join("unrelated.txt")).unwrap(),
        "keep\n"
    );
    Command::cargo_bin("provenance")
        .unwrap()
        .current_dir(&caller)
        .args(init_args)
        .assert()
        .success();
}

#[test]
fn global_copy_conflict_guidance_keeps_the_install_mode() {
    let temporary = tempfile::tempdir().unwrap();
    let caller = temporary.path().join("caller");
    let home = temporary.path().join("home");
    std::fs::create_dir_all(&caller).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    let args = ["skills", "install", "--global", "--copy"];

    Command::cargo_bin("provenance")
        .unwrap()
        .current_dir(&caller)
        .env("HOME", &home)
        .args(args)
        .assert()
        .success();
    let skill = home.join(".agents/skills/provenance-shaping/SKILL.md");
    let original = std::fs::read(&skill).unwrap();
    let edited = [original.as_slice(), b"\nUser edit.\n"].concat();
    std::fs::write(&skill, &edited).unwrap();

    Command::cargo_bin("provenance")
        .unwrap()
        .current_dir(&caller)
        .env("HOME", &home)
        .args(args)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "run `provenance skills install --global --copy --force`",
        ));
    assert_eq!(std::fs::read(&skill).unwrap(), edited);

    Command::cargo_bin("provenance")
        .unwrap()
        .current_dir(&caller)
        .env("HOME", &home)
        .args(["skills", "install", "--global", "--copy", "--force"])
        .assert()
        .success();
    assert_eq!(std::fs::read(&skill).unwrap(), original);
    assert!(!caller.join(".agents").exists());
}
