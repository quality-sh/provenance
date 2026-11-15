//! A history of `LONG` commits that each change the statement of `req_a`,
//! so that the entry operation reads past the first page of 200 versions.

use super::history_fixture::{Tree, STATE};
use provenance_store::review::history::contract::Commit;
use serde_json::{json, Value};

/// The commits of the long history.
pub const LONG: i64 = 205;

/// The name of commit `index` of the long history.
pub fn long_name(index: i64) -> String {
    format!("edit_{index:03}")
}

/// `LONG` commits, each the child of the one before, that each give
/// `req_a` in the Requirement file of `default` in `working` a new
/// statement.
pub fn long_history(working: &Tree) -> Vec<Commit> {
    let path = working
        .keys()
        .find(|path| path.starts_with(&format!("{STATE}scopes/default/requirements/")))
        .expect("a Requirement file in default");
    let text = String::from_utf8(working[path].clone()).unwrap();
    (0..LONG)
        .map(|index| {
            let mut file = String::new();
            for line in text.lines() {
                let mut record: Value = serde_json::from_str(line).unwrap();
                if record["id"] == json!("req_a") {
                    record["statement"] = json!(format!("Edit {index} of the statement."));
                }
                file.push_str(&serde_json::to_string(&record).unwrap());
                file.push('\n');
            }
            let parent = (index > 0).then(|| long_name(index - 1));
            let parents: Vec<&str> = parent.iter().map(String::as_str).collect();
            Commit::new(
                &long_name(index),
                &parents,
                "Reviewer",
                1_700_000_000 + 60 * index,
            )
            .with_file(path, &file)
        })
        .collect()
}
