mod browser;
mod catalog_cli;
mod cli;
mod handlers;
mod invocation;
mod output;
mod read_policy;
mod registry_cli;
#[cfg(windows)]
use provenance_review::owner_file;
mod review;
mod review_link;
mod skills;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let arguments = std::env::args().collect::<Vec<_>>();
    let invocation = invocation::Invocation::parse(arguments)?;
    let json = invocation.json_output();
    match invocation.dispatch().await {
        Err(error) if json => {
            catalog_cli::refusal::exit_with_json(&catalog_cli::refusal::json_refusal(&error))
        }
        result => result,
    }
}
