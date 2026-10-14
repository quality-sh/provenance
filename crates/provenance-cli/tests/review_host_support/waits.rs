//! Deadlines for real host test operations.
use provenance_store::{layout::ProvenanceLayout, publication::PublicationGuard};
use std::{
    io::Read,
    process::{Command, Output, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};

pub const LIMIT: Duration = Duration::from_secs(60);

pub async fn guard(layout: &ProvenanceLayout) -> PublicationGuard {
    tokio::time::timeout(
        LIMIT,
        provenance_store::publication::publication_guard(layout),
    )
    .await
    .expect("publication lock acquisition exceeded 60 seconds")
    .unwrap()
}

pub fn output(command: &mut Command) -> Output {
    let description = format!("{command:?}");
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let stdout = capture(child.stdout.take().unwrap());
    let stderr = capture(child.stderr.take().unwrap());
    let deadline = Instant::now() + LIMIT;
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("CLI process exceeded 60 seconds: {description}");
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    Output {
        status,
        stdout: stdout
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("CLI stdout did not close within 60 seconds"),
        stderr: stderr
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("CLI stderr did not close within 60 seconds"),
    }
}

fn capture(mut pipe: impl Read + Send + 'static) -> mpsc::Receiver<Vec<u8>> {
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        pipe.read_to_end(&mut bytes).unwrap();
        let _ = send.send(bytes);
    });
    receive
}
