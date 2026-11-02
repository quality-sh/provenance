//! Review links from the library alone: no CLI process and no browser.
//!
//! These tests have no Rule binding. They are implementation aids that keep the library usable
//! without the CLI; the CLI tests bind the review link Rules.

use provenance_core::NodeType;
use provenance_review::{
    issue_launch_code, launch_key, owner_file, review_location, AffectedReviewRecord, PageResult,
    RepositoryScope, LAUNCH_CODE_ROUTE,
};
use provenance_transport::{
    fixture::{local_host::LocalHostFixture, records::Repository},
    local_host::{LocalHostRegistration, IDENTITY_ROUTE},
};
use std::{
    io::{Read as _, Write as _},
    net::{TcpListener, TcpStream},
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
    time::Duration,
};

fn place(repository: &Repository) -> RepositoryScope {
    RepositoryScope::new(repository.dir.path().to_str().unwrap(), "default")
}

fn record(kind: NodeType, id: &str) -> AffectedReviewRecord {
    AffectedReviewRecord {
        kind,
        id: id.to_owned(),
    }
}

#[tokio::test]
async fn a_record_in_a_review_document_links_to_its_root_with_the_record_in_focus() {
    let repository = Repository::new("The review output includes a link.");
    let host = LocalHostFixture::start(repository.dir.path(), "default", "local");

    let location = review_location(&place(&repository), &record(NodeType::Rule, "rule_shared"))
        .await
        .unwrap()
        .unwrap();

    assert_eq!(location.root.as_str(), "req_shared");
    assert_eq!(location.focus.as_deref(), Some("rule_shared"));
    assert_eq!(
        location.plain_link().unwrap().as_str(),
        format!("{}/?root=req_shared&focus=rule_shared", host.endpoint)
    );
}

#[tokio::test]
async fn the_document_root_links_without_a_focus() {
    let repository = Repository::new("The review output includes a link.");
    let host = LocalHostFixture::start(repository.dir.path(), "default", "local");

    let location = review_location(
        &place(&repository),
        &record(NodeType::Requirement, "req_shared"),
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(location.focus, None);
    assert_eq!(
        location.plain_link().unwrap().as_str(),
        format!("{}/?root=req_shared", host.endpoint)
    );
}

#[tokio::test]
async fn a_record_outside_every_review_document_has_no_location() {
    let repository = Repository::new("The review output includes a link.");

    let location = review_location(&place(&repository), &record(NodeType::Rule, "rule_absent"))
        .await
        .unwrap();

    assert!(location.is_none());
}

#[tokio::test]
async fn without_a_host_the_location_has_no_link() {
    let repository = Repository::new("The review output includes a link.");

    let location = review_location(&place(&repository), &record(NodeType::Rule, "rule_shared"))
        .await
        .unwrap()
        .unwrap();

    assert!(location.host.is_none());
    assert_eq!(location.plain_link(), None);
    assert_eq!(location.launch_link().unwrap(), None);
    assert_eq!(location.show_in_connected_page().await, PageResult::NoPage);
}

#[tokio::test]
async fn a_launch_link_carries_the_code_the_host_issued_to_the_key_holder() {
    let cache = tempfile::tempdir().unwrap();
    use_user_cache(cache.path());
    let repository = Repository::new("The review output includes a link.");
    let host = CodeHost::start(repository.dir.path());

    let location = review_location(&place(&repository), &record(NodeType::Rule, "rule_shared"))
        .await
        .unwrap()
        .unwrap();
    let launch = location.launch_link().unwrap().unwrap();

    assert_eq!(launch.fragment(), Some(format!("launch={CODE}").as_str()));
    let mut plain = launch.clone();
    plain.set_fragment(None);
    assert_eq!(plain, location.plain_link().unwrap());
    assert_eq!(
        plain.as_str(),
        format!("{}/?root=req_shared&focus=rule_shared", host.endpoint)
    );
    assert_eq!(
        issue_launch_code(location.host.as_ref().unwrap()).unwrap(),
        CODE
    );
    assert_eq!(location.show_in_connected_page().await, PageResult::Shown);
}

/// Points every platform's per-user cache at one isolated directory.
fn use_user_cache(root: &Path) {
    let cache = root.join("cache");
    std::env::set_var("HOME", root);
    std::env::set_var("XDG_CACHE_HOME", &cache);
    std::env::set_var("LOCALAPPDATA", &cache);
}

const KEY: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const CODE: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

/// A published host stand-in that answers the identity route, issues one fixed launch code to
/// the key holder and reports that a connected page showed each link.
struct CodeHost {
    endpoint: String,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    _registration: LocalHostRegistration,
}

impl CodeHost {
    fn start(root: &Path) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let registration =
            LocalHostRegistration::publish(root, "default", &endpoint, "local").unwrap();
        let identity = serde_json::to_string(&registration.identity()).unwrap();
        write_launch_key(&registration.identity().instance_nonce);
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = stop.clone();
        let thread = std::thread::spawn(move || {
            while !thread_stop.load(Ordering::Relaxed) {
                let Ok((mut stream, _)) = listener.accept() else {
                    std::thread::sleep(Duration::from_millis(5));
                    continue;
                };
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let request = read_request(&mut stream);
                if request.is_empty() {
                    continue;
                }
                let (status, body) = answer(&request, &identity);
                write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
            }
        });
        Self {
            endpoint,
            stop,
            thread: Some(thread),
            _registration: registration,
        }
    }
}

impl Drop for CodeHost {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect(self.endpoint.trim_start_matches("http://"));
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}

fn write_launch_key(nonce: &str) {
    let path = launch_key::path(nonce).unwrap();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(
            path.parent().unwrap(),
            std::fs::Permissions::from_mode(0o700),
        )
        .unwrap();
    }
    owner_file::create(&path)
        .unwrap()
        .write_all(KEY.as_bytes())
        .unwrap();
}

/// Reads one request with its full body, as the content length names it.
fn read_request(stream: &mut TcpStream) -> String {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 4096];
    loop {
        let count = stream.read(&mut chunk).unwrap_or(0);
        if count == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..count]);
        let text = String::from_utf8_lossy(&buffer);
        let Some(split) = text.find("\r\n\r\n") else {
            continue;
        };
        let length = text[..split]
            .lines()
            .find_map(|line| {
                line.to_ascii_lowercase()
                    .strip_prefix("content-length:")
                    .and_then(|value| value.trim().parse::<usize>().ok())
            })
            .unwrap_or(0);
        if text.len() - split - 4 >= length {
            break;
        }
    }
    String::from_utf8_lossy(&buffer).into_owned()
}

fn answer(request: &str, identity: &str) -> (&'static str, String) {
    let body = request.split_once("\r\n\r\n").map_or("", |(_, body)| body);
    let holds_key = serde_json::from_str::<serde_json::Value>(body)
        .is_ok_and(|value| value["launchKey"] == KEY);
    if request.starts_with(&format!("GET {IDENTITY_ROUTE} HTTP/1.1")) {
        ("200 OK", identity.to_owned())
    } else if request.starts_with(&format!("POST {LAUNCH_CODE_ROUTE} HTTP/1.1")) && holds_key {
        ("200 OK", format!("{{\"code\":\"{CODE}\"}}"))
    } else if request.starts_with("POST /review-navigation HTTP/1.1") && holds_key {
        ("200 OK", "{\"result\":\"shown\"}".to_owned())
    } else {
        ("404 Not Found", "{}".to_owned())
    }
}
