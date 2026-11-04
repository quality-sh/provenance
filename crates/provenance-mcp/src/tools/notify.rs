//! The notify action of the change tool: register a client thread for the review changes
//! of one record, or end that registration.

use provenance_porcelain::get::{NotifyInput, NotifyResult, ReadError, ViewResult};
use provenance_transport::StatementHost;
use rmcp::model::{CallToolResult, ContentBlock};
use serde_json::{Map, Value};

/// Sends the notify request through the same get operation as the CLI notify option.
pub async fn call(host: &StatementHost, arguments: Map<String, Value>) -> CallToolResult {
    let Ok(input) = serde_json::from_value::<NotifyInput>(Value::Object(arguments)) else {
        return crate::surface::envelope_error(&ReadError::InvalidOptions.failure());
    };
    match host.porcelain().get().get(input.into()).await {
        Ok(outcome) => {
            let ViewResult::Review(review) = outcome.result else {
                unreachable!("a notify request reads the review view");
            };
            let notify = review.notify.expect("a review view with notify registers");
            let value = serde_json::to_value(NotifyResult { notify }).expect("notify is JSON");
            let mut result = CallToolResult::structured(value.clone());
            result.content = vec![ContentBlock::text(value.to_string())];
            result
        }
        Err(error) => crate::surface::envelope_error(&error.failure()),
    }
}
