//! A manifest that does not parse in the review library. The open of a
//! repository keeps the fault of the manifest as its cause. The HTTP API of a
//! started review host whose manifest stops parsing refuses a read, a write
//! and the page configuration with `invalid_state_file`, the line of the
//! manifest and the two procedures that correct the state.

use provenance_core::protocol::read_failure::{ReadFailure, UnparsableStateFile, STATE_CORRECTIONS};
use provenance_macros::verifies;
use provenance_porcelain::check::{Category, CheckPort, PortFuture};
use provenance_review::{HostOptions, RepositoryScope};
use provenance_transport::fixture::records::Repository;
use serde_json::{json, Value};
use std::{path::Path, sync::Arc};

/// A check port for a host whose test never runs a check.
struct NoChecks;

impl CheckPort for NoChecks {
    fn run<'a>(&'a self, _: Category, _: Option<&'a str>) -> PortFuture<'a> {
        Box::pin(async { Err("this host runs no checks".to_owned()) })
    }
}

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
    std::fs::write(repository.layout.manifest_path(), "{\"scopes\":\"default\"}\n").unwrap();

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
        "hint":STATE_CORRECTIONS});
    assert_eq!(
        answers,
        ["GET /requirements/req_shared", "POST /requirements", "GET /review-config"]
            .map(|call| json!({"call": call, "status": 500, "error": refusal}))
    );
}

#[test]
#[verifies("rule_state_parse_refusal_names_the_corrections", examples)]
fn the_open_of_a_manifest_that_does_not_parse_has_the_fault_as_its_cause() {
    let repository = Repository::new("The review library opens the repository for a read.");
    std::fs::write(repository.layout.manifest_path(), "{\"scopes\":\"default\"}\n").unwrap();
    let place = RepositoryScope::new(repository.dir.path().to_str().unwrap(), "default");

    let error = place.open().unwrap_err();
    let fault = UnparsableStateFile::find(error.as_ref()).map(UnparsableStateFile::failure);
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
    (status, serde_json::from_str(&response.into_string().unwrap()).unwrap())
}

/// Points every platform's per-user cache at one isolated directory.
fn use_user_cache(root: &Path) {
    let cache = root.join("cache");
    std::env::set_var("HOME", root);
    std::env::set_var("XDG_CACHE_HOME", &cache);
    std::env::set_var("LOCALAPPDATA", &cache);
}
