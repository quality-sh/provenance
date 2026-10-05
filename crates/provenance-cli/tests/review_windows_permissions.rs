#![cfg(windows)]

#[path = "review_host_support/mod.rs"]
mod review_host_support;

use review_host_support::{repository, start_with};
use std::{
    path::Path,
    process::Command,
    time::{Duration, Instant},
};

fn assert_owner_only(path: &Path) {
    // Independent ACL inspection: every allowed right must belong to the current user.
    let script = r#"
$ErrorActionPreference = 'Stop'
$acl = Get-Acl -LiteralPath $env:REVIEW_SECRET_FILE
$user = [System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value
$owner = $acl.GetOwner([System.Security.Principal.SecurityIdentifier]).Value
if ($owner -ne $user) { throw 'The file owner is not the current user' }
if (-not $acl.AreAccessRulesProtected) { throw 'The file inherits access rights' }
$rules = $acl.GetAccessRules($true, $true, [System.Security.Principal.SecurityIdentifier])
$allowed = @($rules | Where-Object { $_.AccessControlType -eq 'Allow' })
if ($allowed.Count -eq 0) { throw 'The owner has no access' }
foreach ($rule in $allowed) {
    if ($rule.IsInherited) { throw 'An access rule is inherited' }
    if ($rule.IdentityReference.Value -ne $user -and $rule.IdentityReference.Value -ne 'S-1-3-4') {
        throw 'Another user has access'
    }
}
"#;
    let output = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .env("REVIEW_SECRET_FILE", path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
/// Security aid: this flow checks the ACLs of both secret files from a real host launch.
fn launch_key_and_redirect_page_allow_only_the_owner() {
    let repo = repository();
    let browser = tempfile::tempdir().unwrap();
    let program = browser.path().join("browser.cmd");
    let receipt = browser.path().join("page-path.txt");
    std::fs::write(&program, "@echo %~1>\"%BROWSER_RECEIPT%\"\r\n").unwrap();
    let host = start_with(repo.path(), |command| {
        command
            .env("BROWSER", &program)
            .env("BROWSER_RECEIPT", &receipt);
        for name in ["SSH_CONNECTION", "SSH_CLIENT", "SSH_TTY"] {
            command.env_remove(name);
        }
    });
    let deadline = Instant::now() + Duration::from_secs(15);
    let page = loop {
        if let Ok(value) = std::fs::read_to_string(&receipt) {
            if !value.trim().is_empty() {
                break std::path::PathBuf::from(value.trim());
            }
        }
        assert!(
            Instant::now() < deadline,
            "The browser did not receive a page"
        );
        std::thread::sleep(Duration::from_millis(25));
    };
    assert_owner_only(&host.launch_key_path());
    assert_owner_only(&page);
    std::fs::remove_file(page).unwrap();
}
