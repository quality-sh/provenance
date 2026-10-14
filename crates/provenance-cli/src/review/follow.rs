//! Native review subscription. Reads run in order while the socket retains invalidations.
use super::events::stream::{emit, Baseline};
use anyhow::Context as _;
use futures_util::{SinkExt, StreamExt};
use provenance_cli::repo_context::RepoContext;
use provenance_macros::rule;
use provenance_porcelain::get::GetInput;
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Message};

/// Streams saved document state when the repository host sends an invalidation.
#[rule("rule_review_follow_push")]
pub async fn run(context: &RepoContext, input: GetInput) -> anyhow::Result<()> {
    anyhow::ensure!(
        input.max_depth.is_none() && input.returned_kinds.is_empty(),
        "review follow does not accept --depth or --kind"
    );
    let limit = input.limit.unwrap_or(50);
    anyhow::ensure!(limit > 0, "--limit must be positive");
    let host =
        provenance_transport::local_host::discover(context.repo.as_std_path(), &context.scope)?
            .context("no review host is running for this repository and scope")?;
    let key = super::launch::LaunchKey::read(host.instance_nonce())?;
    let mut endpoint = host.endpoint().clone();
    if endpoint.set_scheme("ws").is_err() {
        anyhow::bail!("invalid host endpoint");
    }
    endpoint.set_path("/review-events");
    let mut request = endpoint.as_str().into_client_request()?;
    request
        .headers_mut()
        .insert("x-provenance-launch-key", key.parse()?);
    let (mut socket, _) = tokio_tungstenite::connect_async(request)
        .await
        .context("cannot subscribe to the review host")?;
    let mut baseline = Baseline::default();
    let mut epoch = host.instance_nonce().to_owned();
    let mut sequence = 0;
    let mut initialized = false;
    let result: anyhow::Result<()> = async {
        loop {
            let incoming = tokio::select! {
                _ = tokio::signal::ctrl_c() => return Ok(()),
                frame = socket.next() => frame,
            };
            let Some(incoming) = incoming else { anyhow::bail!("review host disconnected"); };
            let frame = match incoming? {
                Message::Text(text) => serde_json::from_str::<Value>(&text)?,
                Message::Close(_) => anyhow::bail!("review host disconnected"),
                _ => continue,
            };
            let next_epoch = frame["epoch"].as_str().context("feed frame has no epoch")?;
            let next_sequence = frame["sequence"].as_u64().context("feed frame has no sequence")?;
            let reset = initialized && (frame["type"] == "reset" || next_epoch != epoch
                || next_sequence != sequence + 1);
            next_epoch.clone_into(&mut epoch);
            sequence = next_sequence;
            tokio::select! {
                _ = tokio::signal::ctrl_c() => return Ok(()),
                result = baseline.reconcile(context, &input.target, limit, &epoch, sequence, reset) => result?,
            }
            initialized = true;
        }
    }.await;
    if result.is_err() {
        emit(
            &json!({"type": "disconnected", "epoch": epoch, "sequence": sequence,
            "continuity": "unknown", "records": [], "removedIds": [], "continuations": []}),
        )?;
    }
    let _ = socket.close(None).await;
    result
}
