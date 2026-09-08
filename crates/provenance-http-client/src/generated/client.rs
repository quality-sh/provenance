// Generated from OpenAPI. Do not edit.
use crate::types::{
    CheckStatementFailureOutput, CheckStatementRequestInput, CheckStatementSuccessOutput,
};
pub const PROTOCOL_VERSION: u32 = 7;
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid HTTP host URL")]
    InvalidUrl,
    #[error("HTTP transport failed: {0}")]
    Transport(reqwest::Error),
    #[error("metadata request failed with status {0}")]
    MetadataStatus(u16),
    #[error("incompatible operation protocol: expected {expected}, received {received}")]
    ProtocolMismatch { expected: u32, received: u32 },
    #[error("operation refused with status {status}")]
    Operation {
        status: u16,
        failure: CheckStatementFailureOutput,
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
            let failure = response.json().await.map_err(Error::Transport)?;
            return Err(Error::Operation {
                status: status.as_u16(),
                failure,
            });
        }
        response.json().await.map_err(Error::Transport)
    }
}
