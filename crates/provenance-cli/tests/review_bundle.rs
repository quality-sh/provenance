//! The pinned renderer bundle is served byte-for-byte by a standalone binary.
//!
//! Ignored by default: it fetches the pinned renderer archive and rebuilds the
//! CLI with it. Set `PROVENANCE_REVIEW_ARCHIVE` to a saved archive, or allow
//! authenticated `gh run download` access, then run with `--ignored`.

#[path = "review_bundle/archive.rs"]
mod archive;
#[path = "review_bundle/pin.rs"]
mod pin;

use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader, Read},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::Duration,
};

use provenance_core::{Manifest, RepoPathPrefix, ScopeId};
use provenance_store::layout::ProvenanceLayout;
use serde_json::{json, Value};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("CLI crate is nested under the workspace root")
        .to_path_buf()
}

/// Every file below the asset root, keyed by its served URL path.
fn inventory(root: &Path) -> std::io::Result<BTreeMap<String, Vec<u8>>> {
    let mut files = BTreeMap::new();
    for entry in std::fs::read_dir(root)? {
        let path = entry?.path();
        let name = format!(
            "/{}",
            path.strip_prefix(root)
                .expect("asset below the root")
                .to_string_lossy()
        );
        if path.is_dir() {
            files.extend(
                inventory(&path)?
                    .into_iter()
                    .map(|(child, bytes)| (format!("{name}{child}"), bytes)),
            );
        } else {
            files.insert(name, std::fs::read(path)?);
        }
    }
    Ok(files)
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

/// Copy verified assets to a new directory for reuse in later builds.
fn retain_directory(source: &Path, destination: &Path) -> std::io::Result<()> {
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::create_dir(destination)?;
    for entry in std::fs::read_dir(source)? {
        let path = entry?.path();
        let name = path
            .strip_prefix(source)
            .expect("asset below the root")
            .to_owned();
        if path.is_dir() {
            retain_directory(&path, &destination.join(name))?;
        } else {
            std::fs::copy(&path, destination.join(name))?;
        }
    }
    Ok(())
}

fn request(host: &Value, method: &str, path: &str, auth: bool) -> ureq::Request {
    let request = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(5))
        .build()
        .request(
            method,
            &format!("{}{path}", host["endpoint"].as_str().unwrap()),
        );
    if auth {
        request.set(
            "Authorization",
            &format!("Bearer {}", host["bearer"].as_str().unwrap()),
        )
    } else {
        request
    }
}

fn response(result: Result<ureq::Response, ureq::Error>) -> ureq::Response {
    match result {
        Ok(response) | Err(ureq::Error::Status(_, response)) => response,
        Err(error) => panic!("{error}"),
    }
}

fn body(response: ureq::Response) -> Vec<u8> {
    let mut bytes = Vec::new();
    response
        .into_reader()
        .read_to_end(&mut bytes)
        .expect("read the response body");
    bytes
}

fn call_operation(host: &Value, target: &str) -> ureq::Response {
    response(
        request(host, "POST", "/v7/operations/list-threads", true)
            .set("Content-Type", "application/json")
            .send_string(
                &json!({"context":{"repository":target,"scope":"default"},"request":null})
                    .to_string(),
            ),
    )
}

fn start_host(work: &Path, repo: &Path) -> Child {
    Command::new(work.join("provenance"))
        .args([
            "review",
            "--repo",
            repo.to_str().expect("repository path is UTF-8"),
            "--repository-id",
            "A",
            "--scope",
            "default",
        ])
        .current_dir(work)
        // The copied executable has no Node executable or asset directory available.
        .env("PATH", work)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("start the copied review host")
}

fn read_config(child: &mut Child) -> Value {
    let stdout = child.stdout.take().expect("piped host stdout");
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        BufReader::new(stdout)
            .read_line(&mut line)
            .expect("read host line");
        let _ = send.send(line);
    });
    let line = receive
        .recv_timeout(Duration::from_secs(15))
        .expect("host startup timed out");
    serde_json::from_str(&line).expect("host configuration line")
}

