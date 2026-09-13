//! Loopback HTTP fixture that serves the synthetic dictionary PDF to the
//! `init` child.
//!
//! Copies the pattern from `tests/cli_ste_onboarding.rs`: one std-only
//! `TcpListener` on 127.0.0.1, one worker thread, one response per
//! connection, and a wake-up socket on drop. `PROVENANCE_TEST_STE100_ASSET_URL`
//! accepts loopback hosts only, so this server is the one place the child can
//! fetch the dictionary from, and no packet leaves the machine.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

pub(super) struct FixtureServer {
    address: std::net::SocketAddr,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl FixtureServer {
    pub(super) fn start(body: Vec<u8>) -> std::io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        listener.set_nonblocking(true)?;
        let address = listener.local_addr()?;
        let stop = Arc::new(AtomicBool::new(false));
        let shared_stop = Arc::clone(&stop);
        let thread = thread::spawn(move || {
            while !shared_stop.load(Ordering::SeqCst) {
                match listener.accept() {
                    // The wake-up socket can land here before the stop flag is
                    // observed; never serve once shutdown has begun.
                    Ok(_) if shared_stop.load(Ordering::SeqCst) => break,
                    Ok((stream, _)) => serve(stream, &body),
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                    }
                    // One bad connection must not end the fixture; drop it
                    // and keep accepting until shutdown.
                    Err(_) => thread::sleep(Duration::from_millis(5)),
                }
            }
        });
        Ok(Self {
            address,
            stop,
            thread: Some(thread),
        })
    }

    /// The URL handed to the child through `PROVENANCE_TEST_STE100_ASSET_URL`.
    pub(super) fn asset_url(&self) -> String {
        format!("http://{}/ASD-STE100_ISSUE9.pdf", self.address)
    }
}

impl Drop for FixtureServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // Connect once so the nonblocking accept wakes up and sees the flag.
        let _ = TcpStream::connect(self.address);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn serve(mut stream: TcpStream, body: &[u8]) {
    // BSD-derived systems make accepted sockets inherit the listener's
    // O_NONBLOCK; restore blocking mode so writes drain fully.
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 1024];
    while !bytes.ends_with(b"\r\n\r\n") {
        // A torn or instantly-closed connection must never panic the worker.
        let read = match stream.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(read) => read,
        };
        bytes.extend_from_slice(&buffer[..read]);
    }
    if bytes.is_empty() {
        return;
    }
    let _ = write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(body);
}
