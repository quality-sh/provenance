//! Compose a Topic summary from bounded, stamped resource reads.
use super::{operation_error, response_parts};
use provenance_core::protocol::{ListData, RecordResolution, ResolveRecordQuery, ResponseMeta, SuccessEnvelope};
use provenance_core::{NodeType, Question, SDK_PROTOCOL_VERSION};
use provenance_porcelain::get::{ReadError, SessionSummary, TraversalRequest, View};
use provenance_store::operations::catalog::{self, Operation as _};

pub(super) async fn summary(
    host: &crate::StatementHost,
    request: &TraversalRequest,
    traversal: &ResponseMeta,
    truncated: bool,
    filtered: bool,
) -> Result<Option<SessionSummary>, ReadError> {
    if request.kind != NodeType::Topic || request.view != View::Children {
        return Ok(None);
    }
    if !crate::routing::find(&axum::http::Method::GET, "/questions")
        .is_some_and(|route| host.advertises(route.definition.name))
    {
        return Ok(None);
    }
    let resolved = host
        .invoke_scoped_typed::<catalog::ResolveRecord>(ResolveRecordQuery {
            protocol_version: Some(SDK_PROTOCOL_VERSION),
            id: request.target.clone(),
            allowed_node_types: vec![NodeType::Topic],
        })
        .await
        .map_err(|error| operation_error(&error))?;
    let (record, metadata) = response_parts(resolved);
    let RecordResolution::Found(provenance_core::protocol::GraphNode::Topic(topic)) =
        record.resolution
    else {
        return Err(ReadError::NotFound);
    };
    let page = host
        .invoke_resource(
            axum::http::Method::GET,
            "/questions",
            serde_json::json!({}),
            std::collections::BTreeMap::from([
                ("limit".into(), request.limit.min(200).to_string()),
                ("exclude_terminal".into(), "false".into()),
            ]),
            axum::http::HeaderMap::new(),
        )
        .await
        .map_err(|error| ReadError::Operation(error.to_string()))?;
    let page: SuccessEnvelope<ListData<Question>> = serde_json::from_value(page)
        .map_err(|error| ReadError::Operation(error.to_string()))?;
    let complete = !truncated
        && !filtered
        && page.meta.has_more == Some(false)
        && page.meta.freshness_error.is_none()
        && metadata.freshness_error.is_none()
        && traversal.freshness_error.is_none()
        && metadata.stamp.as_ref().is_some_and(|stamp| {
            page.meta.stamp.as_ref().is_some_and(|page_stamp| {
                stamp.instance_id == page_stamp.instance_id
                    && stamp.serial == page_stamp.serial
                    && stamp.digest == page_stamp.digest
            })
                && traversal.stamp.as_ref().is_some_and(|trace| {
                    trace.instance_id == stamp.instance_id
                        && trace.serial == stamp.serial
                        && trace.digest == stamp.digest
                })
        });
    Ok(Some(SessionSummary::from_questions(
        &topic,
        &page.data.items,
        complete,
    )))
}