/// Stop like a local caller does, and require a clean exit.
fn stop_gracefully(child: &mut Child) {
    #[cfg(unix)]
    {
        assert!(Command::new("kill")
            .args(["-TERM", &child.id().to_string()])
            .status()
            .expect("send SIGTERM")
            .success());
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(status) = child.try_wait().expect("wait for the review host") {
                assert!(status.success(), "review host did not exit cleanly");
                return;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "host shutdown timed out"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    #[cfg(not(unix))]
    {
        child.kill().expect("terminate the review host");
        child.wait().expect("wait for the review host");
        // Windows termination cannot express a clean exit code.
    }
}

#[test]
#[ignore = "builds with the pinned renderer archive; set PROVENANCE_REVIEW_ARCHIVE or allow gh run download"]
fn pinned_archive_is_served_by_a_standalone_binary() {
    let pin = pin::pin();
    let work = tempfile::tempdir().expect("bundle workspace");
    let assets = work.path().join("assets");
    let saved = std::env::var_os("PROVENANCE_REVIEW_ARCHIVE").map(PathBuf::from);
    pin::prepare(&assets, saved.as_deref(), &pin).expect("prepare the pinned review assets");
    let files = verified_inventory(&assets);

    let binary = build_standalone_binary(work.path(), &assets);
    std::fs::remove_dir_all(&assets).expect("remove the build input");

    let repo = repository(work.path());
    let mut host = start_host(work.path(), &repo);
    let config = read_config(&mut host);
    assert_served_bundle(&config, &files);

    stop_gracefully(&mut host);
    let address = config["endpoint"]
        .as_str()
        .unwrap()
        .strip_prefix("http://")
        .unwrap();
    assert!(TcpStream::connect(address).is_err(), "listener still open");
    assert!(TcpListener::bind(address).is_ok(), "port was not released");

    if let Some(destination) = std::env::var_os("PROVENANCE_REVIEW_BINARY_OUTPUT") {
        std::fs::copy(&binary, Path::new(&destination)).expect("retain the validated binary");
    }
    println!(
        "Served {} verified assets and authorized reads without Node or asset files.",
        files.len()
    );
    println!("Source {}; SHA-256 {}.", pin.commit, pin.sha256);
    println!(
        "The generic HTML is an empty shell. Document bootstrap and the adapter remain in .3."
    );
}

/// Verify the renderer shape, keep optional outputs, and inventory the assets.
fn verified_inventory(assets: &Path) -> BTreeMap<String, Vec<u8>> {
    let files = inventory(assets).expect("asset inventory");
    for required in [
        "/index.html",
        "/review.js",
        "/review.css",
        "/build-info.json",
    ] {
        assert!(
            files.contains_key(required),
            "pinned archive omits {required}"
        );
    }
    assert!(
        contains(&files["/review.js"], b"mountReview"),
        "the renderer export is missing"
    );
    assert!(
        !contains(&files["/index.html"], b"<script"),
        "index.html must not carry scripts"
    );
    if let Some(destination) = std::env::var_os("PROVENANCE_REVIEW_ASSETS_OUTPUT") {
        retain_directory(assets, Path::new(&destination)).expect("retain verified assets");
    }
    files
}

/// Build the CLI with the real renderer embedded and copy it out of the tree.
fn build_standalone_binary(work: &Path, assets: &Path) -> PathBuf {
    let status = Command::new(env!("CARGO"))
        .args([
            "build",
            "--locked",
            "-p",
            "provenance-cli",
            "--bin",
            "provenance",
        ])
        .env("PROVENANCE_REVIEW_ASSETS_DIR", assets)
        .current_dir(workspace_root())
        .status()
        .expect("run cargo build");
    assert!(
        status.success(),
        "cargo build with the pinned assets failed"
    );
    let metadata = Command::new(env!("CARGO"))
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(workspace_root())
        .output()
        .expect("run cargo metadata");
    let metadata: Value = serde_json::from_slice(&metadata.stdout).expect("parse cargo metadata");
    let built = Path::new(metadata["target_directory"].as_str().unwrap())
        .join("debug")
        .join("provenance");
    let binary = work.join("provenance");
    std::fs::copy(&built, &binary).expect("copy the standalone binary");
    binary
}

/// An initialized repository with the selected default scope.
fn repository(work: &Path) -> PathBuf {
    let repo = work.join("repository");
    let layout = ProvenanceLayout::new(repo.to_str().expect("repository path is UTF-8"));
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_vec(&Manifest::default_with_scope(
            ScopeId::new("default").unwrap(),
            RepoPathPrefix::new("."),
        ))
        .unwrap(),
    )
    .unwrap();
    repo
}

fn assert_served_bundle(config: &Value, files: &BTreeMap<String, Vec<u8>>) {
    let index = body(response(request(config, "GET", "/", false).call()));
    assert_eq!(index, files["/index.html"]);
    for (path, expected) in files {
        let reply = response(request(config, "GET", path, false).call());
        assert_eq!(reply.header("Cache-Control"), Some("no-store"), "{path}");
        if Path::new(path)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("js"))
        {
            let kind = reply.header("Content-Type").unwrap_or_default();
            assert!(kind.starts_with("text/javascript"), "{path}: {kind}");
        }
        assert_eq!(&body(reply), expected, "{path}");
    }
    assert_eq!(
        response(request(config, "GET", "/metadata", false).call()).status(),
        200
    );
    assert_eq!(call_operation(config, "A").status(), 200);
    assert_eq!(call_operation(config, "B").status(), 404);
    assert_eq!(
        response(request(config, "GET", "/assets/missing.js", false).call()).status(),
        404
    );
    let runtime: Value = serde_json::from_str(
        &response(request(config, "GET", "/review-config", true).call())
            .into_string()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(runtime["repositoryId"], "A");
    assert_eq!(runtime["scope"], "default");
    assert!(runtime.get("bearer").is_none());
    assert_eq!(
        config["url"],
        format!("{}/", config["endpoint"].as_str().unwrap())
    );
}
