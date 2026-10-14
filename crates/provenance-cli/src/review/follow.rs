//! Native review subscription. Reads run in order while the socket retains invalidations.
use super::events::{
    frames::Frame,
    output::{emit, OutputFrame},
    stream::Baseline,
};
use anyhow::Context as _;
use futures_util::{SinkExt, StreamExt};
use provenance_cli::repo_context::RepoContext;
use provenance_macros::rule;
use provenance_porcelain::get::GetInput;
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
    let mut read_failure = None;
    let result: anyhow::Result<()> = async {
        loop {
            let incoming = tokio::select! {
                signal = tokio::signal::ctrl_c() => {
                    signal.context("cannot install the Ctrl-C handler")?;
                    return Ok(());
                },
                frame = socket.next() => frame,
            };
            let Some(incoming) = incoming else { anyhow::bail!("review host disconnected"); };
            let frame = match incoming? {
                Message::Text(text) => serde_json::from_str::<Frame>(&text)?,
                Message::Close(_) => anyhow::bail!("review host disconnected"),
                _ => continue,
            };
            let (next_epoch, next_sequence) = frame.position();
            let reset = initialized && (matches!(frame, Frame::Reset { .. }) || next_epoch != epoch
                || next_sequence != sequence + 1);
            next_epoch.clone_into(&mut epoch);
            sequence = next_sequence;
            let frames = tokio::select! {
                signal = tokio::signal::ctrl_c() => {
                    signal.context("cannot install the Ctrl-C handler")?;
                    return Ok(());
                },
                result = baseline.reconcile(context, &input.target, limit, &epoch, sequence, reset) => match result {
                    Ok(frames) => frames,
                    Err(error) => {
                        read_failure = Some(OutputFrame::read_error(epoch.clone(), sequence, error.to_string()));
                        return Err(error);
                    }
                },
            };
            for frame in frames { emit(&frame).await?; }
            initialized = true;
        }
    }.await;
    let emitted = if result.is_err() {
        emit(&read_failure.unwrap_or_else(|| OutputFrame::disconnected(epoch, sequence))).await
    } else {
        Ok(())
    };
    let _ = socket.close(None).await;
    emitted?;
    result
}
