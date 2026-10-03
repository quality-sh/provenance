use std::process::Command;
use std::time::Duration;

pub fn open(url: &str, no_open: bool) {
    if no_open {
        return;
    }
    if remote_session() {
        eprintln!("Review link was not opened because this is an SSH session: {url}");
        return;
    }
    #[cfg(target_os = "linux")]
    if !graphical_linux_session() {
        eprintln!("Review link was not opened because no graphical display is available: {url}");
        return;
    }
    match opener(url).spawn() {
        Ok(mut child) => {
            let (send, receive) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let _ = send.send(child.wait());
            });
            if let Ok(result) = receive.recv_timeout(Duration::from_millis(250)) {
                match result {
                    Ok(status) if status.success() => {}
                    Ok(status) => eprintln!(
                        "warning: cannot open the review link (opener exited with {status}): {url}"
                    ),
                    Err(error) => {
                        eprintln!("warning: cannot open the review link ({error}): {url}")
                    }
                }
            }
        }
        Err(error) => {
            eprintln!("warning: cannot open the review link ({error}): {url}");
        }
    }
}

fn remote_session() -> bool {
    ["SSH_CONNECTION", "SSH_CLIENT", "SSH_TTY"]
        .iter()
        .any(|name| environment_has_value(name))
}

#[cfg(target_os = "linux")]
fn graphical_linux_session() -> bool {
    environment_has_value("DISPLAY") || environment_has_value("WAYLAND_DISPLAY")
}

fn environment_has_value(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|value| !value.is_empty())
}

#[cfg(target_os = "macos")]
fn opener(url: &str) -> Command {
    let mut command = Command::new("open");
    command.arg(url);
    command
}

#[cfg(target_os = "linux")]
fn opener(url: &str) -> Command {
    let mut command = Command::new("xdg-open");
    command.arg(url);
    command
}

#[cfg(target_os = "windows")]
fn opener(url: &str) -> Command {
    let mut command = Command::new("cmd");
    command.args(["/C", "start", "", url]);
    command
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn opener(_url: &str) -> Command {
    Command::new("provenance-browser-opener-is-not-supported")
}
