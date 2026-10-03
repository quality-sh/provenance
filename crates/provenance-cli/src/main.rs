mod atomic_file;
mod browser_open;
mod catalog_cli;
mod cli;
mod docs;
mod gitignore;
mod handlers;
mod html;
mod init_summary;
mod invocation;
mod legacy_cleanup;
mod onboarding;
mod output;
use provenance_cli::{repo_context, store};
mod review;
mod review_launch;
mod review_link;
mod reviewer;
mod skills;
mod ste_onboarding;
mod wiki;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let arguments = std::env::args().collect::<Vec<_>>();
    invocation::Invocation::parse(arguments)?.dispatch().await
}
