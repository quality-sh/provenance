mod atomic_file;
mod cli;
mod docs;
mod gitignore;
mod handlers;
mod legacy_cleanup;
mod onboarding;
mod output;
mod skills;
mod ste_onboarding;
mod wiki;

use clap::Parser;
use cli::Cli;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let quiet = cli.quiet;
    handlers::dispatch(cli.command, quiet).await
}
