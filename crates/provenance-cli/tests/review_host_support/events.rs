//! Real WebSocket client for review event tests.
use super::{redeem, request, waits, Host};
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    net::TcpStream,
    path::Path,
    time::Duration,
};

pub fn cookie(host: &Host) -> String {
    let key = std::fs::read_to_string(host.launch_key_path()).unwrap();
    let issued = request(host, "POST", "/review-launch/code", false)
        .send_string(&json!({"launchKey": key}).to_string())
        .unwrap();
    let issued: Value = serde_json::from_str(&issued.into_string().unwrap()).unwrap();
    let session = redeem(host, issued["code"].as_str().unwrap());
    assert_eq!(session.status(), 200);
    session
        .header("Set-Cookie")
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned()
}

pub struct Socket(pub TcpStream);

impl Socket {
    pub fn connect(host: &Host) -> Self {
        let (status, socket) = upgrade(
            host,
            "/review-events",
            &format!(
                "Cookie: {}\r\nOrigin: {}\r\nSec-Fetch-Site: same-origin\r\n",
                cookie(host),
                host.config["endpoint"].as_str().unwrap()
            ),
        );
        assert_eq!(status, 101, "authenticated event route must upgrade");
        socket
    }

    pub fn frame(&mut self) -> Value {
        self.0.set_read_timeout(Some(waits::LIMIT)).unwrap();
        let mut head = [0; 2];
        self.0
            .read_exact(&mut head)
            .expect("server sends an event frame");
        assert_eq!(head[0], 0x81, "complete text frame");
        assert_eq!(head[1] & 0x80, 0, "server frame is unmasked");
        let length = match head[1] {
            126 => {
                let mut n = [0; 2];
                self.0.read_exact(&mut n).unwrap();
                u64::from(u16::from_be_bytes(n))
            }
            127 => {
                let mut n = [0; 8];
                self.0.read_exact(&mut n).unwrap();
                u64::from_be_bytes(n)
            }
            n => u64::from(n),
        };
        assert!(length < 1024 * 1024);
        let mut bytes = vec![0; usize::try_from(length).unwrap()];
        self.0.read_exact(&mut bytes).unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    /// Sends a masked text frame, as a browser does.
    pub fn send(&mut self, value: &Value) {
        let payload = value.to_string().into_bytes();
        assert!(payload.len() < 126);
        let mask = [0x12, 0x34, 0x56, 0x78];
        let mut bytes = vec![0x81, 0x80 | u8::try_from(payload.len()).unwrap()];
        bytes.extend(mask);
        bytes.extend(
            payload
                .iter()
                .enumerate()
                .map(|(index, byte)| byte ^ mask[index % 4]),
        );
        self.0.write_all(&bytes).unwrap();
    }

    pub fn quiet(&mut self) {
        self.0
            .set_read_timeout(Some(Duration::from_millis(250)))
            .unwrap();
        let mut byte = [0];
        let Err(error) = self.0.peek(&mut byte) else {
            panic!("no duplicate invalidation: {}", self.frame());
        };
        assert!(matches!(
            error.kind(),
            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
        ));
        self.0
            .set_read_timeout(Some(Duration::from_secs(15)))
            .unwrap();
    }
}

pub fn upgrade(host: &Host, path: &str, headers: &str) -> (u16, Socket) {
    upgrade_with_host(host, path, headers, host.address())
}

pub fn upgrade_with_host(host: &Host, path: &str, headers: &str, authority: &str) -> (u16, Socket) {
    let address = host.address().parse().unwrap();
    let mut socket = TcpStream::connect_timeout(&address, waits::LIMIT).unwrap();
    socket.set_write_timeout(Some(waits::LIMIT)).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(15)))
        .unwrap();
    write!(socket, "GET {path} HTTP/1.1\r\nHost: {}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n{headers}\r\n", authority).unwrap();
    let mut bytes = Vec::new();
    while !bytes.ends_with(b"\r\n\r\n") {
        let mut byte = [0];
        socket.read_exact(&mut byte).unwrap();
        bytes.push(byte[0]);
        assert!(bytes.len() < 8192);
    }
    let response = String::from_utf8(bytes).unwrap();
    let status = response.split_whitespace().nth(1).unwrap().parse().unwrap();
    (status, Socket(socket))
}

pub fn source(host: &Host, root: &Path, id: &str) {
    source_with(host.cli(), root, id);
}

pub fn source_before_host(root: &Path, id: &str) {
    source_with(
        std::process::Command::new(assert_cmd::cargo::cargo_bin("provenance")),
        root,
        id,
    );
}

fn source_with(mut command: std::process::Command, root: &Path, id: &str) {
    command.args([
        "sources",
        "create",
        "--repo",
        root.to_str().unwrap(),
        "--id",
        id,
        "--name",
        id,
    ]);
    let output = waits::output(&mut command);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Waits through public upgrade refusals while native observation recovers.
pub fn connect_when_available(host: &Host) -> Socket {
    let headers = format!(
        "Cookie: {}\r\nOrigin: {}\r\nSec-Fetch-Site: same-origin\r\n",
        cookie(host),
        host.config["endpoint"].as_str().unwrap()
    );
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    loop {
        let (status, socket) = upgrade(host, "/review-events", &headers);
        if status == 101 {
            return socket;
        }
        assert_eq!(
            status, 503,
            "only observation unavailability permits a retry"
        );
        assert!(
            std::time::Instant::now() < deadline,
            "native observation did not recover"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
}
