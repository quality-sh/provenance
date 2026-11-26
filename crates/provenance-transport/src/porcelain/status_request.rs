//! The status request that a create or an update of a Rule or a Requirement
//! opened, as the CLI and MCP show it.

use provenance_core::{DispositionActorRefusal, NodeType};
use provenance_porcelain::api::{ApiInput, ApiMethod, ApiOutcome, ApiRequest};
use serde_json::Value;

/// A status request that a write opened.
pub struct OpenStatusRequest {
    /// The decision state of the record after the write. Its
    /// `pending.status_request` names the requested status.
    pub decision: Value,
    /// The text for the reader: the request, the call that accepts it, the
    /// status that the record keeps until then, and an empty reviewer list.
    pub note: String,
}

impl crate::StatementHost {
    /// The status request that a write of `target` opened when it asked for
    /// the status `requested`. A write that has the status that it asked for
    /// opened no request.
    pub async fn open_status_request(
        &self,
        kind: NodeType,
        target: &str,
        requested: &str,
        written: &Value,
    ) -> Option<OpenStatusRequest> {
        let collection = match kind {
            NodeType::Rule => "rules",
            NodeType::Requirement => "requirements",
            _ => return None,
        };
        let record = written.get("data").unwrap_or(written);
        if record.get("status").and_then(Value::as_str) == Some(requested) {
            return None;
        }
        let read = ApiRequest::Invoke(ApiInput {
            method: ApiMethod::Get,
            path: format!("/{collection}/{target}"),
            query: std::collections::BTreeMap::new(),
            headers: std::collections::BTreeMap::new(),
            body: None,
        });
        let Ok(ApiOutcome::Invoked(current)) = self.porcelain().api().execute_api(read).await
        else {
            return None;
        };
        let decision = current["data"]["decision"].clone();
        let pending = &decision["pending"];
        if pending["status_request"]["status"].as_str() != Some(requested) {
            return None;
        }
        let proposal = pending["proposal_id"].as_str()?;
        let kept = current["data"]["status"].as_str()?;
        let empty_list = if self.reviewer_ids().is_some_and(|ids| ids.is_empty()) {
            format!(
                "{}\n",
                DispositionActorRefusal::NoActorsConfigured(provenance_core::IdentityType::Human)
            )
        } else {
            String::new()
        };
        let note = format!(
            "Status request {proposal} is open. The status of {target} changes to {requested} \
             when a reviewer on the reviewer list accepts the request with `provenance api \
             {collection}/{target}/submissions/{proposal}/decide --method post --input -`. \
             Until then, {target} keeps the status {kept}.\n{empty_list}"
        );
        Some(OpenStatusRequest { decision, note })
    }
}
