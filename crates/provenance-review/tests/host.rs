//! The review host from the library alone: no CLI process, no signals and no browser.
//!
//! This test has no Rule binding. It is an implementation aid that keeps the host usable
//! without the CLI; the CLI tests bind the review host Rules.

use provenance_porcelain::check::{Category, CheckPort, PortFuture};
use provenance_review::{
    bundle, launch_key, HostOptions, RepositoryScope, LAUNCH_CODE_ROUTE, LAUNCH_SESSION_ROUTE,
};
use provenance_transport::fixture::records::Repository;
use serde_json::{json, Value};
use std::{
    path::Path,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

/// A check port for a host whose test never runs a check.
struct NoChecks;

impl CheckPort for NoChecks {
    fn run<'a>(&'a self, _: Category, _: Option<&'a str>) -> PortFuture<'a> {
        Box::pin(async { Err("this host runs no checks".to_owned()) })
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_started_host_signs_a_page_in_serves_assets_and_leaves_no_record_after_stop() {
    let cache = tempfile::tempdir().unwrap();
    use_user_cache(cache.path());
    let repository = Repository::new("The review host serves from the library.");
    let place = RepositoryScope::new(repository.dir.path().to_str().unwrap(), "default");

    let host = provenance_review::start(HostOptions {
        place: place.clone(),
        repository_id: "local".to_owned(),
        port: 0,
        check_port: Arc::new(NoChecks),
        on_read_only: Box::new(|| {}),
    })
    .await
    .unwrap();

    let startup = host.startup();
    assert_eq!(startup.repository_id(), "local");
    assert_eq!(startup.scope(), "default");
    assert_eq!(startup.url(), format!("{}/", startup.endpoint()));
    assert_eq!(
        host.launch_link().as_str().len(),
        startup.url().len() + "#launch=".len() + 64
    );
    // The fixture manifest names no reviewer, so the page is read-only.
    assert!(!host.has_reviewer());
    let discovered = place.host().unwrap().unwrap();
    assert_eq!(discovered.instance_nonce(), startup.instance_nonce());
    let key_path = launch_key::path(startup.instance_nonce()).unwrap();
    assert!(key_path.is_file());

    let endpoint = startup.endpoint().to_owned();
    let key = launch_key::read(startup.instance_nonce()).unwrap();
    let (code, session, config, page) = tokio::task::spawn_blocking(move || {
        let code = post(&endpoint, LAUNCH_CODE_ROUTE, &json!({ "launchKey": key }))["code"]
            .as_str()
            .unwrap()
            .to_owned();
        let session = post(&endpoint, LAUNCH_SESSION_ROUTE, &json!({ "code": code }))["session"]
            .as_str()
            .unwrap()
            .to_owned();
        let config: Value = serde_json::from_str(
            &ureq::get(&format!("{endpoint}/review-config"))
                .set("Authorization", &format!("Bearer {session}"))
                .call()
                .unwrap()
                .into_string()
                .unwrap(),
        )
        .unwrap();
        let page = ureq::get(&format!("{endpoint}/")).call().unwrap();
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut page.into_reader(), &mut bytes).unwrap();
        (code, session, config, bytes)
    })
    .await
    .unwrap();
    assert!(launch_key::is_secret(&code));
    assert!(session.starts_with(&format!("{}.", startup.instance_nonce())));
    assert_eq!(config["repositoryId"], "local");
    assert_eq!(config["scope"], "default");
    assert_eq!(config["dispositionActorIds"], json!([]));
    let index = bundle()
        .iter()
        .find(|(path, _)| *path == "/index.html")
        .map(|(_, bytes)| *bytes)
        .unwrap();
    assert_eq!(page, index);

    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let stopping = Arc::new(AtomicUsize::new(0));
    let counted = stopping.clone();
    let serving = host.serve(
        async move {
            let _ = stopped.await;
        },
        move || {
            counted.fetch_add(1, Ordering::SeqCst);
        },
    );
    tokio::pin!(serving);
    tokio::select! {
        result = &mut serving => panic!("the host stopped before the stop future: {result:?}"),
        () = tokio::time::sleep(std::time::Duration::from_millis(200)) => {}
    }
    assert_eq!(stopping.load(Ordering::SeqCst), 0);
    stop.send(()).unwrap();
    serving.await.unwrap();

    assert_eq!(stopping.load(Ordering::SeqCst), 1);
    assert!(place.host().unwrap().is_none());
    assert!(!key_path.exists());
}

fn post(endpoint: &str, route: &str, body: &Value) -> Value {
    let reply = ureq::post(&format!("{endpoint}{route}"))
        .set("Content-Type", "application/json")
        .send_string(&body.to_string())
        .unwrap()
        .into_string()
        .unwrap();
    serde_json::from_str(&reply).unwrap()
}

/// Points every platform's per-user cache at one isolated directory.
fn use_user_cache(root: &Path) {
    let cache = root.join("cache");
    std::env::set_var("HOME", root);
    std::env::set_var("XDG_CACHE_HOME", &cache);
    std::env::set_var("LOCALAPPDATA", &cache);
}
