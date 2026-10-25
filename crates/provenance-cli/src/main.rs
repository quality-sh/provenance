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
    let invocation = invocation::Invocation::parse(arguments)?;
    let json = invocation.json_output();
    match invocation.dispatch().await {
        Ok(()) => Ok(()),
        Err(error) => match json
            .then(|| catalog_cli::refusal::json_refusal(&error))
            .flatten()
        {
            Some(refusal) => catalog_cli::refusal::exit_with_json(&refusal),
            None => Err(error),
        },
    }
}
