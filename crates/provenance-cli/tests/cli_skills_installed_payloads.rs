//! Implementation aid: compare complete installed payloads with the root sources.
//! Runtime ownership and copy behaviour have separate Rule-bound tests.

use assert_cmd::Command;
use std::path::Path;

#[test]
fn installed_bundled_payloads_match_the_root_sources() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let installed = tempfile::tempdir().unwrap();
    Command::cargo_bin("provenance")
        .unwrap()
        .current_dir(installed.path())
        .args(["skills", "install", "--copy", "--format", "json"])
        .assert()
        .success();
    let names = std::fs::read_dir(workspace.join("skills"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(names.len(), 4, "{names:?}");
    for name in &names {
        let source =
            std::fs::read_to_string(workspace.join("skills").join(name).join("SKILL.md")).unwrap();
        let canonical = std::fs::read_to_string(
            installed
                .path()
                .join(".agents/skills")
                .join(name)
                .join("SKILL.md"),
        )
        .unwrap();
        let copied = std::fs::read_to_string(
            installed
                .path()
                .join(".claude/skills")
                .join(name)
                .join("SKILL.md"),
        )
        .unwrap();
        assert_eq!(copied, canonical, "Claude copy differs for {name}");
        let stamp_start = canonical.find("<!-- Installed by provenance ").unwrap();
        let stamp_end = stamp_start + canonical[stamp_start..].find('\n').unwrap() + 1;
        let payload = format!("{}{}", &canonical[..stamp_start], &canonical[stamp_end..]);
        assert_eq!(payload, source, "installed payload differs for {name}");
    }
}
