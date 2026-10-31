//! Native review subscription. Reads run in order while the socket retains invalidations.
use super::events::{
    frames::Frame,
    output::{emit, OutputFrame},
    stream::Follower,
};
use anyhow::Context as _;
use futures_util::StreamExt;
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
            .context(provenance_porcelain::get::NO_REVIEW_HOST)?;
    let key = provenance_cli::notify::read_launch_key(host.instance_nonce())?;
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
    let mut follower = Follower::new(host.instance_nonce());
    let mut read_failure = None;
    let interrupt = tokio::signal::ctrl_c();
    tokio::pin!(interrupt);
    let result: anyhow::Result<()> = async {
        loop {
            let incoming = tokio::select! {
                signal = &mut interrupt => {
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
            tokio::select! {
                signal = &mut interrupt => {
                    signal.context("cannot install the Ctrl-C handler")?;
                    return Ok(());
                },
                result = async {
                    let frames = match follower.step(context, &input.target, limit, &frame).await {
                        Ok(frames) => frames,
                        Err(error) => {
                            let (epoch, sequence) = follower.position();
                            read_failure = Some(OutputFrame::read_error(epoch, sequence, error.to_string()));
                            return Err(error);
                        }
                    };
                    for frame in frames { emit(&frame).await?; }
                    Ok::<(), anyhow::Error>(())
                } => result?,
            }
        }
    }.await;
    let emitted = if result.is_err() {
        let (epoch, sequence) = follower.position();
        emit(&read_failure.unwrap_or_else(|| OutputFrame::disconnected(epoch, sequence))).await
    } else {
        Ok(())
    };
    let _ = socket.close(None).await;
    emitted?;
    result
}
