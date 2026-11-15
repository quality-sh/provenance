//! Review decisions through the HTTP routes.

use super::resource_http::call;
use provenance_transport::StatementHost;

/// Accepts the pending submission of one record as the reviewer through the
/// decide route. The repository must allow the reviewer.
pub async fn accept_pending(host: &StatementHost, collection: &str, kind: &str, id: &str) {
    let path = format!("/{collection}/{id}");
    let (status, read) = call(host, "GET", &path, None).await;
    assert_eq!(status, 200, "{read}");
    let proposal = read["data"]["decision"]["pending"]["proposal_id"]
        .as_str()
        .unwrap_or_else(|| panic!("no pending submission: {read}"))
        .to_owned();
    let (status, decided) = call(
        host,
        "POST",
        &format!("{path}/submissions/{proposal}/decide"),
        Some(serde_json::json!({"data":{
            "actor":{"identity_type":"human","id":"reviewer"},
            "decision":"accepted", "rationale":null,
            "canonical_artifact":{"artifact_type":kind,"artifact_id":id},
            "feedback":null, "declared_by":null
        }})),
    )
    .await;
    assert_eq!(status, 200, "{decided}");
}
