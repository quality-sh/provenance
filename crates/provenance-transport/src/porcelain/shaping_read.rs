//! Compose a Topic summary from bounded, stamped resource reads.
use super::{operation_error, response_parts};
use provenance_core::protocol::{RecordResolution, ResolveRecordQuery, ResponseMeta};
use provenance_core::{NodeType, SDK_PROTOCOL_VERSION};
use provenance_porcelain::get::{ReadError, SessionSummary, TraversalRequest, View};
use provenance_store::operations::catalog::{self, resource_pages::ResourcePageRequest};

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
    if !host.advertises(catalog::resource_pages::PageQuestions::NAME) {
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
        .invoke_scoped_typed::<catalog::resource_pages::PageQuestions>(ResourcePageRequest {
            limit: request.limit.min(200),
            cursor: None,
            exclude_terminal: false,
        })
        .await
        .map_err(|error| operation_error(&error))?;
    let complete = !truncated
        && !filtered
        && !page.result.has_more
        && page.freshness_error.is_none()
        && metadata.freshness_error.is_none()
        && traversal.freshness_error.is_none()
        && metadata.stamp.as_ref().is_some_and(|stamp| {
            stamp.instance_id == page.stamp.instance_id
                && stamp.serial == page.stamp.serial
                && stamp.digest == page.stamp.digest
                && traversal.stamp.as_ref().is_some_and(|trace| {
                    trace.instance_id == stamp.instance_id
                        && trace.serial == stamp.serial
                        && trace.digest == stamp.digest
                })
        });
    Ok(Some(SessionSummary::from_questions(
        &topic,
        &page.result.items,
        complete,
    )))
}

use provenance_store::operations::catalog::Operation as _;
