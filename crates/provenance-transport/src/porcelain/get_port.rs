use axum::http::{HeaderMap, Method};
use provenance_porcelain::get::{
    Bounds, GetPort, Impact, PortFuture, ReadError, Record, Traversal, TraversalRequest, View,
};
use provenance_store::operations::catalog::{self, Operation as _};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

const ID_PLACEHOLDER: &str = concat!("{", "id", "}");

struct RecordRoute {
    kind: &'static str,
    path: &'static str,
    operation: &'static str,
}

fn record_routes() -> impl Iterator<Item = RecordRoute> {
    catalog::definitions().iter().filter_map(|definition| {
        let kind = definition
            .registration
            .queries
            .iter()
            .find(|query| query.name == catalog::Trace::NAME)?
            .request
            .node_type?;
        Some(RecordRoute {
            kind,
            path: definition.path,
            operation: definition.name,
        })
    })
}

/// Existing resource routes adapted to the injected Porcelain read port.
#[derive(Clone)]
pub struct HostGetPort {
    host: crate::StatementHost,
}

impl HostGetPort {
    pub const fn new(host: crate::StatementHost) -> Self {
        Self { host }
    }

    async fn query(&self, path: &str, query: BTreeMap<String, String>) -> Result<Value, ReadError> {
        self.host
            .invoke_resource(
                Method::GET,
                path,
                Value::Object(Map::default()),
                query,
                HeaderMap::new(),
            )
            .await
            .map_err(|error| operation_error(&error))
    }
}

impl GetPort for HostGetPort {
    fn resolve<'a>(&'a self, id: &'a str) -> PortFuture<'a, Option<Record>> {
        Box::pin(async move {
            let mut found = None;
            for route in record_routes() {
                if !self.host.advertises(route.operation) {
                    continue;
                }
                let path = route.path.replace(ID_PLACEHOLDER, id);
                match self.query(&path, BTreeMap::new()).await {
                    Ok(value) => {
                        let data = value.get("data").cloned().ok_or_else(malformed)?;
                        if found.is_some() {
                            return Err(ReadError::AmbiguousIdentity);
                        }
                        let metadata = value.get("meta").cloned().ok_or_else(malformed)?;
                        found = Some(
                            Record::new(id, route.kind, data).with_response_metadata(metadata),
                        );
                    }
                    Err(ReadError::NotFound) => {}
                    Err(error) => return Err(error),
                }
            }
            Ok(found)
        })
    }

    fn traverse(&self, request: TraversalRequest) -> PortFuture<'_, Traversal> {
        Box::pin(async move {
            let route = route_for_kind(&request.kind)?;
            let query = BTreeMap::from([
                ("query".to_owned(), catalog::Trace::NAME.to_owned()),
                ("limit".to_owned(), request.limit.to_string()),
                ("max_depth".to_owned(), request.max_depth.to_string()),
                (
                    "direction".to_owned(),
                    if request.view == View::Children {
                        "in"
                    } else {
                        "out"
                    }
                    .to_owned(),
                ),
            ]);
            let value = self
                .query(&route.path.replace(ID_PLACEHOLDER, &request.target), query)
                .await?;
            let records = value
                .pointer("/data/nodes")
                .and_then(Value::as_array)
                .ok_or_else(malformed)?;
            let mut permitted = Vec::new();
            for entry in records {
                let record = record_from_traced_node(entry)?;
                if route_for_kind(&record.kind)
                    .is_ok_and(|candidate| self.host.advertises(candidate.operation))
                {
                    permitted.push(record);
                }
            }
            Ok(Traversal {
                records: permitted,
                bounds: bounds(&value, Some(request.max_depth))?,
                response_metadata: value.get("meta").cloned(),
            })
        })
    }

    fn impact<'a>(&'a self, record: &'a Record, limit: usize) -> PortFuture<'a, Impact> {
        Box::pin(async move {
            if !record_routes().all(|route| self.host.advertises(route.operation)) {
                return Err(ReadError::InvalidOptions);
            }
            let route = route_for_kind(&record.kind)?;
            let query = BTreeMap::from([
                ("query".to_owned(), catalog::Impact::NAME.to_owned()),
                ("limit".to_owned(), limit.to_string()),
            ]);
            let value = self
                .query(&route.path.replace(ID_PLACEHOLDER, &record.id), query)
                .await?;
            let detail = value.get("data").cloned().ok_or_else(malformed)?;
            let mut result_bounds = bounds(&value, None)?;
            result_bounds.truncated |= detail
                .get("scan_cut")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            Ok(Impact {
                detail,
                bounds: result_bounds,
                response_metadata: value.get("meta").cloned(),
            })
        })
    }
}

fn route_for_kind(kind: &str) -> Result<RecordRoute, ReadError> {
    record_routes()
        .find(|route| route.kind == kind)
        .ok_or(ReadError::InvalidOptions)
}

fn record_from_traced_node(value: &Value) -> Result<Record, ReadError> {
    let node = value.get("node").ok_or_else(malformed)?;
    let id = node
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(malformed)?;
    let kind = node
        .get("node_type")
        .and_then(Value::as_str)
        .ok_or_else(malformed)?;
    let depth = value
        .get("depth")
        .and_then(Value::as_u64)
        .and_then(|depth| usize::try_from(depth).ok())
        .ok_or_else(malformed)?;
    Ok(Record::new(id, kind, node.clone()).at_depth(depth))
}

fn bounds(value: &Value, max_depth: Option<usize>) -> Result<Bounds, ReadError> {
    let limit = value
        .pointer("/meta/limit")
        .and_then(Value::as_u64)
        .and_then(|limit| usize::try_from(limit).ok())
        .ok_or_else(malformed)?;
    let has_more = value
        .pointer("/meta/has_more")
        .and_then(Value::as_bool)
        .ok_or_else(malformed)?;
    Ok(Bounds {
        limit,
        max_depth,
        has_more,
        continuation: value
            .pointer("/meta/next_cursor")
            .and_then(Value::as_str)
            .map(str::to_owned),
        truncated: has_more,
    })
}

fn malformed() -> ReadError {
    ReadError::Operation("operation returned an invalid get result".to_owned())
}

fn operation_error(error: &provenance_core::protocol::failure::ErasedFailure) -> ReadError {
    if error.error.get("kind").and_then(Value::as_str) == Some("resource_not_found") {
        ReadError::NotFound
    } else {
        ReadError::Operation(serde_json::to_string(error).unwrap_or_default())
    }
}

pub(super) fn is_available(host: &crate::StatementHost) -> bool {
    record_routes().any(|route| host.advertises(route.operation))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn malformed_or_missing_bounds_are_refused() {
        for value in [
            json!({"meta":{"has_more":false}}),
            json!({"meta":{"limit":7}}),
        ] {
            assert_eq!(
                bounds(&value, Some(2)),
                Err(ReadError::Operation(
                    "operation returned an invalid get result".to_owned()
                ))
            );
        }
    }
}
