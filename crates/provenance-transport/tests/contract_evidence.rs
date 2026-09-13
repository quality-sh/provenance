#![cfg(feature = "test-fixture")]
#[allow(dead_code)]
#[path = "support/records.rs"]
mod records;
use records::{call, host, Repository};
use serde_json::json;

#[tokio::test]
async fn six_evidence_reads_are_registered_and_preserve_native_shapes() {
    let repo = Repository::new("The evidence is readable.");
    std::fs::write(
        repo.dir.path().join("code.rs"),
        "#[rule(\"rule_shared\")]\nfn check() {}\n",
    )
    .unwrap();
    let host = host(&[("selected", &repo)], &["selected"]);
    for (operation, request) in [
        ("impact", json!({"id":"req_shared"})),
        ("resolve-symbol", json!({"file":"code.rs"})),
        (
            "evidence",
            json!({"rule":"rule_shared", "head":"ignored-without-base"}),
        ),
        ("verification-runs", json!({})),
        ("verification-bindings", json!({"rule":"rule_shared"})),
    ] {
        let (status, answer) = call(
            &host,
            operation,
            json!({"context":{"repository":"selected","scope":"default"},"request":request}),
        )
        .await;
        assert_eq!(status, 200, "{operation}: {answer}");
        if operation.starts_with("verification-") {
            assert!(answer.is_array());
        } else {
            assert_eq!(answer["operation"], operation);
        }
        if operation == "evidence" {
            assert!(answer["stale"].is_null());
            assert_eq!(answer["stamp"]["live"], json!(["verification_runs"]));
        }
    }
    let (status, answer) = call(
        &host,
        "stale",
        json!({"context":{"repository":"selected","scope":"default"},"request":{"base":"HEAD"}}),
    )
    .await;
    assert_ne!(status, 404, "{answer}");
    host.shutdown().await;
}

#[tokio::test]
async fn verification_lists_reject_unused_freshness_and_unknown_filters() {
    let repo = Repository::new("The evidence is readable.");
    let host = host(&[("selected", &repo)], &["selected"]);
    for operation in ["verification-runs", "verification-bindings"] {
        for body in [
            json!({"context":{"repository":"selected","scope":"default","freshness":"catch_up"},"request":{}}),
            json!({"context":{"repository":"selected","scope":"default"},"request":{"limit":1}}),
        ] {
            let (status, answer) = call(&host, operation, body).await;
            assert_eq!(status, 400, "{operation}: {answer}");
        }
    }
    host.shutdown().await;
}

#[tokio::test]
async fn git_required_reads_return_typed_missing_capability_and_revision_failures() {
    let repo = Repository::new("The evidence is readable.");
    let host = host(&[("selected", &repo)], &["selected"]);
    let body = |base: &str| json!({"context":{"repository":"selected","scope":"default"},"request":{"rule":"rule_shared","base":base}});
    let (status, answer) = call(&host, "evidence", body("HEAD")).await;
    assert_eq!(status, 503, "{answer}");
    assert_eq!(answer["error"]["kind"], "git_unavailable");
    for args in [
        &["init", "-q"][..],
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "--allow-empty",
            "-qm",
            "Initial",
        ],
    ] {
        assert!(std::process::Command::new("git")
            .args(args)
            .current_dir(repo.dir.path())
            .status()
            .unwrap()
            .success());
    }
    let (status, answer) = call(&host, "evidence", body("HEAD")).await;
    assert_eq!(status, 200, "{answer}");
    assert!(answer["stale"].is_object());
    assert_eq!(
        answer["stamp"]["live"],
        json!(["canonical", "diff", "verification_runs"])
    );
    for revision in ["missing-revision", "--output=/tmp/never-write"] {
        let (status, answer) = call(&host, "evidence", body(revision)).await;
        assert_eq!(status, 409, "{answer}");
        assert_eq!(answer["error"]["kind"], "git_revision_not_found");
        assert!(!answer
            .to_string()
            .contains(repo.dir.path().to_str().unwrap()));
    }
    host.shutdown().await;
}

#[tokio::test]
async fn all_four_evidence_cuts_and_scope_isolation_remain_independent() {
    let repo = Repository::new("The evidence is readable.");
    let base = repo.evidence();
    repo.add_scope("other", "The other scope has no evidence.");
    let host = host(&[("selected", &repo)], &["selected"]);
    for scope in ["default", "other"] {
        let (status, answer) = call(&host, "evidence", json!({"context":{"repository":"selected","scope":scope},"request":{"rule":"rule_shared","limit":1,"base":base}})).await;
        assert_eq!(status, 200, "{answer}");
        for field in [
            "implementation_bindings",
            "verification_bindings",
            "verification_runs",
            "reviews",
        ] {
            assert_eq!(
                answer[field].as_array().unwrap().len(),
                usize::from(scope == "default")
            );
            assert_eq!(answer[format!("{field}_has_more")], scope == "default");
        }
        assert_eq!(answer["has_more"], scope == "default");
        if scope == "default" {
            assert_eq!(answer["latest_verification_run"]["id"], "run_fixture_1");
        }
    }
    host.shutdown().await;
}

#[tokio::test]
async fn historically_unbounded_lists_return_every_row() {
    let repo = Repository::new("The evidence is readable.");
    repo.evidence();
    let scope = provenance_core::ScopeId::new("default").unwrap();
    for path in [
        repo.layout.verification_runs_path(&scope),
        provenance_store::shards::verification_bindings_path(&repo.layout, &scope),
    ] {
        let template: serde_json::Value = serde_json::from_str(
            std::fs::read_to_string(&path)
                .unwrap()
                .lines()
                .next()
                .unwrap(),
        )
        .unwrap();
        let rows = (0..205)
            .map(|index| {
                let mut row = template.clone();
                row["id"] = json!(format!("row_{index}"));
                row.to_string()
            })
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(path, rows + "\n").unwrap();
    }
    let host = host(&[("selected", &repo)], &["selected"]);
    for operation in ["verification-runs", "verification-bindings"] {
        let (status, answer) = call(
            &host,
            operation,
            json!({"context":{"repository":"selected","scope":"default"},"request":{}}),
        )
        .await;
        assert_eq!(status, 200, "{answer}");
        assert_eq!(answer.as_array().unwrap().len(), 205);
    }
    host.shutdown().await;
}
