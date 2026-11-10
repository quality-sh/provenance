//! CLI argv bindings for the shared api action.

use super::grammar::ApiArgs;
use crate::catalog_cli::usage_error;
use provenance_porcelain::api::{ApiArguments, ApiError, ApiRequest};
use std::collections::BTreeMap;
use std::io::Read as _;

/// One refused api command, with the correction when the CLI knows it.
pub struct Refused {
    pub error: ApiError,
    pub hint: Option<String>,
}

impl From<ApiError> for Refused {
    fn from(error: ApiError) -> Self {
        Self { error, hint: None }
    }
}

/// Translate one parsed api command into the shared semantic request.
///
/// The method flag and the `--input` file or stdin body carry the whole
/// mutation selection. The options are checked before the body is read, so a
/// refused call never waits on standard input. The headers are checked
/// before the queries, so a repeated header wins over a repeated query.
#[provenance_macros::rule("rule_porcelain_api_body_inputs")]
pub fn request(args: &ApiArgs) -> Result<ApiRequest, Refused> {
    let header = pairs(&args.headers, ':', "headers use NAME: VALUE")
        .map_err(|_| ApiError::invalid_options(Some("header")))?;
    let query =
        pairs(&args.queries, '=', "query options use NAME=VALUE").map_err(|name| Refused {
            error: ApiError::invalid_options(Some("query")),
            hint: Some(repeated_query_hint(args, &name)),
        })?;
    let arguments = ApiArguments {
        path: args.path.clone(),
        method: args.method,
        input: args.input.as_ref().map(|_| serde_json::Map::new()),
        header,
        query,
        filter: args.filter.clone(),
        limit: args.limit,
    };
    let mut request = ApiRequest::from(arguments)?;
    if let (ApiRequest::Invoke(input), Some(source)) = (&mut request, args.input.as_deref()) {
        input.body = Some(read_body(source));
    }
    Ok(request)
}

/// Reads the `NAME<separator>VALUE` options, or gives the name that occurs
/// more than one time.
fn pairs(raw: &[String], separator: char, usage: &str) -> Result<BTreeMap<String, String>, String> {
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
            return Err(name.to_owned());
        }
    }
    Ok(pairs)
}

/// The correction for a query parameter that the caller gave more than once.
///
/// The api takes each query parameter one time. Only a parameter whose
/// query schema is an array takes its items as one comma-separated value.
fn repeated_query_hint(args: &ApiArgs, name: &str) -> String {
    let once = format!("--query {name} occurs more than one time; give each parameter one time");
    let selector = args.queries.iter().find_map(|entry| {
        entry
            .split_once('=')
            .filter(|(key, _)| key.trim() == "query")
            .map(|(_, value)| value.trim())
    });
    let takes_list = args.path.as_deref().is_some_and(|path| {
        provenance_transport::porcelain::api_port::query_takes_list(
            args.method.unwrap_or_default(),
            path,
            selector,
            name,
        )
    });
    if takes_list {
        format!(
            "{once} and put the items of a list in one comma-separated value, \
             for example --query {name}=a,b"
        )
    } else {
        once
    }
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
        Err(refused) => {
            return Err(crate::catalog_cli::refusal::HostRefusal {
                failure: serde_json::to_value(&refused.error.failure)?,
                hint: refused.hint,
                text_shows_failure: true,
            }
            .into())
        }
    };
    provenance_cli::porcelain::dispatch_api(
        &args.common.repo,
        &args.common.scope,
        args.common.format(),
        request,
    )
    .await
}
