//! Implementation-aid tests, without a Rule binding: no active Rule states these help
//! lines. They read the help of the spawned binary, so that a new user who reads only
//! the help finds a description for each root command, the record types that submit
//! accepts, the update that cites a Source, and the archive form of update (beads
//! provenance-8h5u, provenance-19gh and provenance-9v5u).

use std::process::Command;

fn help(words: &[&str]) -> String {
    let output = Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .args(words)
        .arg("--help")
        .output()
        .unwrap();
    assert!(output.status.success(), "{words:?}: {output:?}");
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn root_help_gives_each_command_a_description() {
    let text = help(&[]);
    let rows = text
        .lines()
        .skip_while(|line| *line != "Commands:")
        .skip(1)
        .take_while(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();
    assert!(rows.len() > 10, "{text}");
    let bare = rows
        .iter()
        .filter(|row| row.split_whitespace().count() < 2)
        .collect::<Vec<_>>();
    assert!(
        bare.is_empty(),
        "rows with no description: {bare:?}\n{text}"
    );
}

#[test]
fn submit_help_names_each_record_type_that_submit_accepts() {
    let text = help(&["record_x", "submit"]);
    assert!(
        text.starts_with(
            "Submit the target Source, Requirement, Resolution, Rule, Topic, or Question for \
             review.\n"
        ),
        "{text}"
    );
}

#[test]
fn create_help_names_the_update_that_cites_a_source() {
    let form = "update --if-match <etag> \\\n    --relationships-json \
                '{\"cites\":{\"add\":[{\"source_id\":\"<source_id>\",\"clause\":\"<clause>\"}]}}'";
    let text = help(&["record_x", "create"]);
    assert!(text.contains("Create takes no Source citation."), "{text}");
    assert!(text.contains(form), "{text}");

    // The help of one record type lists only the fields that create accepts for that
    // type, so it points to the untyped help and names no update flag.
    let text = help(&["record_x", "create", "--type", "requirement"]);
    assert!(
        text.contains(
            "Create takes no Source citation. To cite a Source, update the Requirement after \
             create\nand add a cites relationship. The create help with no record type shows \
             the update."
        ),
        "{text}"
    );
}

#[test]
fn update_help_shows_how_to_archive_a_record_and_the_cascade() {
    let text = help(&["record_x", "update"]);
    assert!(
        text.contains(
            "provenance <id> update --status archived --archived-in-commit <full_commit_sha>"
        ),
        "{text}"
    );
    assert!(
        text.contains(
            "each Requirement, Rule, and Resolution under it that has no other live parent"
        ),
        "{text}"
    );
    assert!(
        text.contains("Archiving a Topic also archives all the Questions in that Topic."),
        "{text}"
    );
}
