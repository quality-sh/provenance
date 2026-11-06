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
    #[test]
    fn git_command_runs_git_and_returns_its_output() {
        let output = super::git().arg("--version").output().unwrap();
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).starts_with("git version"));
    }

    #[cfg(windows)]
    #[test]
    fn a_child_started_without_a_window_has_no_console_window() {
        // GetConsoleWindow gives 0 in a console process that has no window.
        let script = "(Add-Type -Name ConsoleWindow -PassThru -MemberDefinition \
            '[DllImport(\"kernel32.dll\")] public static extern IntPtr GetConsoleWindow();')\
            ::GetConsoleWindow()";
        let output = super::without_window(std::process::Command::new("powershell"))
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "0");
    }
}
