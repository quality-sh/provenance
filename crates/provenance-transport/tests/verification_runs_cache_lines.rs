//! A line of the verification runs cache that the writer refuses is refused
//! by the reads of the runs too, at its line, as the start and the completion
//! of a run refuse it.
#![cfg(feature = "test-fixture")]
mod support {
    pub mod records;
    pub mod resource_http;
}
use provenance_core::ScopeId;
use serde_json::json;
use support::records::Repository;
use support::resource_http::{call, host};

// No Rule binding: `rule_state_parse_refusal_names_the_line` is for record
// files below the state directory, and the runs cache is a cache file.
#[tokio::test]
async fn the_reads_of_the_runs_name_a_line_of_the_runs_cache_that_the_writer_refuses() {
    let cases = [
        ("{not json", "key must be a string"),
        (r#"{"id":"verification_1"}"#, "missing field `scope_id`"),
    ];
    for (line, message) in cases {
        let repo = Repository::new("The shared graph is readable.");
        let host = host(&repo, false);
        let cache = repo
            .layout
            .verification_runs_path(&ScopeId::new("default").unwrap());
        std::fs::create_dir_all(cache.parent().unwrap()).unwrap();
        std::fs::write(&cache, format!("{line}\n")).unwrap();
        let error = json!({
            "kind": "invalid_state_file", "file": "verification-runs.jsonl", "line": 1,
            "message": message
        });

        for route in ["/verification-runs", "/verification-runs/verification_1"] {
            let (status, answer) = call(&host, "GET", route, None).await;

            assert_eq!(
                (status, &answer["error"]),
                (500, &error),
                "{route}: {answer}"
            );
        }
    }
}
