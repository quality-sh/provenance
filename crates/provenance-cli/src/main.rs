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
    let started = std::time::Instant::now();
    let arguments = std::env::args().collect::<Vec<_>>();
    let invocation = invocation::Invocation::parse(arguments)?;
    let json = invocation.json_output();
    let dispatched = invocation.dispatch().await;
    if std::env::var_os("PROVENANCE_STALE_PROFILE").is_some() {
        eprintln!(
            "PROF CLI main (after process start, before exit): {:.2} ms",
            started.elapsed().as_secs_f64() * 1000.0
        );
    }
    match dispatched {
        Ok(()) => Ok(()),
        Err(error) => json
            .then(|| catalog_cli::refusal::json_refusal(&error))
            .flatten()
            .map_or(Err(error), |refusal| {
                catalog_cli::refusal::exit_with_json(&refusal)
            }),
    }
}
