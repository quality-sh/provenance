#[path = "review_host_support/mod.rs"]
mod review_host_support;
use review_host_support::{
    follow::{create, Follow},
    repository, start,
};

#[test]
// Implementation aid: host loss must be explicit instead of a silent successful EOF.
fn host_loss_reports_unknown_continuity_and_fails() {
    let repo = repository();
    create(repo.path(), "req_follow", "requirement", &[]);
    let host = start(repo.path());
    let mut follow = Follow::start(&host, repo.path(), "req_follow");
    assert_eq!(follow.frame()["type"], "state");
    drop(host);
    let loss = follow.frame();
    assert_eq!(loss["type"], "disconnected");
    assert_eq!(loss["continuity"], "unknown");
    let end = std::time::Instant::now() + std::time::Duration::from_secs(15);
    loop {
        if let Some(status) = follow.child.try_wait().unwrap() {
            assert!(!status.success());
            break;
        }
        assert!(std::time::Instant::now() < end);
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
}
