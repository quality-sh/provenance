//! CLI argv bindings for the shared api action.

use super::grammar::ApiArgs;
use crate::catalog_cli::usage_error;
use provenance_porcelain::api::{ApiArguments, ApiRequest};
use std::collections::BTreeMap;
use std::io::Read as _;

/// Translate one parsed api command into the shared semantic request.
///
/// The method flag and the `--input` file or stdin body carry the whole
/// mutation selection; malformed pairs are refused before any call.
#[provenance_macros::rule("rule_porcelain_api_body_inputs")]
pub fn request(args: &ApiArgs) -> ApiRequest {
    let arguments = ApiArguments {
        path: args.path.clone(),
        method: args.path.as_ref().map(|_| args.method),
        query: pairs(&args.queries, '=', "query options use NAME=VALUE"),
        headers: pairs(&args.headers, ':', "headers use NAME: VALUE"),
        body: args.input.as_deref().map(read_body),
    };
    ApiRequest::from(arguments).unwrap_or_else(|error| usage_error(error))
}

fn pairs(raw: &[String], separator: char, usage: &str) -> BTreeMap<String, String> {
    let mut pairs = BTreeMap::new();
    for entry in raw {
        let Some((name, value)) = entry.split_once(separator) else {
            usage_error(anyhow::anyhow!("{usage}"));
        };
        let name = name.trim();
        if name.is_empty() {
            usage_error(anyhow::anyhow!("{usage}"));
        }
        if pairs
            .insert(name.to_owned(), value.trim_start().to_owned())
            .is_some()
        {
            usage_error(anyhow::anyhow!("{usage}"));
        }
    }
    pairs
}

fn read_body(input: &str) -> serde_json::Map<String, serde_json::Value> {
    let text = if input == "-" {
        let mut text = String::new();
        std::io::stdin()
            .read_to_string(&mut text)
            .unwrap_or_else(|error| usage_error(error));
        text
    } else {
        std::fs::read_to_string(input).unwrap_or_else(|error| usage_error(error))
    };
    let value: serde_json::Value = serde_json::from_str(&text).unwrap_or_else(|error| {
        usage_error(anyhow::anyhow!("--input needs one JSON object: {error}"))
    });
    value
        .as_object()
        .cloned()
        .unwrap_or_else(|| usage_error(anyhow::anyhow!("--input needs one JSON object")))
}

pub async fn dispatch(args: ApiArgs) -> anyhow::Result<()> {
    provenance_cli::porcelain::dispatch_api(
        &args.common.repo,
        &args.common.scope,
        args.common.format(),
        request(&args),
    )
    .await
}
