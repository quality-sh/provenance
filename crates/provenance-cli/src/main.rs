mod atomic_file;
mod browser;
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
#[cfg(windows)]
mod owner_file;
mod read_policy;
use provenance_cli::{repo_context, store};
mod review;
mod review_link;
mod reviewer;
mod skills;
mod ste_onboarding;
mod user_cache;
mod wiki;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let arguments = std::env::args().collect::<Vec<_>>();
    invocation::Invocation::parse(arguments)?.dispatch().await
}
