//! Implementation aid: establish that this runner can cause native queue loss.
#![cfg(target_os = "linux")]
use notify::{RecursiveMode, Watcher};
use std::{
    io::{BufRead, BufReader},
    process::{Command, Stdio},
    sync::mpsc,
    time::Duration,
};

#[test]
fn watcher_overflow_child() {
    let Some(path) = std::env::var_os("REVIEW_OVERFLOW_PROBE_PATH") else {
        return;
    };
    let (send, receive) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |event| {
        let _ = send.send(event);
    })
    .unwrap();
    watcher
        .watch(std::path::Path::new(&path), RecursiveMode::Recursive)
        .unwrap();
    println!("WATCHING");
    let deadline = std::time::Instant::now() + Duration::from_secs(45);
    loop {
        let event = receive
            .recv_timeout(deadline.saturating_duration_since(std::time::Instant::now()))
            .expect("native overflow event")
            .unwrap();
        if event.need_rescan() {
            println!("RESCAN");
            return;
        }
    }
}

#[test]
fn suspended_native_watcher_reports_kernel_queue_loss() {
    let directory = tempfile::tempdir().unwrap();
    let count: usize = std::fs::read_to_string("/proc/sys/fs/inotify/max_queued_events")
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert!(
        count <= 262_144,
        "a dedicated overflow runner is required for this queue size"
    );
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "watcher_overflow_child", "--nocapture"])
        .env("REVIEW_OVERFLOW_PROBE_PATH", directory.path())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            if send.send(line.unwrap()).is_err() {
                break;
            }
        }
    });
    while !receive
        .recv_timeout(Duration::from_secs(15))
        .unwrap()
        .contains("WATCHING")
    {}
    assert!(Command::new("kill")
        .args(["-STOP", &child.id().to_string()])
        .status()
        .unwrap()
        .success());
    for index in 0..count + 1024 {
        let path = directory.path().join(format!("event-{index}"));
        std::fs::write(&path, "event").unwrap();
        std::fs::remove_file(path).unwrap();
    }
    assert!(Command::new("kill")
        .args(["-CONT", &child.id().to_string()])
        .status()
        .unwrap()
        .success());
    let mut rescan = false;
    while let Ok(line) = receive.recv_timeout(Duration::from_secs(60)) {
        rescan |= line.contains("RESCAN");
    }
    let status = child.wait().unwrap();
    assert!(status.success());
    assert!(rescan, "the real OS queue must emit need_rescan");
}
