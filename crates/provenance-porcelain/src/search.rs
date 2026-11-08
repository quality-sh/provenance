//! Scope-wide discovery through the canonical search operation.

use provenance_core::model::{
    QuestionStatus, RequirementStatus, ResolutionStatus, RuleStatus, TopicStatus,
};
use provenance_core::protocol::{
    GraphNode, QueryResponse, ResolveSymbolMatch, ResolveSymbolQuery, ResolveSymbolResult,
    SearchQuery, SearchResult, QUERY_DEFAULT_LIMIT,
};
use provenance_core::{NodeType, SDK_PROTOCOL_VERSION};
use provenance_macros::rule;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{fmt::Display, future::Future, pin::Pin};

/// One future returned by an injected search port.
pub type PortFuture<'a> =
    Pin<Box<dyn Future<Output = Result<QueryResponse<SearchResult>, SearchError>> + Send + 'a>>;

/// One future returned by the code lookup of an injected search port.
pub type SymbolFuture<'a> = Pin<
    Box<dyn Future<Output = Result<QueryResponse<ResolveSymbolResult>, SearchError>> + Send + 'a>,
>;

/// The one search request. Its fields are the search fields.
#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SearchInput {
    /// Text to find in record ids and text.
    #[serde(default)]
    pub text: Option<String>,
    /// Record kinds to return.
    #[serde(default)]
    pub kind: Vec<NodeType>,
    /// Include archived and other terminal records. A text search omits them when this
    /// field is not given; a search with a file returns every bound Rule and refuses
    /// false.
    #[serde(default)]
    pub include_terminal: Option<bool>,
    /// The continuation from an earlier page of the same search.
    #[serde(default)]
    pub cursor: Option<String>,
    /// The page size.
    #[serde(default)]
    #[schemars(range(min = 1, max = provenance_core::protocol::QUERY_MAX_LIMIT))]
    pub limit: Option<usize>,
    /// A repository-relative source file. Returns the Rules bound in that file.
    #[serde(default)]
    pub file: Option<String>,
    /// A symbol in the file. Rules bound to that symbol come first.
    #[serde(default)]
    pub symbol: Option<String>,
    /// A line in the file. Rules bound at that line come first.
    #[serde(default)]
    pub line: Option<usize>,
}

/// One search page, with the code sites when the search has a file.
#[derive(Debug, Serialize, JsonSchema)]
pub struct SearchOutcome {
    #[serde(flatten)]
    pub response: QueryResponse<SearchResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matches: Option<Vec<ResolveSymbolMatch>>,
}

/// A failure from search validation, authorization, or the canonical operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SearchError {
    InvalidOptions {
        reason: String,
    },
    /// A code lookup option is not valid; `field` names the request field.
    InvalidCodeLookup {
        field: &'static str,
        reason: String,
    },
    AccessDenied,
    Operation {
        message: String,
        detail: serde_json::Value,
    },
}

impl Display for SearchError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidOptions { reason } => formatter.write_str(reason),
            Self::InvalidCodeLookup { field, reason } => write!(formatter, "{field}: {reason}"),
            Self::AccessDenied => formatter.write_str("search access denied"),
            Self::Operation { message, .. } => formatter.write_str(message),
        }
    }
}

impl std::error::Error for SearchError {}

impl SearchError {
    /// The inputs that recover from a validation failure, as objects of field names and
    /// placeholder values. A consumer renders each one in its own syntax after the
    /// message, so the refusal gives the text and filter forms.
    #[provenance_macros::rule("rule_search_refusal_names_query")]
    pub fn recovery_inputs(&self) -> Vec<serde_json::Value> {
        match self {
            Self::InvalidOptions { .. } => vec![
                serde_json::json!({"text": "<query>"}),
                serde_json::json!({"kind": ["<kind>"]}),
            ],
            Self::InvalidCodeLookup { .. } => vec![
                serde_json::json!({"file": "<path>", "symbol": "<name>"}),
                serde_json::json!({"file": "<path>", "line": "<n>"}),
            ],
            Self::AccessDenied | Self::Operation { .. } => Vec::new(),
        }
    }
}

/// The canonical operations needed for scope-wide discovery and code lookup.
pub trait SearchPort: Send + Sync {
    fn search(&self, request: SearchQuery) -> PortFuture<'_>;
    fn resolve_symbol(&self, request: ResolveSymbolQuery) -> SymbolFuture<'_>;
}

