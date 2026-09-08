// Generated from OpenAPI. Do not edit.
use crate::types::{
    ApplyFailureOutput, ApplyRequestInput, ApplySuccessOutput, BeginVerificationFailureOutput,
    BeginVerificationRequestInput, BeginVerificationSuccessOutput, CheckStatementFailureOutput,
    CheckStatementRequestInput, CheckStatementSuccessOutput, CompleteVerificationFailureOutput,
    CompleteVerificationRequestInput, CompleteVerificationSuccessOutput, EvidenceFailureOutput,
    EvidenceRequestInput, EvidenceSuccessOutput, GetFailureOutput, GetRequestInput,
    GetSuccessOutput, ImpactFailureOutput, ImpactRequestInput, ImpactSuccessOutput,
    InfoFailureOutput, InfoRequestInput, InfoSuccessOutput, NeighborsFailureOutput,
    NeighborsRequestInput, NeighborsSuccessOutput, PlanFailureOutput, PlanRequestInput,
    PlanSuccessOutput, ResolveSymbolFailureOutput, ResolveSymbolRequestInput,
    ResolveSymbolSuccessOutput, SearchFailureOutput, SearchRequestInput, SearchSuccessOutput,
    StaleFailureOutput, StaleRequestInput, StaleSuccessOutput, TraceFailureOutput,
    TraceRequestInput, TraceSuccessOutput, VerificationBindingsFailureOutput,
    VerificationBindingsRequestInput, VerificationBindingsSuccessOutput,
    VerificationRunsFailureOutput, VerificationRunsRequestInput, VerificationRunsSuccessOutput,
};
use crate::{runtime, Error};
pub const PROTOCOL_VERSION: u32 = 7;
#[derive(Debug, serde::Serialize)]
#[serde(untagged)]
pub enum OperationFailure {
    Apply(Box<ApplyFailureOutput>),
    BeginVerification(Box<BeginVerificationFailureOutput>),
    CheckStatement(Box<CheckStatementFailureOutput>),
    CompleteVerification(Box<CompleteVerificationFailureOutput>),
    Evidence(Box<EvidenceFailureOutput>),
    Get(Box<GetFailureOutput>),
    Impact(Box<ImpactFailureOutput>),
    Info(Box<InfoFailureOutput>),
    Neighbors(Box<NeighborsFailureOutput>),
    Plan(Box<PlanFailureOutput>),
    ResolveSymbol(Box<ResolveSymbolFailureOutput>),
    Search(Box<SearchFailureOutput>),
    Stale(Box<StaleFailureOutput>),
    Trace(Box<TraceFailureOutput>),
    VerificationBindings(Box<VerificationBindingsFailureOutput>),
    VerificationRuns(Box<VerificationRunsFailureOutput>),
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
                .map_err(|cause| runtime::connection("metadata", false, cause))?,
        };
        let response = client
            .http
            .get(format!("{}/metadata", client.base_url))
            .send()
            .await
            .map_err(|cause| runtime::connection("metadata", false, cause))?;
        if !response.status().is_success() {
            return Err(runtime::metadata_status());
        }
        let value = runtime::read_json(response, "metadata", false).await?;
        runtime::validate(&value, "MetadataOutput", "metadata", false)?;
        let metadata: Metadata = runtime::decode(value, "metadata", false)?;
        if metadata.protocol_version != PROTOCOL_VERSION {
            return Err(Error::ProtocolMismatch {
                expected: PROTOCOL_VERSION,
                received: metadata.protocol_version,
            });
        }
        Ok(client)
    }
}
include!("operations/apply.rs");
include!("operations/begin_verification.rs");
include!("operations/check_statement.rs");
include!("operations/complete_verification.rs");
include!("operations/evidence.rs");
include!("operations/get.rs");
include!("operations/impact.rs");
include!("operations/info.rs");
include!("operations/neighbors.rs");
include!("operations/plan.rs");
include!("operations/resolve_symbol.rs");
include!("operations/search.rs");
include!("operations/stale.rs");
include!("operations/trace.rs");
include!("operations/verification_bindings.rs");
include!("operations/verification_runs.rs");
