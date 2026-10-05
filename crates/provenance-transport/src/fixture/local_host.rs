//! Shared local-host fixture for transport and CLI integration tests.

use crate::local_host::{LocalHostIdentity, LocalHostRegistration, IDENTITY_ROUTE};
use serde_json::Value;
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
    time::Duration,
};

pub struct LocalHostFixture {
    pub endpoint: String,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    registration: Option<LocalHostRegistration>,
}

impl LocalHostFixture {
    pub fn start(root: &Path, scope: &str, repository_id: &str) -> Self {
        Self::start_with_identity(root, scope, repository_id, |identity| {
            serde_json::to_value(identity).unwrap()
        })
    }

    pub fn start_with_identity(
        root: &Path,
        scope: &str,
        repository_id: &str,
        response: impl FnOnce(LocalHostIdentity) -> Value,
    ) -> Self {
        let (listener, endpoint) = listener();
        let registration =
            LocalHostRegistration::publish(root, scope, &endpoint, repository_id).unwrap();
        let body = response(registration.identity()).to_string();
        Self::serve(listener, endpoint, body, Some(registration))
    }

    pub fn identity(&self) -> LocalHostIdentity {
        self.registration.as_ref().unwrap().identity()
    }

    pub fn stop_listener(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect(self.endpoint.trim_start_matches("http://"));
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }

    fn serve(
        listener: TcpListener,
        endpoint: String,
        body: String,
        registration: Option<LocalHostRegistration>,
    ) -> Self {
        let (stop, thread) = serve(listener, body);
        Self {
            endpoint,
            stop,
            thread: Some(thread),
            registration,
        }
    }
}

impl Drop for LocalHostFixture {
    fn drop(&mut self) {
        self.stop_listener();
    }
}

fn listener() -> (TcpListener, String) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    (listener, endpoint)
}

fn serve(listener: TcpListener, body: String) -> (Arc<AtomicBool>, JoinHandle<()>) {
    listener.set_nonblocking(true).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = stop.clone();
    let thread = std::thread::spawn(move || {
        while !thread_stop.load(Ordering::Relaxed) {
            let Ok((mut stream, _)) = listener.accept() else {
                std::thread::sleep(Duration::from_millis(5));
                continue;
            };
            // Accepted sockets can inherit non-blocking mode on macOS.
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(30)))
                .unwrap();
            let mut request = [0_u8; 2048];
            let count = stream.read(&mut request).unwrap_or(0);
            let path_matches = String::from_utf8_lossy(&request[..count])
                .starts_with(&format!("GET {IDENTITY_ROUTE} HTTP/1.1"));
            let (status, response) = if path_matches {
                ("200 OK", body.as_str())
            } else {
                ("404 Not Found", "{}")
            };
            write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",
                response.len()
            )
            .unwrap();
        }
    });
    (stop, thread)
}
