//! Native review subscription. Reads run in order while the socket retains invalidations.
use super::events::{
    frames::Frame,
    output::{emit, OutputFrame},
    stream::Follower,
};
use crate::RepositoryScope;
use anyhow::Context as _;
use futures_util::StreamExt;
use provenance_macros::rule;
use provenance_porcelain::get::GetInput;
use std::future::Future;
use tokio::io::AsyncWrite;
use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Message};

/// Streams saved document state to `output` when the repository host sends an invalidation.
///
/// `stop` is first polled after the connection opens. The stream ends with success when `stop`
/// completes with `Ok`, and with its error otherwise.
#[rule("rule_review_follow_push")]
pub async fn follow(
    place: &RepositoryScope,
    input: GetInput,
    mut output: impl AsyncWrite + Unpin + Send,
    stop: impl Future<Output = anyhow::Result<()>> + Send,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        input.max_depth.is_none() && input.returned_kinds.is_empty(),
        "review follow does not accept --depth or --kind"
    );
    let limit = input.limit.unwrap_or(50);
    anyhow::ensure!(limit > 0, "--limit must be positive");
    let host = place
        .host()?
        .context(provenance_porcelain::get::NO_REVIEW_HOST)?;
    let key = crate::launch_key::read(host.instance_nonce())?;
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
    tokio::pin!(stop);
    let result: anyhow::Result<()> = async {
        loop {
            let incoming = tokio::select! {
                signal = &mut stop => {
                    signal?;
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
                signal = &mut stop => {
                    signal?;
                    return Ok(());
                },
                result = async {
                    let frames = match follower.step(place, &input.target, limit, &frame).await {
                        Ok(frames) => frames,
                        Err(error) => {
                            let (epoch, sequence) = follower.position();
                            read_failure = Some(OutputFrame::read_error(epoch, sequence, error.to_string()));
                            return Err(error);
                        }
                    };
                    for frame in frames { emit(&mut output, &frame).await?; }
                    Ok::<(), anyhow::Error>(())
                } => result?,
            }
        }
    }.await;
    let emitted = if result.is_err() {
        let (epoch, sequence) = follower.position();
        emit(
            &mut output,
            &read_failure.unwrap_or_else(|| OutputFrame::disconnected(epoch, sequence)),
        )
        .await
    } else {
        Ok(())
    };
    let _ = socket.close(None).await;
    emitted?;
    result
}