impl<P: SearchPort> crate::Porcelain<P> {
    /// Validate one search and invoke its canonical typed operation.
    pub async fn search(&self, input: SearchInput) -> Result<SearchOutcome, SearchError> {
        if input.file.is_some() {
            return self.code_lookup(input).await;
        }
        for (field, present) in [
            ("symbol", input.symbol.is_some()),
            ("line", input.line.is_some()),
        ] {
            if present {
                return Err(code_lookup_refusal(field, "requires a file"));
            }
        }
        let request = SearchQuery {
            protocol_version: Some(SDK_PROTOCOL_VERSION),
            cursor: input.cursor,
            exclude_terminal: !input.include_terminal.unwrap_or(false),
            text: input.text,
            node_types: input.kind,
            limit: input.limit.unwrap_or(QUERY_DEFAULT_LIMIT),
        };
        request
            .validate()
            .map_err(|error| SearchError::InvalidOptions {
                reason: error.to_string(),
            })?;
        let response = self.port.search(request).await?;
        Ok(SearchOutcome {
            response,
            file: None,
            symbol: None,
            matches: None,
        })
    }

    /// Returns the Rules bound in one file, with their code sites.
    #[rule("rule_search_file_returns_bound_rules")]
    async fn code_lookup(&self, input: SearchInput) -> Result<SearchOutcome, SearchError> {
        let request = code_lookup_request(input)?;
        let response = self.port.resolve_symbol(request).await?;
        let QueryResponse {
            protocol_version,
            operation,
            stamp,
            freshness_error,
            freshness_cause,
            result,
        } = response;
        Ok(SearchOutcome {
            response: QueryResponse {
                protocol_version,
                operation,
                stamp,
                freshness_error,
                freshness_cause,
                result: SearchResult {
                    next_cursor: result.next_cursor,
                    limit: result.limit,
                    has_more: result.has_more,
                    nodes: result.rules,
                },
            },
            file: Some(result.file.into_string()),
            symbol: result.symbol,
            matches: Some(result.matches),
        })
    }
}

/// Refuses the search filters that a code lookup cannot apply.
#[rule("rule_search_file_refuses_other_filters")]
fn code_lookup_request(input: SearchInput) -> Result<ResolveSymbolQuery, SearchError> {
    if input.text.is_some() {
        return Err(code_lookup_refusal(
            "text",
            "a search with a file cannot also have text",
        ));
    }
    if input.kind.iter().any(|kind| *kind != NodeType::Rule) {
        return Err(code_lookup_refusal(
            "kind",
            "a search with a file returns only Rules",
        ));
    }
    if input.include_terminal == Some(false) {
        return Err(code_lookup_refusal(
            "include_terminal",
            "a search with a file returns every bound Rule; omit include_terminal or set it to true",
        ));
    }
    let request = ResolveSymbolQuery {
        protocol_version: Some(SDK_PROTOCOL_VERSION),
        file: input.file.unwrap_or_default().into(),
        symbol: input.symbol,
        line: input.line,
        cursor: input.cursor,
        limit: input.limit.unwrap_or(QUERY_DEFAULT_LIMIT),
    };
    request
        .validate()
        .map_err(|error| code_lookup_refusal("limit", &error.to_string()))?;
    Ok(request)
}

fn code_lookup_refusal(field: &'static str, reason: &str) -> SearchError {
    SearchError::InvalidCodeLookup {
        field,
        reason: reason.to_owned(),
    }
}

fn schema<T: JsonSchema>(contract: schemars::generate::Contract) -> Value {
    serde_json::to_value(
        schemars::generate::SchemaSettings::draft2020_12()
            .with(|settings| settings.contract = contract)
            .into_generator()
            .into_root_schema_for::<T>(),
    )
    .expect("typed search schema is JSON")
}

/// The schema of the request that the search service deserializes.
pub fn input_schema() -> Value {
    schema::<SearchInput>(schemars::generate::Contract::Deserialize)
}

/// The schema of the page that the search service emits.
pub fn output_schema() -> Value {
    schema::<SearchOutcome>(schemars::generate::Contract::Serialize)
}

