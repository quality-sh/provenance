//! A stored message row with no actor, or with a blank actor, does not fit the Message
//! record. The graph check names the file and the line, and gives the step that adds the
//! actor to that line.

use crate::cli_mcp_write_actor::{repository, run};
use crate::cli_message_actor::discussion_with_reply;
use provenance_macros::verifies;
use std::path::Path;

/// The step that the refusal gives for a message row with no actor or a blank actor.
const CORRECTION: &str = "To correct this fault, add the field actor to this line, with the \
    actor that wrote the Message. If that actor is not known, use the actor authoring.";

/// Replaces the stored actor `,"actor":"agent-b"` of the reply row with `actor`, and gives
/// the path of the shard below the state directory and the number of the line.
fn edit_reply_actor(repo: &Path, reply: &str, actor: &str) -> (String, usize) {
    let threads = repo.join(".provenance/state/scopes/default/threads");
    for entry in std::fs::read_dir(&threads).unwrap() {
        let path = entry.unwrap().path();
        let stored = std::fs::read_to_string(&path).unwrap();
        let Some(index) = stored.lines().position(|line| line.contains(reply)) else {
            continue;
        };
        let changed = stored.replacen(",\"actor\":\"agent-b\"", actor, 1);
        assert_ne!(changed, stored, "{stored}");
        std::fs::write(&path, changed).unwrap();
        let name = path.file_name().unwrap().to_str().unwrap();
        return (format!("scopes/default/threads/{name}"), index + 1);
    }
    panic!("no message row holds {reply}");
}

#[test]
#[verifies("rule_message_records_its_actor", examples)]
fn a_message_row_with_no_actor_or_a_blank_actor_is_refused_with_its_line() {
    for (actor, message) in [
        ("", "missing field `actor`"),
        (",\"actor\":\"\"", "the field `actor` is blank"),
        (",\"actor\":\"  \"", "the field `actor` is blank"),
    ] {
        let directory = repository();
        let repo = directory.path();
        let (_, _, reply) = discussion_with_reply(repo);
        let (file, line) = edit_reply_actor(repo, &reply, actor);

        let fault = format!("{file} line {line}: {message}; {CORRECTION}");
        let (code, stdout, stderr) = run(repo, &["check", "--graph"]);
        assert_eq!(
            (code, stdout.replace('\\', "/"), stderr.replace('\\', "/")),
            (
                1,
                format!("graph: findings\n  - {fault}\n"),
                format!("Error: {fault}\n")
            ),
            "{actor}"
        );
        let (code, stdout, _) = run(repo, &["api", "messages"]);
        assert_eq!((code, stdout.as_str()), (1, ""), "{actor}");
    }
}
