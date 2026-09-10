use super::{served, ReadContext, ReadPolicy};
use camino::Utf8PathBuf;
use provenance_core::protocol::{ReadDocumentQuery, ReadDocumentResult, Stamped};
use provenance_core::{Question, Requirement, Resolution, Rule, ScopeId, Source, StableId, Topic};

pub async fn read_document(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    policy: ReadPolicy,
    request: ReadDocumentQuery,
) -> anyhow::Result<Stamped<ReadDocumentResult>> {
    served(repo, scope, policy, move |ctx| {
        Box::pin(async move { read(ctx, request).await })
    })
    .await
}

pub(super) async fn read(
    ctx: &ReadContext,
    request: ReadDocumentQuery,
) -> anyhow::Result<ReadDocumentResult> {
    request
        .validate()
        .map_err(provenance_core::protocol::QueryValidation::into_native)?;
    let snapshot = ctx.snapshot();
    Ok(ReadDocumentResult {
        root_id: StableId::new(request.id)?,
        requirements: snapshot.table::<Requirement>().all().await?,
        resolutions: snapshot.table::<Resolution>().all().await?,
        rules: snapshot.table::<Rule>().all().await?,
        sources: snapshot.table::<Source>().all().await?,
        topics: snapshot.table::<Topic>().all().await?,
        questions: snapshot.table::<Question>().all().await?,
        threads: snapshot.threads().await?,
        messages: snapshot.messages().await?,
    })
}
