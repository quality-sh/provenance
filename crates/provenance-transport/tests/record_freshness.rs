#![cfg(feature = "test-fixture")]
#[path = "support/records.rs"]
mod records;
use records::{call, get_call, host, Repository};
use serde_json::json;

#[tokio::test]
async fn annotate_only_refuses_no_projection_without_creating_storage() {
    let repo = Repository::new("The shared graph is readable.");
    let before = repo.bytes();
    let host = host(&[("first", &repo)], &["first"]);
    let mut request = get_call("first", "default");
    request["context"]["freshness"] = json!("annotate_only");
    let (status, failure) = call(&host, "get", request).await;
    assert_eq!(status, 409, "{failure}");
    assert_eq!(failure["error"], json!({"kind":"no_projection"}));
    assert_eq!(repo.bytes(), before);
    host.shutdown().await;
}

#[tokio::test]
async fn failed_catch_up_has_a_safe_typed_cause_and_the_actual_stored_stamp() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&[("first", &repo)], &["first"]);
    let (_, before) = call(&host, "get", get_call("first", "default")).await;
    let locks = repo.layout.cache_dir().join("locks");
    std::fs::remove_dir_all(&locks).unwrap();
    std::fs::write(&locks, b"blocked").unwrap();
    let (status, answer) = call(&host, "get", get_call("first", "default")).await;
    assert_eq!(status, 200, "{answer}");
    assert_eq!(answer["stamp"]["policy"], "catch_up_failed");
    assert_eq!(answer["freshness_cause"], "catch_up_failed");
    for field in ["serial", "digest", "instance_id"] {
        assert_eq!(answer["stamp"][field], before["stamp"][field]);
    }
    assert!(!answer
        .to_string()
        .contains(repo.dir.path().to_str().unwrap()));
    assert_eq!(
        answer["freshness_error"],
        "catch-up failed; answer uses the stored projection"
    );
    host.shutdown().await;
}

#[tokio::test]
async fn overlapping_scopes_and_annotate_only_keep_the_selected_value() {
    let repo = Repository::new("The default shared graph is readable.");
    repo.add_scope("other", "The other shared graph is readable.");
    let host = host(&[("first", &repo)], &["first"]);
    let (_, original) = call(&host, "get", get_call("first", "default")).await;
    repo.edit("default", "The saved shared graph is readable.");
    let mut request = get_call("first", "default");
    request["context"]["freshness"] = json!("annotate_only");
    let (_, stored) = call(&host, "get", request).await;
    assert_eq!(stored["node"], original["node"]);
    assert_eq!(stored["stamp"]["policy"], "annotate_only");
    let (_, other) = call(&host, "get", get_call("first", "other")).await;
    assert_eq!(
        other["node"]["statement"],
        "The other shared graph is readable."
    );
    let (_, edited) = call(&host, "get", get_call("first", "default")).await;
    assert_eq!(
        edited["node"]["statement"],
        "The saved shared graph is readable."
    );
    host.shutdown().await;
}

#[tokio::test]
async fn schema_and_partial_migration_refusals_have_distinct_safe_kinds() {
    use sqlx::Connection;
    for (sql, expected) in [
        (
            "UPDATE projection_validation SET version = 0",
            "schema_behind",
        ),
        ("DELETE FROM projection_family_digests", "half_migrated"),
    ] {
        let repo = Repository::new("The shared graph is readable.");
        let host = host(&[("first", &repo)], &["first"]);
        assert_eq!(
            call(&host, "get", get_call("first", "default")).await.0,
            200
        );
        let options =
            sqlx::sqlite::SqliteConnectOptions::new().filename(repo.layout.cache_db_path());
        let mut connection = sqlx::SqliteConnection::connect_with(&options)
            .await
            .unwrap();
        sqlx::query(sql).execute(&mut connection).await.unwrap();
        connection.close().await.unwrap();
        let mut request = get_call("first", "default");
        request["context"]["freshness"] = json!("annotate_only");
        let (status, refusal) = call(&host, "get", request).await;
        assert_eq!(status, 409, "{refusal}");
        assert_eq!(refusal["error"], json!({"kind":expected}));
        host.shutdown().await;
    }
}

#[cfg(unix)]
#[tokio::test]
async fn unreadable_unit_refuses_by_logical_name_without_a_host_path() {
    use std::os::unix::fs::PermissionsExt;
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&[("first", &repo)], &["first"]);
    assert_eq!(
        call(&host, "get", get_call("first", "default")).await.0,
        200
    );
    let rules = provenance_store::shards::rules_path(
        &repo.layout,
        &provenance_core::ScopeId::new("default").unwrap(),
    );
    let permissions = std::fs::metadata(&rules).unwrap().permissions();
    std::fs::set_permissions(&rules, std::fs::Permissions::from_mode(0o000)).unwrap();
    if std::fs::read(&rules).is_ok() {
        std::fs::set_permissions(&rules, permissions).unwrap();
        eprintln!("SKIP unreadable unit: this user bypasses Unix file permissions");
        return;
    }
    let mut request = get_call("first", "default");
    request["context"]["freshness"] = json!("refuse_stale");
    let (status, refusal) = call(&host, "get", request).await;
    std::fs::set_permissions(&rules, permissions).unwrap();
    assert_eq!(status, 409, "{refusal}");
    assert_eq!(
        refusal["error"],
        json!({"kind":"unit_unreadable","unit":"scope:default"})
    );
    host.shutdown().await;
}

#[tokio::test]
async fn stale_refusal_distinguishes_new_and_departed_units() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&[("first", &repo)], &["first"]);
    assert_eq!(
        call(&host, "get", get_call("first", "default")).await.0,
        200
    );
    repo.add_scope("other", "The other shared graph is readable.");
    let mut request = get_call("first", "default");
    request["context"]["freshness"] = json!("refuse_stale");
    let (_, added) = call(&host, "get", request.clone()).await;
    let unit = added["error"]["moved"]
        .as_array()
        .unwrap()
        .iter()
        .find(|unit| unit["unit"] == "scope:other")
        .unwrap();
    assert_eq!(unit["stored"], "");
    assert_ne!(unit["live"], "");
    assert_eq!(
        call(&host, "get", get_call("first", "default")).await.0,
        200
    );
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(repo.layout.manifest_path()).unwrap()).unwrap();
    manifest["scopes"]
        .as_array_mut()
        .unwrap()
        .retain(|scope| scope["id"] != "other");
    std::fs::write(
        repo.layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let (_, departed) = call(&host, "get", request).await;
    let unit = departed["error"]["moved"]
        .as_array()
        .unwrap()
        .iter()
        .find(|unit| unit["unit"] == "scope:other")
        .unwrap();
    assert_ne!(unit["stored"], "");
    assert_eq!(unit["live"], "");
    host.shutdown().await;
}
