use super::*;

fn codes(directory: &tempfile::TempDir) -> LaunchCodes {
    let key = LaunchKey {
        path: directory.path().join("host.key"),
        value: secret(),
    };
    LaunchCodes::new(&secret(), key)
}

#[test]
/// Implementation aid: security hardening keeps a launch code valid for 120 seconds only.
fn an_expired_code_is_refused() {
    let directory = tempfile::tempdir().unwrap();
    let codes = codes(&directory);
    let issued_at = Instant::now();
    let inside = codes.issue(issued_at);
    let expired = codes.issue(issued_at);

    assert!(codes
        .redeem(&inside, issued_at + CODE_LIFETIME - Duration::from_secs(1))
        .is_some());
    assert!(codes.redeem(&expired, issued_at + CODE_LIFETIME).is_none());
}
