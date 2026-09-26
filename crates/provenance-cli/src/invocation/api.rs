//! CLI argv bindings for the shared api action.

use super::grammar::ApiArgs;
use crate::catalog_cli::usage_error;
use provenance_porcelain::api::{ApiArguments, ApiError, ApiRequest};
use std::collections::BTreeMap;
use std::io::Read as _;

/// Translate one parsed api command into the shared semantic request.
///
/// The method flag and the `--input` file or stdin body carry the whole
/// mutation selection. The options are checked before the body is read, so a
/// refused call never waits on standard input.
#[provenance_macros::rule("rule_porcelain_api_body_inputs")]
pub fn request(args: &ApiArgs) -> Result<ApiRequest, ApiError> {
    let arguments = ApiArguments {
        path: args.path.clone(),
        method: args.method,
        query: pairs(&args.queries, '=', "query", "query options use NAME=VALUE")?,
        headers: pairs(&args.headers, ':', "headers", "headers use NAME: VALUE")?,
        body: args.input.as_ref().map(|_| serde_json::Map::new()),
    };
    let mut request = ApiRequest::from(arguments)?;
    if let (ApiRequest::Invoke(input), Some(source)) = (&mut request, args.input.as_deref()) {
        input.body = Some(read_body(source));
    }
    Ok(request)
}

fn pairs(
    raw: &[String],
    separator: char,
    field: &str,
    usage: &str,
) -> Result<BTreeMap<String, String>, ApiError> {
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
            return Err(ApiError::invalid_options(Some(field)));
        }
    }
    Ok(pairs)
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
    let request = match request(&args) {
        Ok(request) => request,
        Err(error) => anyhow::bail!("{}", serde_json::to_string(&error.failure)?),
    };
    provenance_cli::porcelain::dispatch_api(
        &args.common.repo,
        &args.common.scope,
        args.common.format(),
        request,
    )
    .await
}
