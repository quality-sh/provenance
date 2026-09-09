//! Canonical and live evidence kept separate in client fixtures.
use super::Repository;
use provenance_core::ScopeId;
use provenance_store::shards;
use serde_json::{json, Value};
use std::io::Write;

impl Repository {
    /// Seed two rows per evidence collection and two real source revisions.
    pub fn evidence(&self) -> String {
        let scope = ScopeId::new("default").unwrap();
        std::fs::write(
            self.dir.path().join("code.rs"),
            "#[rule(\"rule_shared\")]\nfn check() { let value = 1; }\n",
        )
        .unwrap();
        git(self, &["init", "-q"]);
        git(self, &["add", "code.rs"]);
        git(self, &["commit", "-qm", "Initial source"]);
        let base = git(self, &["rev-parse", "HEAD"]);
        std::fs::write(
            self.dir.path().join("code.rs"),
            "#[rule(\"rule_shared\")]\nfn check() { let value = 2; }\n",
        )
        .unwrap();
        git(self, &["add", "code.rs"]);
        git(self, &["commit", "-qm", "Changed source"]);
        for index in 0..2 {
            append(
                shards::implementation_bindings_path(&self.layout, &scope),
                &json!({"schema_version":2,"scope_id":"default","id":format!("implementation_fixture_{index}"),"rule_id":"rule_shared","declared_by":"fixture","file":"code.rs","symbol":"check"}),
            );
            append(
                shards::verification_bindings_path(&self.layout, &scope),
                &json!({"schema_version":2,"scope_id":"default","id":format!("binding_fixture_{index}"),"rule_id":"rule_shared","key":format!("fixture_{index}"),"method":"examples","declared_by":"fixture","file":"code.rs","symbol":"check"}),
            );
            append(
                shards::requirement_reviews_path(&self.layout, &scope),
                &json!({"schema_version":2,"scope_id":"default","id":format!("review_fixture_{index}"),"rule_id":"rule_shared","requirement_id":"req_shared","field":"statement","before":"Old text.","after":"New text.","changed_at":index}),
            );
            append(
                self.layout.verification_runs_path(&scope),
                &json!({"schema_version":2,"scope_id":"default","id":format!("run_fixture_{index}"),"binding_id":format!("binding_fixture_{index}"),"rule_id":"rule_shared","method":"examples","declared_by":"fixture","file":"code.rs","symbol":"check","status":"passed","started_at":index,"completed_at":index+1}),
            );
        }
        base
    }
}
fn append(path: impl AsRef<std::path::Path>, value: &Value) {
    let path = path.as_ref();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .unwrap();
    writeln!(file, "{value}").unwrap();
}
fn git(repo: &Repository, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .args([
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
        ])
        .args(args)
        .current_dir(repo.dir.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}
