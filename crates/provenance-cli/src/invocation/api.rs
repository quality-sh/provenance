//! CLI argv bindings for the shared api action.

use super::grammar::ApiArgs;
use crate::catalog_cli::refusal::{usage, HostRefusal};
use provenance_porcelain::api::{ApiArguments, ApiError, ApiRequest};
use std::collections::BTreeMap;
use std::io::Read as _;

/// Translate one parsed api command into the shared semantic request.
///
/// The method flag and the `--input` file or stdin body carry the whole
/// mutation selection. The options are checked before the body is read, so a
/// refused call never waits on standard input. The headers are checked
/// before the queries, so a repeated header wins over a repeated query.
#[provenance_macros::rule("rule_porcelain_api_body_inputs")]
pub fn request(args: &ApiArgs) -> anyhow::Result<ApiRequest> {
    let header = pairs(&args.headers, ':', "headers use NAME: VALUE")?
        .map_err(|_| refused(&ApiError::invalid_options(Some("header")), None))?;
    let query = pairs(&args.queries, '=', "query options use NAME=VALUE")?.map_err(|name| {
        refused(
            &ApiError::invalid_options(Some("query")),
            repeated_query_hint(args, &name),
        )
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
    let mut request = ApiRequest::from(arguments).map_err(|error| refused(&error, None))?;
    if let (ApiRequest::Invoke(input), Some(source)) = (&mut request, args.input.as_deref()) {
        input.body = Some(read_body(source)?);
    }
    Ok(request)
}

/// The host refusal of one api command, with the correction when the CLI knows it.
fn refused(error: &ApiError, hint: Option<String>) -> anyhow::Error {
    match serde_json::to_value(&error.failure) {
        Ok(failure) => HostRefusal {
            failure,
            hint,
            text_shows_failure: true,
        }
        .into(),
        Err(error) => error.into(),
    }
}

/// Reads the `NAME<separator>VALUE` options, or gives the name that occurs
/// more than one time.
fn pairs(
    raw: &[String],
    separator: char,
    form: &str,
) -> anyhow::Result<Result<BTreeMap<String, String>, String>> {
    let mut pairs = BTreeMap::new();
    for entry in raw {
        let Some((name, value)) = entry.split_once(separator) else {
            return Err(usage(form));
        };
        let name = name.trim();
        if name.is_empty() {
            return Err(usage(form));
        }
        if pairs
            .insert(name.to_owned(), value.trim_start().to_owned())
            .is_some()
        {
            return Ok(Err(name.to_owned()));
        }
    }
    Ok(Ok(pairs))
}

/// Names a query parameter that the caller gave more than once to a catalog
/// route, and shows one comma-separated value when the parameter takes a list.
///
/// With no path or no route, one use of the parameter also fails, so there is
/// no correction to give. Only a parameter whose query schema is an array
/// takes its items as one comma-separated value.
#[provenance_macros::rule("rule_api_repeated_query_refusal_names_parameter")]
fn repeated_query_hint(args: &ApiArgs, name: &str) -> Option<String> {
    let selector = args.queries.iter().find_map(|entry| {
        entry
            .split_once('=')
            .filter(|(key, _)| key.trim() == "query")
            .map(|(_, value)| value.trim())
    });
    let takes_list = provenance_transport::porcelain::api_port::query_takes_list(
        args.method.unwrap_or_default(),
        args.path.as_deref()?,
        selector,
        name,
    )?;
    let once = format!("--query {name} occurs more than one time; give each parameter one time");
    Some(if takes_list {
        format!(
            "{once} and put the items of a list in one comma-separated value, \
             for example --query {name}=a,b"
        )
    } else {
        once
    })
}

fn read_body(input: &str) -> anyhow::Result<serde_json::Map<String, serde_json::Value>> {
    let text = if input == "-" {
        let mut text = String::new();
        std::io::stdin().read_to_string(&mut text).map_err(usage)?;
        text
    } else {
        std::fs::read_to_string(input).map_err(usage)?
    };
    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|error| usage(format!("--input needs one JSON object: {error}")))?;
    value
        .as_object()
        .cloned()
        .ok_or_else(|| usage("--input needs one JSON object"))
}

pub async fn dispatch(args: ApiArgs) -> anyhow::Result<()> {
    let request = request(&args)?;
    provenance_cli::porcelain::dispatch_api(
        &args.common.repo,
        &args.common.scope,
        args.common.format(),
        request,
    )
    .await
}
