use crate::cli::workspace::WikiCommand;

pub(super) async fn handle(command: WikiCommand) -> anyhow::Result<()> {
    match command {
        WikiCommand::Build {
            context,
            out,
            coverage,
            format,
        } => crate::wiki::site::build(
            context.repo,
            context.scope,
            out,
            coverage.as_deref(),
            format,
        )?,
        WikiCommand::Serve {
            context,
            coverage,
            host,
            port,
        } => {
            crate::wiki::site::serve(context.repo, context.scope, coverage.as_deref(), host, port)
                .await?;
        }
    }
    Ok(())
}
