use crate::cli::workspace::DocsCommand;

pub(super) async fn handle(command: DocsCommand) -> anyhow::Result<()> {
    match command {
        DocsCommand::Check { repo, .. } => crate::docs::check(&repo)?,
        DocsCommand::Serve { repo, host, port } => crate::docs::serve(repo, host, port).await?,
    }
    Ok(())
}
