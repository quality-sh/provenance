// Generated from OpenAPI. Do not edit.
use crate::types::{
    CheckStatementFailureOutput, CheckStatementRequestInput, CheckStatementSuccessOutput,
    EvidenceFailureOutput, EvidenceRequestInput, EvidenceSuccessOutput, GetFailureOutput,
    GetRequestInput, GetSuccessOutput, ImpactFailureOutput, ImpactRequestInput,
    ImpactSuccessOutput, InfoFailureOutput, InfoRequestInput, InfoSuccessOutput,
    NeighborsFailureOutput, NeighborsRequestInput, NeighborsSuccessOutput,
    ResolveSymbolFailureOutput, ResolveSymbolRequestInput, ResolveSymbolSuccessOutput,
    SearchFailureOutput, SearchRequestInput, SearchSuccessOutput, StaleFailureOutput,
    StaleRequestInput, StaleSuccessOutput, TraceFailureOutput, TraceRequestInput,
    TraceSuccessOutput, VerificationBindingsFailureOutput, VerificationBindingsRequestInput,
    VerificationBindingsSuccessOutput, VerificationRunsFailureOutput, VerificationRunsRequestInput,
    VerificationRunsSuccessOutput,
};
pub const PROTOCOL_VERSION: u32 = 7;
#[derive(Debug, serde::Serialize)]
#[serde(untagged)]
pub enum OperationFailure {
    CheckStatement(Box<CheckStatementFailureOutput>),
    Evidence(Box<EvidenceFailureOutput>),
    Get(Box<GetFailureOutput>),
    Impact(Box<ImpactFailureOutput>),
    Info(Box<InfoFailureOutput>),
    Neighbors(Box<NeighborsFailureOutput>),
    ResolveSymbol(Box<ResolveSymbolFailureOutput>),
    Search(Box<SearchFailureOutput>),
    Stale(Box<StaleFailureOutput>),
    Trace(Box<TraceFailureOutput>),
    VerificationBindings(Box<VerificationBindingsFailureOutput>),
    VerificationRuns(Box<VerificationRunsFailureOutput>),
}
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid HTTP host URL")]
    InvalidUrl,
    #[error("invalid bearer credential format")]
    InvalidCredentials,
    #[error("HTTP transport failed: {0}")]
    Transport(reqwest::Error),
    #[error("metadata request failed with status {0}")]
    MetadataStatus(u16),
    #[error("incompatible operation protocol: expected {expected}, received {received}")]
    ProtocolMismatch { expected: u32, received: u32 },
    #[error("operation refused with status {status}")]
    Operation {
        status: u16,
        failure: OperationFailure,
    },
}
#[derive(serde::Deserialize)]
struct Metadata {
    protocol_version: u32,
}
#[derive(Clone)]
pub struct HttpClient {
    base_url: String,
    http: reqwest::Client,
}
impl HttpClient {
    pub async fn connect(base_url: &str) -> Result<Self, Error> {
        Self::connect_with_headers(base_url, reqwest::header::HeaderMap::new()).await
    }
    pub async fn connect_with_bearer(base_url: &str, bearer: &str) -> Result<Self, Error> {
        let mut value = reqwest::header::HeaderValue::from_str(&format!("Bearer {bearer}"))
            .map_err(|_| Error::InvalidCredentials)?;
        value.set_sensitive(true);
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(reqwest::header::AUTHORIZATION, value);
        Self::connect_with_headers(base_url, headers).await
    }
    async fn connect_with_headers(
        base_url: &str,
        headers: reqwest::header::HeaderMap,
    ) -> Result<Self, Error> {
        let url = reqwest::Url::parse(base_url).map_err(|_| Error::InvalidUrl)?;
        if !matches!(url.scheme(), "http" | "https")
            || url.query().is_some()
            || url.fragment().is_some()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err(Error::InvalidUrl);
        }
        let client = Self {
            base_url: base_url.trim_end_matches('/').to_owned(),
            http: reqwest::Client::builder()
                .default_headers(headers)
                .retry(reqwest::retry::never())
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(Error::Transport)?,
        };
        let response = client
            .http
            .get(format!("{}/metadata", client.base_url))
            .send()
            .await
            .map_err(Error::Transport)?;
        if !response.status().is_success() {
            return Err(Error::MetadataStatus(response.status().as_u16()));
        }
        let metadata: Metadata = response.json().await.map_err(Error::Transport)?;
        if metadata.protocol_version != PROTOCOL_VERSION {
            return Err(Error::ProtocolMismatch {
                expected: PROTOCOL_VERSION,
                received: metadata.protocol_version,
            });
        }
        Ok(client)
    }
}
include!("operations/check_statement.rs");
include!("operations/evidence.rs");
include!("operations/get.rs");
include!("operations/impact.rs");
include!("operations/info.rs");
include!("operations/neighbors.rs");
include!("operations/resolve_symbol.rs");
include!("operations/search.rs");
include!("operations/stale.rs");
include!("operations/trace.rs");
include!("operations/verification_bindings.rs");
include!("operations/verification_runs.rs");
