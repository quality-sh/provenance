//! Builds reviewer-facing URLs from the running host and existing graph reads.

use provenance_cli::repo_context::RepoContext;
use provenance_core::NodeType;
use provenance_porcelain::get::{GetInput, View};
use provenance_transport::StatementHost;
use serde::Serialize;
use serde_json::Value;
use std::{net::ToSocketAddrs as _, time::Duration};

#[derive(Serialize)]
struct LinkOutput {
    review_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

pub async fn print(
    context: &RepoContext,
    record_id: &str,
    format: Option<provenance_cli::porcelain::OutputFormat>,
) -> anyhow::Result<()> {
    let host = context.local_host()?;
    let output = link_output(context, &host, record_id).await?;
    if format == Some(provenance_cli::porcelain::OutputFormat::Json) {
        crate::output::print_json(&output)
    } else {
        println!(
            "{}",
            output
                .review_url
                .as_deref()
                .or(output.message.as_deref())
                .unwrap()
        );
        Ok(())
    }
}

pub async fn annotate_write(
    context: &RepoContext,
    host: &StatementHost,
    value: &mut Value,
    record_write: bool,
) -> anyhow::Result<()> {
    let Some(record_id) = affected_record(value, record_write).map(str::to_owned) else {
        return Ok(());
    };
    let output = link_output(context, host, &record_id).await?;
    if let Some(url) = output.review_url {
        value["data"]["review_url"] = Value::String(url);
    } else if let Some(message) = output.message {
        value["data"]["review_message"] = Value::String(message);
    }
    Ok(())
}

async fn link_output(
    context: &RepoContext,
    host: &StatementHost,
    record_id: &str,
) -> anyhow::Result<LinkOutput> {
    let root = containing_requirement(host, record_id).await?;
    let Some(runtime) = crate::review_runtime::read(context.repo.as_std_path(), &context.scope)?
        .filter(host_is_running)
    else {
        return Ok(LinkOutput {
            review_url: None,
            message: Some(start_message(context)),
        });
    };
    let focus = (root != record_id).then_some(record_id);
    Ok(LinkOutput {
        review_url: Some(build_url(&runtime.endpoint, &root, focus)?),
        message: None,
    })
}

async fn containing_requirement(host: &StatementHost, record_id: &str) -> anyhow::Result<String> {
    let get = host.porcelain().get();
    let record = get.get(GetInput::new(record_id, View::Record)).await?;
    if record.record.node_type() == NodeType::Requirement {
        return Ok(record_id.to_owned());
    }
    let mut requirements = Vec::new();
    for view in [View::Grounding, View::Children] {
        let mut input = GetInput::new(record_id, view);
        input.max_depth = Some(provenance_core::protocol::TRACE_MAX_DEPTH);
        input.returned_kinds = vec![NodeType::Requirement];
        input.limit = Some(provenance_core::protocol::QUERY_MAX_LIMIT);
        requirements.extend(
            get.get(input)
                .await?
                .related()
                .iter()
                .map(|node| node.node.id().as_str().to_owned()),
        );
    }
    requirements.sort();
    requirements.dedup();
    requirements.into_iter().next().ok_or_else(|| {
        anyhow::anyhow!("record {record_id} is not in a Requirement review document")
    })
}

fn affected_record(value: &Value, record_write: bool) -> Option<&str> {
    let data = value.get("data")?;
    if !record_write
        && !data
            .pointer("/decision/pending")
            .is_some_and(|pending| !pending.is_null())
        && !matches!(
            data.get("fact").and_then(Value::as_str),
            Some("submitted" | "decided" | "withdrawn")
        )
    {
        return None;
    }
    data.get("id")
        .or_else(|| data.get("requirement_id"))
        .and_then(Value::as_str)
}

fn host_is_running(runtime: &crate::review_runtime::RunningHost) -> bool {
    let Ok(url) = url::Url::parse(&runtime.endpoint) else {
        return false;
    };
    let Some(port) = url.port_or_known_default() else {
        return false;
    };
    let Some(host) = url.host_str() else {
        return false;
    };
    (host, port)
        .to_socket_addrs()
        .ok()
        .and_then(|mut addresses| addresses.next())
        .is_some_and(|address| {
            std::net::TcpStream::connect_timeout(&address, Duration::from_millis(200)).is_ok()
        })
}

fn start_message(context: &RepoContext) -> String {
    format!(
        "No review host is running. Start it with `provenance review --repo {} --repository-id local --scope {}`.",
        context.repo, context.scope
    )
}

fn build_url(endpoint: &str, root: &str, focus: Option<&str>) -> anyhow::Result<String> {
    let mut url = url::Url::parse(endpoint)?;
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("root", root);
        if let Some(focus) = focus {
            query.append_pair("focus", focus);
        }
    }
    Ok(url.into())
}

#[cfg(test)]
mod tests {
    use super::build_url;

    #[test]
    fn link_builder_encodes_record_ids_and_never_adds_a_credential() {
        let url = build_url("http://127.0.0.1:1234/", "req root", Some("rule/focus")).unwrap();
        assert_eq!(
            url,
            "http://127.0.0.1:1234/?root=req+root&focus=rule%2Ffocus"
        );
        assert!(!url.contains("bearer"));
    }
}
