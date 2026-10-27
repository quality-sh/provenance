use super::*;
use provenance_macros::verifies;

fn codes(directory: &tempfile::TempDir) -> LaunchCodes {
    let key = LaunchKey {
        path: directory.path().join("host.key"),
        value: secret(),
    };
    LaunchCodes::new(&secret(), key)
}

#[test]
#[verifies("rule_review_launch_code_lifetime", examples)]
fn an_expired_code_is_refused() {
    let directory = tempfile::tempdir().unwrap();
    let codes = codes(&directory);
    let issued_at = Instant::now();
    let inside = codes.issue(issued_at);
    let expired = codes.issue(issued_at);

    assert!(codes
        .redeem(&inside, issued_at + Duration::from_secs(24 * 60 * 60 - 1))
        .is_some());
    assert!(codes
        .redeem(&expired, issued_at + Duration::from_hours(24))
        .is_none());
}

#[test]
#[verifies("rule_review_launch_code_lifetime", examples)]
fn a_full_store_replaces_the_oldest_live_code() {
    let directory = tempfile::tempdir().unwrap();
    let codes = codes(&directory);
    let issued_at = Instant::now();
    let oldest = codes.issue(issued_at);
    let retained: Vec<_> = (1..64).map(|_| codes.issue(issued_at)).collect();
    let newest = codes.issue(issued_at + Duration::from_secs(1));
    let redeemed_at = issued_at + Duration::from_secs(24 * 60 * 60 - 1);

    assert!(codes.redeem(&oldest, redeemed_at).is_none());
    assert!(codes.redeem(&newest, redeemed_at).is_some());
    for code in retained {
        assert!(codes.redeem(&code, redeemed_at).is_some());
    }
    assert!(codes.redeem(&newest, redeemed_at).is_none());
}
