//! Composed reads for one known repository record.

use provenance_macros::rule;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{fmt::Display, future::Future, pin::Pin};

/// One future returned by an injected read port.
pub type PortFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, ReadError>> + Send + 'a>>;

/// A canonical record with its native JSON representation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Record {
    pub id: String,
    pub kind: String,
    pub value: Value,
    /// Metadata from the operation that resolved this record.
    #[serde(skip)]
    pub response_metadata: Option<Value>,
}

impl Record {
    pub fn new(id: impl Into<String>, kind: impl Into<String>, value: Value) -> Self {
        Self {
            id: id.into(),
            kind: kind.into(),
            value,
            response_metadata: None,
        }
    }

    #[must_use]
    pub fn with_response_metadata(mut self, metadata: Value) -> Self {
        self.response_metadata = Some(metadata);
        self
    }
}

/// The named view selected for a known record.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum View {
    #[default]
    Record,
    Children,
    Grounding,
    Impact,
}

/// One semantic known-record request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GetInput {
    pub target: String,
    pub view: View,
    pub max_depth: Option<usize>,
    pub returned_kinds: Vec<String>,
    pub limit: Option<usize>,
}

impl GetInput {
    pub fn new(target: impl Into<String>, view: View) -> Self {
        Self {
            target: target.into(),
            view,
            max_depth: None,
            returned_kinds: Vec::new(),
            limit: None,
        }
    }
}

/// The bound and continuation state of a potentially incomplete result.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Bounds {
    pub limit: usize,
    pub max_depth: Option<usize>,
    pub has_more: bool,
    pub continuation: Option<String>,
    pub truncated: bool,
}

/// A composed known-record result.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GetOutcome {
    pub record: Record,
    pub view: View,
    pub related: Vec<Record>,
    pub detail: Option<Value>,
    pub bounds: Option<Bounds>,
    /// Metadata from the operation that resolved the selected record.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record_metadata: Option<Value>,
    /// Metadata from the traversal or impact operation, when selected.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub view_metadata: Option<Value>,
}

/// A traversal request sent through the injected operation port.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraversalRequest {
    pub target: String,
    pub kind: String,
    pub view: View,
    pub max_depth: usize,
    pub limit: usize,
}

/// A traversal result before returned-kind filtering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Traversal {
    pub records: Vec<Record>,
    pub bounds: Bounds,
    /// Metadata from the traversal operation.
    pub response_metadata: Option<Value>,
}

/// An impact result with its page state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Impact {
    pub detail: Value,
    pub bounds: Bounds,
    /// Metadata from the impact operation.
    pub response_metadata: Option<Value>,
}

/// A failure from validation, identity resolution, or an injected operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReadError {
    InvalidOptions,
    NotFound,
    AmbiguousIdentity,
    Operation(String),
}

impl Display for ReadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidOptions => formatter.write_str("unsupported read options"),
            Self::NotFound => formatter.write_str("record does not exist"),
            Self::AmbiguousIdentity => formatter.write_str("record ID is not unique"),
            Self::Operation(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for ReadError {}

/// Existing operation paths needed to compose a known-record read.
pub trait GetPort: Send + Sync {
    fn resolve<'a>(&'a self, id: &'a str) -> PortFuture<'a, Option<Record>>;
    fn traverse(&self, request: TraversalRequest) -> PortFuture<'_, Traversal>;
    fn impact<'a>(&'a self, record: &'a Record, limit: usize) -> PortFuture<'a, Impact>;
}

impl<P: GetPort> crate::Porcelain<P> {
    /// Resolves one record and composes its selected human-facing view.
    #[rule("rule_porcelain_id_needs_no_kind_selector")]
    #[rule("rule_porcelain_get_returns_record")]
    #[rule("rule_porcelain_get_selects_child_context")]
    #[rule("rule_porcelain_return_filter_keeps_traversal")]
    #[rule("rule_porcelain_get_has_grounding_impact")]
    #[rule("rule_porcelain_read_rejects_bad_options")]
    #[rule("rule_porcelain_output_reports_bounds")]
    pub async fn get(&self, input: GetInput) -> Result<GetOutcome, ReadError> {
        validate(&input)?;
        let record = self
            .port
            .resolve(&input.target)
            .await?
            .ok_or(ReadError::NotFound)?;
        let record_metadata = record.response_metadata.clone();
        if matches!(input.view, View::Children | View::Grounding) {
            let traversal = self
                .port
                .traverse(TraversalRequest {
                    target: input.target,
                    kind: record.kind.clone(),
                    view: input.view,
                    max_depth: input.max_depth.unwrap_or(1),
                    limit: input.limit.unwrap_or(50),
                })
                .await?;
            let related = traversal
                .records
                .into_iter()
                .filter(|candidate| {
                    input.returned_kinds.is_empty()
                        || input.returned_kinds.contains(&candidate.kind)
                })
                .collect();
            return Ok(GetOutcome {
                record,
                view: input.view,
                related,
                detail: None,
                bounds: Some(traversal.bounds),
                record_metadata,
                view_metadata: traversal.response_metadata,
            });
        }
        if input.view == View::Impact {
            let impact = self.port.impact(&record, input.limit.unwrap_or(50)).await?;
            return Ok(GetOutcome {
                record,
                view: input.view,
                related: Vec::new(),
                detail: Some(impact.detail),
                bounds: Some(impact.bounds),
                record_metadata,
                view_metadata: impact.response_metadata,
            });
        }
        Ok(GetOutcome {
            record,
            view: input.view,
            related: Vec::new(),
            detail: None,
            bounds: None,
            record_metadata,
            view_metadata: None,
        })
    }
}

fn validate(input: &GetInput) -> Result<(), ReadError> {
    let has_traversal_options = input.max_depth.is_some() || !input.returned_kinds.is_empty();
    let unsupported = match input.view {
        View::Record => has_traversal_options || input.limit.is_some(),
        View::Children | View::Grounding => false,
        View::Impact => has_traversal_options,
    };
    if unsupported
        || input.max_depth == Some(0)
        || input.limit == Some(0)
        || input.returned_kinds.iter().any(String::is_empty)
    {
        return Err(ReadError::InvalidOptions);
    }
    Ok(())
}
