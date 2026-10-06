use std::{
    os::unix::fs::PermissionsExt as _,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

/// A real opener process that stays alive until the fixture is dropped.
pub struct PersistentBrowser {
    directory: tempfile::TempDir,
}

impl PersistentBrowser {
    pub fn install() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let program = directory.path().join("browser.sh");
        std::fs::write(
            &program,
            "#!/bin/sh\nprintf '%s' \"$$\" > \"$BROWSER_PID_FILE\"\nexec sleep 120\n",
        )
        .unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
        Self { directory }
    }

    pub fn configure(&self, command: &mut Command) {
        command
            .env("BROWSER", self.directory.path().join("browser.sh"))
            .env("BROWSER_PID_FILE", self.directory.path().join("pid"))
            .env("DISPLAY", ":0")
            .env_remove("SSH_CONNECTION")
            .env_remove("SSH_CLIENT")
            .env_remove("SSH_TTY");
    }

    pub fn wait_until_running(&self) {
        let deadline = Instant::now() + Duration::from_secs(15);
        while self.pid().is_none() {
            assert!(Instant::now() < deadline, "browser did not start");
            std::thread::sleep(Duration::from_millis(25));
        }
        self.assert_running();
    }

    fn pid(&self) -> Option<String> {
        let pid = std::fs::read_to_string(self.directory.path().join("pid")).ok()?;
        (!pid.is_empty()).then_some(pid)
    }

    pub fn assert_running(&self) {
        assert!(Command::new("kill")
            .args(["-0", &self.pid().unwrap()])
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success());
    }
}

impl Drop for PersistentBrowser {
    fn drop(&mut self) {
        if let Some(pid) = self.pid() {
            let _ = Command::new("kill")
                .args(["-TERM", &pid])
                .stderr(Stdio::null())
                .status();
        }
    }
}
