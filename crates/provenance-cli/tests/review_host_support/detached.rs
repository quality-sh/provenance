//! Starts detached review hosts with an isolated per-user cache, and stops them at the end.
use super::waits;
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

/// Stops the background host when the test ends, whatever the outcome.
pub struct Detached(pub u32);

impl Drop for Detached {
    fn drop(&mut self) {
        let _ = Command::new("kill")
            .args(["-TERM", &self.0.to_string()])
            .status();
    }
}

/// The per-user cache that the host and the CLI share, as one isolated directory.
pub struct UserCache(tempfile::TempDir);

impl UserCache {
    pub fn isolated() -> Self {
        Self(tempfile::tempdir().unwrap())
    }

    pub fn apply(&self, command: &mut Command) {
        let caches = self.0.path().join("Library").join("Caches");
        command
            .env("HOME", self.0.path())
            .env("XDG_CACHE_HOME", &caches)
            .env("LOCALAPPDATA", &caches)
            .env(
                "PROVENANCE_TEST_STE100_ASSET_URL",
                "http://127.0.0.1:9/unavailable",
            );
    }
}

pub fn detach_command(root: &Path, cache: &UserCache) -> Command {
    let mut command = Command::new(assert_cmd::cargo::cargo_bin("provenance"));
    command.args([
        "review",
        "--repo",
        root.to_str().unwrap(),
        "--repository-id",
        "A",
        "--scope",
        "default",
        "--detach",
    ]);
    cache.apply(&mut command);
    command
}

pub fn detach(root: &Path, cache: &UserCache, configure: impl FnOnce(&mut Command)) -> Value {
    let mut command = detach_command(root, cache);
    configure(&mut command);
    let output = waits::output(&mut command);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

/// The pid of a host that the command started.
pub fn started_pid(output: &Value) -> u32 {
    u32::try_from(output["pid"].as_u64().expect("pid of the started host")).unwrap()
}

/// The host's public identity, or `None` when nothing answers at the endpoint.
pub fn identity(endpoint: &str) -> Option<Value> {
    let response = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(2))
        .build()
        .get(&format!("{endpoint}/local-host-identity"))
        .call()
        .ok()?;
    serde_json::from_str(&response.into_string().ok()?).ok()
}

pub fn wait_until(condition: impl Fn() -> bool, what: &str) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while !condition() {
        assert!(Instant::now() < deadline, "{what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// The discovery record of the repository's default scope.
pub fn registry(root: &Path) -> PathBuf {
    root.join(".provenance/cache/local-hosts/default.json")
}
