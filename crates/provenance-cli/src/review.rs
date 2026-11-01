mod detach;

use anyhow::Context as _;
use provenance_review::{HostOptions, RepositoryScope, StartedHost};
use serde_json::json;
use std::{io::Write, path::PathBuf, sync::Arc};

#[derive(clap::Args)]
pub struct Options {
    /// Repository with an initialized Provenance manifest.
    #[arg(long)]
    repo: PathBuf,
    /// Opaque repository target exposed to the browser.
    #[arg(long)]
    repository_id: String,
    #[arg(long)]
    scope: String,
    /// Loopback port. Zero selects an available port.
    #[arg(long, default_value_t = 0)]
    port: u16,
    /// Do not open the review page in a browser.
    #[arg(long)]
    no_open: bool,
    /// Reuse the running host for this repository and scope, or start one in the background
    /// that keeps running after this command returns. Prints where the review page is served.
    #[arg(long)]
    detach: bool,
}

pub async fn run(options: Options) -> anyhow::Result<()> {
    if options.detach {
        return detach::run(&options);
    }
    let root = match camino::Utf8PathBuf::from_path_buf(options.repo.clone()) {
        Ok(root) => root,
        // The repository refuses a root that is not UTF-8; its error is the one to report.
        Err(path) => {
            return Err(
                match provenance_store::repository::Repository::open(&path, &options.scope) {
                    Err(error) => anyhow::Error::new(error)
                        .context("cannot configure review repository access"),
                    Ok(_) => anyhow::anyhow!("repository path {} is not UTF-8", path.display()),
                },
            )
        }
    };
    // The check port reads the canonical working tree, as the opened repository does.
    let working_tree = dunce::canonicalize(&root)
        .ok()
        .and_then(|path| camino::Utf8PathBuf::from_path_buf(path).ok())
        .unwrap_or_else(|| root.clone());
    let signals = ShutdownSignals::install()?;
    let host = provenance_review::start(HostOptions {
        place: RepositoryScope::new(root, options.scope),
        repository_id: options.repository_id,
        port: options.port,
        check_port: Arc::new(crate::handlers::check::RepositoryCheckPort::new(
            working_tree,
            false,
            None,
        )),
        on_read_only: {
            let warning = crate::reviewer::review_page_warning(&options.repo);
            Box::new(move || eprintln!("{warning}"))
        },
    })
    .await?;
    let startup = host.startup();
    println!(
        "{}",
        json!({
            "endpoint": startup.endpoint(), "bearer": startup.bearer(),
            "repositoryId": startup.repository_id(), "scope": startup.scope(),
            "instanceNonce": startup.instance_nonce(), "url": startup.url(),
        })
    );
    std::io::stdout().flush()?;
    if !options.no_open {
        open_review_page(host.launch_link());
    }
    serve(host, &signals).await
}

/// Opens the review page signed in, or tells the person which link to open.
fn open_review_page(link: url::Url) {
    // A browser program can stay open, so the host does not wait for it before it serves.
    tokio::task::spawn_blocking(move || {
        if let crate::browser::Opening::Printed(reason) =
            crate::browser::open_or_print(&link, false)
        {
            eprintln!(
                "The review page did not open because {}. Open this link in your browser: {link}",
                reason.reason()
            );
        }
    });
}

/// Serves until the first signal, then drains. A signal during the drain forces the exit.
async fn serve(host: StartedHost, signals: &ShutdownSignals) -> anyhow::Result<()> {
    let (armed, forced) = tokio::sync::oneshot::channel();
    let stop = {
        let signals = signals.clone();
        async move {
            signals.received(1).await;
        }
    };
    let stopping = {
        let signals = signals.clone();
        move || {
            eprintln!("Review host is stopping. Wait for active operations, or send a second signal to force exit; writes may be incomplete.");
            // The signal that stopped the host does not force the exit; the next one does.
            let _ = armed.send(if signals.count() >= 1 { 2 } else { 1 });
        }
    };
    let serve = host.serve(stop, stopping);
    tokio::pin!(serve);
    tokio::select! {
        result = &mut serve => result,
        () = async {
            // A host that ends without the drain hook has no forced exit.
            let threshold = forced.await.unwrap_or(u32::MAX);
            signals.received(threshold).await;
        } => {
            eprintln!("Forced review host exit: writes may be incomplete. Check repository state before retrying a write.");
            // Returning would still wait for blocked tasks when Tokio drops its runtime.
            std::process::exit(1);
        }
    }
}

/// Counts the termination signals of the process, so that the first one stops the host and
/// the next one forces the exit.
#[derive(Clone)]
struct ShutdownSignals {
    count: tokio::sync::watch::Receiver<u32>,
}

impl ShutdownSignals {
    fn install() -> anyhow::Result<Self> {
        let mut signals = Signals::install().context("cannot install the signal handlers")?;
        let (sender, count) = tokio::sync::watch::channel(0);
        tokio::spawn(async move {
            while signals.next().await.is_some() {
                sender.send_modify(|count| *count += 1);
                if sender.is_closed() {
                    break;
                }
            }
        });
        Ok(Self { count })
    }

    fn count(&self) -> u32 {
        *self.count.borrow()
    }

    /// Completes when the process has received at least `count` signals.
    async fn received(&self, count: u32) {
        let mut receiver = self.count.clone();
        if receiver
            .wait_for(|received| *received >= count)
            .await
            .is_err()
        {
            std::future::pending::<()>().await;
        }
    }
}

/// The termination signals of the platform.
struct Signals {
    #[cfg(unix)]
    terminate: tokio::signal::unix::Signal,
    #[cfg(unix)]
    interrupt: tokio::signal::unix::Signal,
    #[cfg(windows)]
    interrupt: tokio::signal::windows::CtrlC,
}

impl Signals {
    fn install() -> std::io::Result<Self> {
        Ok(Self {
            #[cfg(unix)]
            terminate: tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?,
            #[cfg(unix)]
            interrupt: tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?,
            #[cfg(windows)]
            interrupt: tokio::signal::windows::ctrl_c()?,
        })
    }

    #[cfg(unix)]
    async fn next(&mut self) -> Option<()> {
        tokio::select! {
            signal = self.terminate.recv() => signal,
            signal = self.interrupt.recv() => signal,
        }
    }

    #[cfg(windows)]
    async fn next(&mut self) -> Option<()> {
        self.interrupt.recv().await
    }
}
