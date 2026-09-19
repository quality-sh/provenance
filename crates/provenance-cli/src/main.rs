mod atomic_file;
mod catalog_cli;
mod cli;
mod docs;
mod gitignore;
mod handlers;
mod legacy_cleanup;
mod onboarding;
mod output;
mod review;
mod skills;
mod ste_onboarding;
mod store;
mod wiki;

use clap::{CommandFactory, Parser};
use cli::Cli;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let arguments = std::env::args().collect::<Vec<_>>();
    if provenance_cli::porcelain::try_dispatch_bare(&arguments).await? {
        return Ok(());
    }
    if provenance_cli::porcelain::explicitly_selects_get(&arguments)?
        && provenance_cli::porcelain::try_dispatch(&arguments).await?
    {
        return Ok(());
    }
    if catalog_cli::try_dispatch(&arguments).await? {
        return Ok(());
    }
    let mut index = 1;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--quiet" => index += 1,
            "--repo" | "--scope" | "--format" => index += 2,
            _ => break,
        }
    }
    let command = arguments.get(index);
    let is_builtin = command.is_none_or(|command| {
        command.starts_with('-')
            || Cli::command()
                .get_subcommands()
                .any(|candidate| candidate.get_name() == command)
    });
    if !is_builtin && provenance_cli::porcelain::try_dispatch(&arguments).await? {
        return Ok(());
    }
    let cli = Cli::parse();
    let quiet = cli.quiet;
    handlers::dispatch(cli.command, quiet).await
}
