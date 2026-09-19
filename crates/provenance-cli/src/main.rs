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

use clap::Parser;
use cli::Cli;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let arguments = std::env::args().collect::<Vec<_>>();
    if catalog_cli::try_dispatch(&arguments).await? {
        return Ok(());
    }
    let cli = Cli::parse();
    let quiet = cli.quiet;
    handlers::dispatch(cli.command, quiet).await
}
