//! Real WebSocket client for review event tests.
use super::{redeem, request, Host};
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

    pub fn quiet(&mut self) {
        self.0
            .set_read_timeout(Some(Duration::from_millis(250)))
            .unwrap();
        let mut byte = [0];
        let error = self
            .0
            .read(&mut byte)
            .expect_err("no duplicate invalidation");
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
    let mut socket = TcpStream::connect(host.address()).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(15)))
        .unwrap();
    write!(socket, "GET {path} HTTP/1.1\r\nHost: {}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n{headers}\r\n", host.address()).unwrap();
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
    let output = host
        .cli()
        .args([
            "sources",
            "create",
            "--repo",
            root.to_str().unwrap(),
            "--id",
            id,
            "--name",
            id,
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
