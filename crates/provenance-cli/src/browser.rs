//! Opens review links in the browser of the person who runs the CLI.

use provenance_macros::rule;
use std::{
    io::Write as _,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

pub enum Opening {
    Opened,
    Printed(NotOpened),
}

/// The reason that the CLI did not open a review link.
pub enum NotOpened {
    Requested,
    RemoteSession,
    NoDisplay,
    OpenerFailed,
}

impl NotOpened {
    pub const fn reason(&self) -> &'static str {
        match self {
            Self::Requested => "the caller asked not to open it",
            Self::RemoteSession => "this is an SSH session",
            Self::NoDisplay => "no graphical display is available",
            Self::OpenerFailed => "the browser did not start",
        }
    }
}

/// Opens the link in a browser unless the caller asks not to or no local browser is available.
///
/// The browser gets the path of an owner-only redirect page, because other users can read
/// process arguments and the link carries a launch code.
#[rule("rule_review_commands_open_the_browser")]
pub fn open_or_print(link: &url::Url, no_open: bool) -> Opening {
    if no_open {
        return Opening::Printed(NotOpened::Requested);
    }
    if ["SSH_CONNECTION", "SSH_CLIENT", "SSH_TTY"]
        .iter()
        .any(|name| is_set(name))
    {
        return Opening::Printed(NotOpened::RemoteSession);
    }
    if cfg!(target_os = "linux") && !is_set("DISPLAY") && !is_set("WAYLAND_DISPLAY") {
        return Opening::Printed(NotOpened::NoDisplay);
    }
    if redirect_page(link).and_then(|page| launch(&page)).is_ok() {
        Opening::Opened
    } else {
        Opening::Printed(NotOpened::OpenerFailed)
    }
}

fn is_set(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|value| !value.is_empty())
}

fn redirect_page(link: &url::Url) -> std::io::Result<PathBuf> {
    let target = link
        .as_str()
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;");
    // A new temporary file is readable and writable only by its owner.
    let mut page = tempfile::Builder::new()
        .prefix("provenance-review-")
        .suffix(".html")
        .tempfile()?;
    writeln!(
        page,
        "<!doctype html>\n<meta charset=\"utf-8\">\n<meta name=\"referrer\" content=\"no-referrer\">\n\
         <meta http-equiv=\"refresh\" content=\"0;url={target}\">\n<title>Provenance review</title>\n\
         <a href=\"{target}\">Open the review page</a>"
    )?;
    page.flush()?;
    // The browser can read the page after this process stops.
    let (_, path) = page.keep().map_err(|error| error.error)?;
    Ok(path)
}

fn launch(page: &Path) -> std::io::Result<()> {
    if let Some(browser) = std::env::var_os("BROWSER").filter(|value| !value.is_empty()) {
        let status = Command::new(browser)
            .arg(page)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .status()?;
        return if status.success() {
            Ok(())
        } else {
            Err(std::io::Error::other("the BROWSER program failed"))
        };
    }
    system_open(page)
}

#[cfg(windows)]
fn system_open(page: &Path) -> std::io::Result<()> {
    // Explorer opens the page with the default browser. Its exit status does not show success.
    Command::new("explorer.exe")
        .arg(page)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .status()
        .map(|_| ())
}

#[cfg(not(windows))]
fn system_open(page: &Path) -> std::io::Result<()> {
    open::that(page)
}
