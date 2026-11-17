//! A manifest that does not parse in the review library. The failure of the
//! open of a repository has the text of the fault of the manifest, and its
//! error is that fault. The HTTP API of a started review host whose manifest
//! stops parsing refuses a read, a write and the page configuration with
//! `invalid_state_file`, the line of the manifest, the correction of that line
//! and then the two procedures that correct the state.

#[path = "support"]
mod support {
    pub mod no_checks;
    pub mod user_cache;
}

use support::no_checks::NoChecks;
use support::user_cache::use_user_cache;

use provenance_core::protocol::read_failure::{
    ReadFailure, UnparsableStateFile, STATE_CORRECTIONS,
};
use provenance_macros::verifies;
use provenance_review::{HostOptions, RepositoryScope};
use provenance_store::repository::OpenFailure;
use provenance_transport::fixture::records::Repository;
use serde_json::{json, Value};
use std::sync::Arc;

/// The hint of a state file that does not parse, as a person reads it.
const HINT: &str = "To correct this fault, correct the line of the file that this answer \
    names. Then do the operation again. If you cannot correct that line, use one of these \
    two procedures. \
    Procedure 1: get .provenance/state from a commit where Provenance can read it. \
    This can remove records and changes that are not in that commit. \
    Procedure 2: remove .provenance/state and then do the command provenance init. \
    This removes all records, the scopes, the disposition actors and the technical names of \
    the dictionary.";

#[tokio::test(flavor = "multi_thread")]
#[verifies("rule_state_parse_refusal_names_the_corrections", examples)]
async fn a_manifest_that_stops_parsing_refuses_each_read_and_write_with_the_corrections() {
    let cache = tempfile::tempdir().unwrap();
    use_user_cache(cache.path());
    let repository = Repository::new("The review host reads its manifest for each call.");
    let place = RepositoryScope::new(repository.dir.path().to_str().unwrap(), "default");
    let host = provenance_review::start(HostOptions {
        place,
        repository_id: "local".to_owned(),
        port: 0,
        check_port: Arc::new(NoChecks),
        on_read_only: Box::new(|| {}),
    })
    .await
    .unwrap();
    std::fs::write(
        repository.layout.manifest_path(),
        "{\"scopes\":\"default\"}\n",
    )
    .unwrap();

    let endpoint = host.startup().endpoint().to_owned();
    let bearer = host.startup().bearer().to_owned();
    let created = json!({"data":{"actor":"reviewer", "id":"req_new",
        "statement":"The system stores each record.", "status":"discovery", "depends_on":[],
        "supersedes":[]}});
    let answers = tokio::task::spawn_blocking(move || {
        [
            ("GET", "/requirements/req_shared", None),
            ("POST", "/requirements", Some(created)),
            ("GET", "/review-config", None),
        ]
        .map(|(method, route, body)| {
            let (status, answer) = call(&endpoint, &bearer, method, route, body.as_ref());
            json!({"call": format!("{method} {route}"), "status": status,
                "error": answer["error"]})
        })
    })
    .await
    .unwrap();

    let refusal = json!({"kind":"invalid_state_file", "file":"manifest.json", "line":1,
        "message":"invalid type: string \"default\", expected a sequence",
        "hint":HINT});
    assert_eq!(
        answers,
        [
            "GET /requirements/req_shared",
            "POST /requirements",
            "GET /review-config"
        ]
        .map(|call| json!({"call": call, "status": 500, "error": refusal}))
    );
}

#[test]
#[verifies("rule_state_parse_refusal_names_the_corrections", examples)]
fn the_open_failure_of_a_manifest_that_does_not_parse_is_the_fault() {
    let repository = Repository::new("The review library opens the repository for a read.");
    std::fs::write(
        repository.layout.manifest_path(),
        "{\"scopes\":\"default\"}\n",
    )
    .unwrap();
    let place = RepositoryScope::new(repository.dir.path().to_str().unwrap(), "default");

    let error = place.open().unwrap_err();
    assert_eq!(
        error.to_string(),
        format!(
            "manifest.json line 1: invalid type: string \"default\", expected a sequence; \
             {STATE_CORRECTIONS}"
        )
    );
    let Some(OpenFailure::Io(cause)) = error.downcast_ref::<OpenFailure>() else {
        panic!("{error:?}");
    };
    let fault = UnparsableStateFile::find(cause.as_ref()).map(UnparsableStateFile::failure);
    assert_eq!(
        fault,
        Some(ReadFailure::InvalidStateFile {
            file: "manifest.json".to_owned(),
            line: 1,
            message: "invalid type: string \"default\", expected a sequence".to_owned(),
            hint: Some(STATE_CORRECTIONS.to_owned()),
        })
    );
}

/// The status and the JSON body of one call with the host bearer.
fn call(
    endpoint: &str,
    bearer: &str,
    method: &str,
    route: &str,
    body: Option<&Value>,
) -> (u16, Value) {
    let request = ureq::request(method, &format!("{endpoint}{route}"))
        .set("Authorization", &format!("Bearer {bearer}"));
    let reply = match body {
        Some(body) => request
            .set("Content-Type", "application/json")
            .send_string(&body.to_string()),
        None => request.call(),
    };
    let response = match reply {
        Ok(response) | Err(ureq::Error::Status(_, response)) => response,
        Err(error) => panic!("{method} {route}: {error}"),
    };
    let status = response.status();
    (
        status,
        serde_json::from_str(&response.into_string().unwrap()).unwrap(),
    )
}
