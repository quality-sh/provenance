//! A local Responses provider for the real Codex CLI.
use crate::review_host_support::waits;
use serde_json::{json, Value};
use std::{io::{BufRead, BufReader, Read, Write}, net::TcpListener, path::Path, process::Command, sync::{atomic::{AtomicBool, Ordering}, Arc}, time::Duration};

pub struct Codex {
    home: tempfile::TempDir,
    stopped: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl Codex {
    pub fn new() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let stopped = Arc::new(AtomicBool::new(false));
        let stop = stopped.clone();
        let worker = std::thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut socket, _)) => {
                        socket.set_read_timeout(Some(Duration::from_secs(15))).unwrap();
                        let mut reader = BufReader::new(socket.try_clone().unwrap());
                        let mut length = 0;
                        loop {
                            let mut line = String::new();
                            reader.read_line(&mut line).unwrap();
                            if line == "\r\n" || line.is_empty() { break; }
                            if let Some(value) = line.to_lowercase().strip_prefix("content-length:") {
                                length = value.trim().parse().unwrap();
                            }
                        }
                        let mut body = vec![0; length];
                        reader.read_exact(&mut body).unwrap();
                        let request: Value = serde_json::from_slice(&body).unwrap();
                        let texts = request["input"].as_array().unwrap().iter()
                            .filter(|item| item["role"] == "user")
                            .flat_map(|item| item["content"].as_array().into_iter().flatten())
                            .filter_map(|item| item["text"].as_str())
                            .filter(|text| text.starts_with("Provenance record data."))
                            .collect::<Vec<_>>().join("\n");
                        let item = json!({"id":"fixture_message","type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":if texts.is_empty() {"READY"} else {&texts},"annotations":[]}]});
                        let response = json!({"id":"fixture_response","object":"response","status":"completed","output":[item],"usage":{"input_tokens":1,"output_tokens":1,"total_tokens":2}});
                        let events = [json!({"type":"response.created","response":{"id":"fixture_response","status":"in_progress","output":[]}}),json!({"type":"response.output_item.added","output_index":0,"item":item}),json!({"type":"response.output_item.done","output_index":0,"item":item}),json!({"type":"response.completed","response":response})];
                        let body = events.iter().enumerate().map(|(index, event)| {
                            let mut event = event.clone(); event["sequence_number"] = json!(index);
                            format!("event: {}\ndata: {}\n\n", event["type"].as_str().unwrap(), event)
                        }).collect::<String>();
                        let _ = write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(10)),
                    Err(error) => panic!("fixture accept: {error}"),
                }
            }
        });
        let home = tempfile::tempdir().unwrap();
        std::fs::write(home.path().join("config.toml"), format!("model = \"fixture\"\nmodel_provider = \"fixture\"\n[model_providers.fixture]\nname = \"Fixture\"\nbase_url = \"http://{address}\"\nwire_api = \"responses\"\nrequires_openai_auth = false\n")).unwrap();
        Self { home, stopped, worker: Some(worker) }
    }
    pub fn home(&self) -> &Path { self.home.path() }
    pub fn thread(&self, repository: &Path) -> String {
        let output = waits::output(Command::new("codex").env("CODEX_HOME", self.home()).current_dir(repository).args(["exec", "--skip-git-repo-check", "--json", "Reply READY."]));
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        String::from_utf8(output.stdout).unwrap().lines().filter_map(|line| serde_json::from_str::<Value>(line).ok()).find_map(|value| value["thread_id"].as_str().map(str::to_owned)).expect("real Codex thread")
    }
    pub fn resume(&self, repository: &Path, thread: &str) -> String {
        let output = waits::output(Command::new("codex").env("CODEX_HOME", self.home()).current_dir(repository).args(["exec", "resume", "--skip-git-repo-check", "--json", thread, "Report received record data."]));
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        String::from_utf8(output.stdout).unwrap().lines().filter_map(|line| serde_json::from_str::<Value>(line).ok()).filter_map(|value| value["item"]["text"].as_str().map(str::to_owned)).collect::<Vec<_>>().join("\n")
    }
}
impl Drop for Codex {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Relaxed);
        self.worker.take().unwrap().join().unwrap();
    }
}