/// Render one bounded search page without repeating full record payloads.
#[must_use]
pub fn render_readable(outcome: &SearchOutcome) -> String {
    let response = &outcome.response;
    let result = &response.result;
    let lookup = outcome.file.as_ref().map_or_else(String::new, |file| {
        let symbol = outcome
            .symbol
            .as_ref()
            .map_or_else(String::new, |symbol| format!(" symbol {symbol}"));
        format!(" for {file}{symbol}")
    });
    let heading = format!("search: {} returned{lookup}", result.nodes.len());
    let mut lines = vec![heading];
    for node in &result.nodes {
        lines.push(format!(
            "- {} {}{}\n  {}",
            node.node_type().as_str(),
            node.id().as_str(),
            lifecycle_suffix(node),
            node.searchable_text().get(1).copied().unwrap_or("")
        ));
        for site in outcome
            .matches
            .iter()
            .flatten()
            .filter(|site| site.rule_id.as_str() == node.id().as_str())
        {
            lines.push(format!("  {}", site_line(site)));
        }
    }
    lines.push(format!(
        "bounds: limit={} has_more={} continuation={}",
        result.limit,
        result.has_more,
        result.next_cursor.as_deref().unwrap_or("none")
    ));
    if let Some(error) = &response.freshness_error {
        lines.push(format!("warning: freshness: {error}"));
    }
    lines.join("\n")
}

fn site_line(site: &ResolveSymbolMatch) -> String {
    let role = match site.role {
        provenance_core::protocol::ResolveSymbolRole::Implementation => "implementation",
        provenance_core::protocol::ResolveSymbolRole::Verification => "verification",
    };
    let place = site
        .line
        .map_or_else(|| "typed binding".to_owned(), |line| format!("line {line}"));
    let match_kind = match site.match_kind {
        provenance_core::protocol::ResolveSymbolMatchKind::Symbol => "symbol",
        provenance_core::protocol::ResolveSymbolMatchKind::File => "file",
    };
    let mut parts = vec![format!("{role} {place}")];
    parts.extend(site.item_name.clone());
    parts.extend(site.verification_method.clone());
    format!("· {} ({match_kind} match)", parts.join(" "))
}

/// Names the stored lifecycle state in shared readable search output.
#[rule("rule_search_readable_lifecycle")]
const fn lifecycle_suffix(node: &GraphNode) -> &'static str {
    match node {
        GraphNode::Requirement(record) => requirement_suffix(&record.status),
        GraphNode::Resolution(record) => resolution_suffix(&record.status),
        GraphNode::Rule(record) => rule_suffix(&record.status),
        GraphNode::Topic(record) => topic_suffix(record.status),
        GraphNode::Question(record) => question_suffix(record.status),
        GraphNode::Source(_) | GraphNode::Domain(_) => "",
    }
}

const fn requirement_suffix(status: &RequirementStatus) -> &'static str {
    match status {
        RequirementStatus::Active => " · active",
        RequirementStatus::Discovery => " · discovery",
        RequirementStatus::Refinement => " · refinement",
        RequirementStatus::Resolved => " · resolved",
        RequirementStatus::Archived => " · archived",
    }
}

const fn resolution_suffix(status: &ResolutionStatus) -> &'static str {
    match status {
        ResolutionStatus::Draft => " · draft",
        ResolutionStatus::Review => " · review",
        ResolutionStatus::Proposed => " · proposed",
        ResolutionStatus::Approved => " · approved",
        ResolutionStatus::Rejected => " · rejected",
        ResolutionStatus::Revised => " · revised",
        ResolutionStatus::Superseded => " · superseded",
        ResolutionStatus::Abandoned => " · abandoned",
        ResolutionStatus::Archived => " · archived",
    }
}

const fn rule_suffix(status: &RuleStatus) -> &'static str {
    match status {
        RuleStatus::Draft => " · draft",
        RuleStatus::Review => " · review",
        RuleStatus::Active => " · active",
        RuleStatus::Deprecated => " · deprecated",
        RuleStatus::Archived => " · archived",
    }
}

const fn topic_suffix(status: TopicStatus) -> &'static str {
    match status {
        TopicStatus::Open => " · open",
        TopicStatus::Explored => " · explored",
        TopicStatus::Closed => " · closed",
        TopicStatus::Archived => " · archived",
    }
}

const fn question_suffix(status: QuestionStatus) -> &'static str {
    match status {
        QuestionStatus::Open => " · open",
        QuestionStatus::BlockedOnHuman => " · blocked_on_human",
        QuestionStatus::Answered => " · answered",
        QuestionStatus::Archived => " · archived",
    }
}
