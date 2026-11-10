//! Proof file for PR 651. Do not merge.

fn main() {
    let _ = std::process::Command::new("git").arg("status").status();
}
