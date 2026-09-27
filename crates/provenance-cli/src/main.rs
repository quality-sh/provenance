mod atomic_file;
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
mod review;
mod safe_fs;
mod skills;
mod ste_onboarding;
mod store;
mod wiki;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let arguments = std::env::args().collect::<Vec<_>>();
    invocation::Invocation::parse(arguments)?.dispatch().await
}
