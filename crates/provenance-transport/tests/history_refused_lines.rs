//! The history route over committed lines that the history read refuses.
#![cfg(feature = "test-fixture")]

use crate::support;

use support::refused_history::refused_history;
use support::resource_http::{call, host};

/// The first read indexes the commits and the second read takes them from
/// the index. Both answer with the versions that the read can read and
/// with one warning for each refused line, in the order of the commits.
#[tokio::test]
async fn history_route_warns_about_each_refused_line() {
    let refused = refused_history();
    let host = host(&refused.repo, false);
    for read in ["first", "indexed"] {
        let (status, body) = call(&host, "GET", "/requirements/req_refused/history", None).await;
        assert_eq!(status, 200, "{read}: {body}");
        let ids = body["data"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["id"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(ids, refused.versions, "{read}: {body}");
        assert_eq!(body["data"]["warnings"], refused.warnings, "{read}: {body}");
    }
}
