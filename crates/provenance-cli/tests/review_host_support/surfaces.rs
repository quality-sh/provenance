//! One API request through the CLI `api` command, the HTTP review host or the
//! MCP `api` tool, with the answer of each surface in one form.

use provenance_mcp::Client;
use serde_json::{json, Value};
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Clone, Copy, Debug)]
pub enum Surface {
    Cli,
    Http,
    Mcp,
}

pub const SURFACES: [Surface; 3] = [Surface::Cli, Surface::Http, Surface::Mcp];

impl Surface {
    /// The CLI exit code, the HTTP status or the MCP `isError` of a request that lands.
    pub fn landed(self) -> Value {
        match self {
            Self::Cli => json!(0),
            Self::Http => json!(200),
            Self::Mcp => json!(false),
        }
    }

    /// The CLI exit code, the HTTP status or the MCP `isError` of a refusal with
    /// the HTTP status `status`.
    pub fn refused(self, status: u16) -> Value {
        match self {
            Self::Cli => json!(1),
            Self::Http => json!(status),
            Self::Mcp => json!(true),
        }
    }

    /// Sends `input` with `method` to `path` of the repository at `root`.
    pub fn call(self, root: &Path, method: &str, path: &str, input: Option<&Value>) -> Answer {
        match self {
            Self::Cli => cli(root, method, path, input),
            Self::Http => http(root, method, path, input),
            Self::Mcp => mcp(root, method, path, input),
        }
    }
}

/// The answer to one request.
pub struct Answer {
    /// The code as [`Surface::landed`] and [`Surface::refused`] give it.
    pub code: Value,
    /// The `data` of the answer, or its `error`.
    pub body: Value,
    /// The complete text of the answer: the CLI output stream that holds it,
    /// the HTTP body, or the MCP result.
    pub text: String,
}

fn cli(root: &Path, method: &str, path: &str, input: Option<&Value>) -> Answer {
    let mut command = Command::new(assert_cmd::cargo::cargo_bin("provenance"));
    command.args(["api", path, "--repo", root.to_str().unwrap()]);
    command.args(["--format", "json", "--method", method]);
    if input.is_some() {
        command.args(["--input", "-"]);
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    if let Some(input) = input {
        std::io::Write::write_all(&mut stdin, input.to_string().as_bytes()).unwrap();
    }
    drop(stdin);
    let output = child.wait_with_output().unwrap();
    let code = json!(output.status.code().unwrap());
    if output.status.success() {
        let text = String::from_utf8(output.stdout).unwrap();
        let answer: Value = serde_json::from_str(&text).unwrap();
        return Answer {
            code,
            body: answer["data"].clone(),
            text,
        };
    }
    assert!(output.stdout.is_empty(), "{output:?}");
    let text = String::from_utf8(output.stderr).unwrap();
    let envelope: Value =
        serde_json::from_str(text.trim()).unwrap_or_else(|error| panic!("{error}: {text}"));
    Answer {
        code,
        body: envelope["error"].clone(),
        text,
    }
}

fn http(root: &Path, method: &str, path: &str, input: Option<&Value>) -> Answer {
    let host = super::start(root);
    let request = super::request(&host, &method.to_uppercase(), &format!("/{path}"), true);
    let answer = super::response(match input {
        Some(input) => request
            .set("Content-Type", "application/json")
            .send_string(&json!({ "data": input }).to_string()),
        None => request.call(),
    });
    let code = json!(answer.status());
    let text = answer.into_string().unwrap();
    let envelope: Value = serde_json::from_str(&text).unwrap();
    Answer {
        code,
        body: data_or_error(&envelope),
        text,
    }
}

fn mcp(root: &Path, method: &str, path: &str, input: Option<&Value>) -> Answer {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let mut child = tokio::process::Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
            .args(["mcp", "--repo", root.to_str().unwrap()])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let io = tokio::io::join(child.stdout.take().unwrap(), child.stdin.take().unwrap());
        let client = Client::connect(io).await.unwrap();
        let mut arguments = json!({"path": path, "method": method});
        if let Some(input) = input {
            arguments["input"] = input.clone();
        }
        let result = client.call("api", arguments).await;
        client.cancel().await.unwrap();
        assert!(child.wait().await.unwrap().success());
        Answer {
            code: result["isError"].clone(),
            body: data_or_error(&result["structuredContent"]),
            text: result.to_string(),
        }
    })
}

fn data_or_error(envelope: &Value) -> Value {
    if envelope["error"].is_null() {
        envelope["data"].clone()
    } else {
        envelope["error"].clone()
    }
}
