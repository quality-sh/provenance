use provenance_http_client::{
    types::{
        SourcesCreateDiscussionMessageFailureOperationError, SourcesCreateDiscussionMessageRequest,
        SourcesListDiscussionsSuccessDiscussionGroup,
    },
    Error, HttpClient, OperationFailure,
};

#[tokio::test]
#[ignore = "the generated-client harness supplies a live fixture host"]
async fn real_host_preserves_typed_metadata_refusal() {
    let base_url = std::env::var("PROVENANCE_CLIENT_BASE_URL").unwrap();
    match HttpClient::connect_with_bearer(&base_url, "wrong-secret")
        .await
        .unwrap_err()
    {
        Error::Operation {
            status: 401,
            failure: OperationFailure::Metadata(failure),
        } => {
            let value = serde_json::to_value(failure).unwrap();
            assert_eq!(value["error"]["kind"], "unauthenticated");
            assert_eq!(value["meta"], serde_json::json!({}));
        }
        error => panic!("expected typed metadata failure, got {error}"),
    }
}

#[tokio::test]
#[ignore = "the generated-client harness supplies a live fixture host"]
async fn real_host_covers_read_guarded_mutation_and_typed_failure() {
    let base_url = std::env::var("PROVENANCE_CLIENT_BASE_URL").unwrap();
    let token = std::env::var("PROVENANCE_CLIENT_TOKEN").unwrap();
    let client = HttpClient::connect_with_bound_identity(&base_url, &token, "fixture", "default")
        .await
        .unwrap();

    let discussions = client
        .source_list_discussions("source_ts", None, None)
        .await
        .unwrap();
    let (discussion_id, version) = match &discussions.data.items[0] {
        SourcesListDiscussionsSuccessDiscussionGroup::Addressed { discussion, .. } => {
            (discussion.discussion_id.as_str(), discussion.version)
        }
        SourcesListDiscussionsSuccessDiscussionGroup::Legacy { .. } => {
            panic!("expected an addressed discussion")
        }
    };

    let message: SourcesCreateDiscussionMessageRequest = serde_json::from_value(
        serde_json::json!({"data":{"actor":"fixture","role":"system","body":"Rust text"}}),
    )
    .unwrap();
    let created = client
        .source_create_discussion_message(
            "source_ts",
            discussion_id,
            "discussion_rust",
            &version.to_string(),
            &message,
        )
        .await
        .unwrap();
    assert_eq!(created.data.version, version + 1);

    let invalid: SourcesCreateDiscussionMessageRequest = serde_json::from_value(
        serde_json::json!({"data":{"actor":"fixture","role":"system","body":" "}}),
    )
    .unwrap();
    match client
        .source_create_discussion_message(
            "source_ts",
            discussion_id,
            "discussion_rust_invalid",
            &created.data.version.to_string(),
            &invalid,
        )
        .await
        .unwrap_err()
    {
        Error::Operation {
            failure: OperationFailure::SourceCreateDiscussionMessage(failure),
            ..
        } => assert!(matches!(
            failure.error,
            SourcesCreateDiscussionMessageFailureOperationError::EmptyMessageBody
        )),
        error => panic!("expected a typed empty-message failure, got {error}"),
    }
}
