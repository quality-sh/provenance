use super::grammar::{DiscussionsArgs, TargetArgs};
use provenance_cli::porcelain;
use provenance_core::{
    protocol::RecordResolution, threads::DiscussionStatusFilter, MessageRole, StableId,
    ThreadParent,
};
use provenance_porcelain::{
    discussion::{
        ConversationInput, DiscussionAction, DiscussionOutcome, ListInput, ReplyInput, StartInput,
    },
    get::GetPort as _,
};

pub async fn dispatch_root(args: DiscussionsArgs) -> anyhow::Result<()> {
    let host = porcelain::local_host(&args.common.repo, &args.common.scope)?;
    let service = provenance_porcelain::Porcelain::new(
        provenance_transport::porcelain::HostDiscussionPort::new(host),
    );
    let outcome = if let Some(id) = args.discussion_id {
        anyhow::ensure!(
            args.action.as_deref() == Some("get"),
            "an addressed Discussion requires get"
        );
        anyhow::ensure!(
            args.status.is_none(),
            "a conversation does not select list status"
        );
        service
            .conversation(ConversationInput {
                discussion_id: StableId::new(id)?,
                limit: args.limit,
                cursor: args.cursor,
            })
            .await?
    } else {
        anyhow::ensure!(args.action.is_none(), "get requires a Discussion ID");
        service
            .discussions(ListInput {
                parent: None,
        status: args.status.unwrap_or_default(),
                limit: args.limit,
                cursor: args.cursor,
            })
            .await?
    };
    print(&outcome, args.common.format())
}

pub async fn dispatch_target(
    args: TargetArgs,
    action: DiscussionAction,
    matches: &clap::ArgMatches,
) -> anyhow::Result<()> {
    let allowed = match action {
        DiscussionAction::Discussions => &["status", "limit", "cursor"][..],
        DiscussionAction::Discussion => &["limit", "cursor"],
        DiscussionAction::Discuss => &["body", "actor", "request_id", "role"],
        DiscussionAction::Reply => &["body", "actor", "request_id", "role", "expected_version"],
    };
    crate::catalog_cli::ensure_only_fields(matches, allowed);
    let target = StableId::new(args.target.clone())?;
    let host = porcelain::local_host(&args.common.repo, &args.common.scope)?;
    let service = provenance_porcelain::Porcelain::new(
        provenance_transport::porcelain::HostDiscussionPort::new(host.clone()),
    );
    let outcome = match action {
        DiscussionAction::Discussions => {
            let parent = resolve_parent(&host, target).await?;
            service
                .discussions(ListInput {
                    parent: Some(parent),
                    status: status(args.status.as_deref()),
                    limit: args.limit,
                    cursor: args.cursor,
                })
                .await?
        }
        DiscussionAction::Discussion => {
            service
                .conversation(ConversationInput {
                    discussion_id: target,
                    limit: args.limit,
                    cursor: args.cursor,
                })
                .await?
        }
        DiscussionAction::Discuss => {
            let parent = resolve_parent(&host, target).await?;
            service
                .discuss(StartInput {
                    parent,
                    request_id: request_id(args.request_id)?,
                    actor: args.actor.unwrap_or_else(|| "cli".into()),
                    declared_by: None,
                    role: role(args.role.as_deref())?,
                    body: args
                        .body
                        .ok_or_else(|| anyhow::anyhow!("--body is required"))?,
                })
                .await?
        }
        DiscussionAction::Reply => {
            service
                .reply(ReplyInput {
                    discussion_id: target,
                    request_id: request_id(args.request_id)?,
                    actor: args.actor.unwrap_or_else(|| "cli".into()),
                    declared_by: None,
                    expected_version: args
                        .expected_version
                        .ok_or_else(|| anyhow::anyhow!("--expected-version is required"))?
                        .parse()?,
                    role: role(args.role.as_deref())?,
                    body: args
                        .body
                        .ok_or_else(|| anyhow::anyhow!("--body is required"))?,
                })
                .await?
        }
    };
    print(&outcome, args.common.format())
}

fn status(status: Option<&str>) -> DiscussionStatusFilter {
    status.map_or(DiscussionStatusFilter::default(), |word| {
        DiscussionStatusFilter::parse(word).unwrap_or_else(|| {
            crate::catalog_cli::usage_error(format!("invalid Discussion status {word}"))
        })
    })
}

fn role(role: Option<&str>) -> anyhow::Result<MessageRole> {
    MessageRole::parse(role.unwrap_or("user"))
}

fn request_id(id: Option<String>) -> anyhow::Result<StableId> {
    StableId::new(id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()))
}

async fn resolve_parent(
    host: &provenance_transport::StatementHost,
    id: StableId,
) -> anyhow::Result<ThreadParent> {
    let port = provenance_transport::porcelain::HostGetPort::new(host.clone());
    let kind = match port.resolve(id.as_str()).await?.result {
        RecordResolution::Found(record) => record.node_type(),
        RecordResolution::Missing => anyhow::bail!("record does not exist"),
        RecordResolution::Ambiguous => anyhow::bail!("record ID is not unique"),
    };
    Ok(ThreadParent {
        node_type: kind,
        node_id: id,
    })
}

fn print(
    outcome: &DiscussionOutcome,
    format: Option<porcelain::OutputFormat>,
) -> anyhow::Result<()> {
    if format == Some(porcelain::OutputFormat::Json) {
        println!("{}", serde_json::to_string_pretty(outcome)?);
    } else {
        println!(
            "{}",
            provenance_porcelain::discussion::render_readable(outcome)
        );
    }
    Ok(())
}
