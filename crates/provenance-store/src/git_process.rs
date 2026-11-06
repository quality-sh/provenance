//! The one place in the workspace that starts Git.
//!
//! A process without a console, for example the detached review host or an MCP server
//! that a desktop application starts, gets a new console window for each console child
//! on Windows. The command from [`git`] sets `CREATE_NO_WINDOW` there, so a Git start
//! shows no window. A test in this crate refuses `Command::new("git")` in other source
//! files.

use std::process::Command;

/// The Windows process creation flag that starts a console child without a window.
#[cfg(windows)]
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// A command that runs `git` from `PATH`. On Windows the command opens no console window.
pub fn git() -> Command {
    without_window(Command::new("git"))
}

/// Sets `CREATE_NO_WINDOW` on Windows. Other systems open no window for a child.
#[cfg_attr(not(windows), allow(unused_mut, clippy::missing_const_for_fn))]
fn without_window(mut command: Command) -> Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

#[cfg(test)]
mod tests {
    /// Implementation aid: no Rule states how Git starts. This checks that the command
    /// from `git()` finds Git on `PATH` and gives its output.
    #[test]
    fn git_command_runs_git_and_returns_its_output() {
        let output = super::git().arg("--version").output().unwrap();
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).starts_with("git version"));
    }

    #[cfg(windows)]
    const PROBE: &str = "PROVENANCE_CONSOLE_WINDOW_PROBE";

    /// Implementation aid: no Rule states how Git starts (bead provenance-a5ru). A copy of
    /// this test binary named `git.exe` is first on `PATH`, so `git()` starts the copy, and
    /// the copy reports its console window. A child without the flag gets the console of
    /// this process, or a new console with a window, so it reports a window.
    #[cfg(windows)]
    #[test]
    fn git_starts_a_child_that_has_no_console_window() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::copy(
            std::env::current_exe().unwrap(),
            directory.path().join("git.exe"),
        )
        .unwrap();
        let output = super::git()
            .env("PATH", directory.path())
            .env(PROBE, "1")
            .args([
                "--exact",
                "git_process::tests::report_console_window",
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success(),
            "{stdout}{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(stdout.contains("console window: none"), "{stdout}");
    }

    /// The child side of `git_starts_a_child_that_has_no_console_window`.
    #[cfg(windows)]
    #[test]
    #[ignore = "a child process of git_starts_a_child_that_has_no_console_window"]
    fn report_console_window() {
        if std::env::var_os(PROBE).is_none() {
            return;
        }
        #[link(name = "kernel32")]
        extern "system" {
            fn GetConsoleWindow() -> *mut std::ffi::c_void;
        }
        // SAFETY: GetConsoleWindow takes no arguments and only returns a handle.
        let window = unsafe { GetConsoleWindow() };
        println!(
            "console window: {}",
            if window.is_null() { "none" } else { "open" }
        );
    }
}
