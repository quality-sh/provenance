// Generated from OpenAPI. Do not edit.
use crate::types::{
    CheckStatementFailureOutput, CheckStatementRequestInput, CheckStatementSuccessOutput,
    GetFailureOutput, GetRequestInput, GetSuccessOutput, InfoFailureOutput, InfoRequestInput,
    InfoSuccessOutput, NeighborsFailureOutput, NeighborsRequestInput, NeighborsSuccessOutput,
    SearchFailureOutput, SearchRequestInput, SearchSuccessOutput, TraceFailureOutput,
    TraceRequestInput, TraceSuccessOutput,
};
pub const PROTOCOL_VERSION: u32 = 7;
#[derive(Debug, serde::Serialize)]
#[serde(untagged)]
pub enum OperationFailure {
    CheckStatement(Box<CheckStatementFailureOutput>),
    Get(Box<GetFailureOutput>),
    Info(Box<InfoFailureOutput>),
    Neighbors(Box<NeighborsFailureOutput>),
    Search(Box<SearchFailureOutput>),
    Trace(Box<TraceFailureOutput>),
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
    pub async fn check_statement(
        &self,
        call: &CheckStatementRequestInput,
    ) -> Result<CheckStatementSuccessOutput, Error> {
        let response = self
            .http
            .post(format!("{}/v7/operations/check-statement", self.base_url))
            .json(call)
            .send()
            .await
            .map_err(Error::Transport)?;
        let status = response.status();
        if !status.is_success() {
            let failure: CheckStatementFailureOutput =
                response.json().await.map_err(Error::Transport)?;
            return Err(Error::Operation {
                status: status.as_u16(),
                failure: OperationFailure::CheckStatement(Box::new(failure)),
            });
        }
        response.json().await.map_err(Error::Transport)
    }
    pub async fn get(&self, call: &GetRequestInput) -> Result<GetSuccessOutput, Error> {
        let response = self
            .http
            .post(format!("{}/v7/operations/get", self.base_url))
            .json(call)
            .send()
            .await
            .map_err(Error::Transport)?;
        let status = response.status();
        if !status.is_success() {
            let failure: GetFailureOutput = response.json().await.map_err(Error::Transport)?;
            return Err(Error::Operation {
                status: status.as_u16(),
                failure: OperationFailure::Get(Box::new(failure)),
            });
        }
        response.json().await.map_err(Error::Transport)
    }
    pub async fn info(&self, call: &InfoRequestInput) -> Result<InfoSuccessOutput, Error> {
        let response = self
            .http
            .post(format!("{}/v7/operations/info", self.base_url))
            .json(call)
            .send()
            .await
            .map_err(Error::Transport)?;
        let status = response.status();
        if !status.is_success() {
            let failure: InfoFailureOutput = response.json().await.map_err(Error::Transport)?;
            return Err(Error::Operation {
                status: status.as_u16(),
                failure: OperationFailure::Info(Box::new(failure)),
            });
        }
        response.json().await.map_err(Error::Transport)
    }
    pub async fn neighbors(
        &self,
        call: &NeighborsRequestInput,
    ) -> Result<NeighborsSuccessOutput, Error> {
        let response = self
            .http
            .post(format!("{}/v7/operations/neighbors", self.base_url))
            .json(call)
            .send()
            .await
            .map_err(Error::Transport)?;
        let status = response.status();
        if !status.is_success() {
            let failure: NeighborsFailureOutput =
                response.json().await.map_err(Error::Transport)?;
            return Err(Error::Operation {
                status: status.as_u16(),
                failure: OperationFailure::Neighbors(Box::new(failure)),
            });
        }
        response.json().await.map_err(Error::Transport)
    }
    pub async fn search(&self, call: &SearchRequestInput) -> Result<SearchSuccessOutput, Error> {
        let response = self
            .http
            .post(format!("{}/v7/operations/search", self.base_url))
            .json(call)
            .send()
            .await
            .map_err(Error::Transport)?;
        let status = response.status();
        if !status.is_success() {
            let failure: SearchFailureOutput = response.json().await.map_err(Error::Transport)?;
            return Err(Error::Operation {
                status: status.as_u16(),
                failure: OperationFailure::Search(Box::new(failure)),
            });
        }
        response.json().await.map_err(Error::Transport)
    }
    pub async fn trace(&self, call: &TraceRequestInput) -> Result<TraceSuccessOutput, Error> {
        let response = self
            .http
            .post(format!("{}/v7/operations/trace", self.base_url))
            .json(call)
            .send()
            .await
            .map_err(Error::Transport)?;
        let status = response.status();
        if !status.is_success() {
            let failure: TraceFailureOutput = response.json().await.map_err(Error::Transport)?;
            return Err(Error::Operation {
                status: status.as_u16(),
                failure: OperationFailure::Trace(Box::new(failure)),
            });
        }
        response.json().await.map_err(Error::Transport)
    }
}
