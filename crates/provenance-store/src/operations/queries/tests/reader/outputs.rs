//! What the catch-up, the rebuild and the schema check give over the file
//! under each freshness policy: the reports, the stamps and the refusal
//! text. No Rule: the expected lines in `outputs.txt` hold the behaviour
//! while the catch-up and the schema check move onto the read database
//! port. On a difference the test prints every line it read.

use super::super::comparison::test_stores::{self, TestStore};
use super::get_through;
use crate::cache::{catch_up_state, materialize_state, open_cache, CatchUpReport};
use crate::operations::read_policy::{FreshnessPolicy, ReadPolicy};
use provenance_core::RequirementStatus;

const EXPECTED: &str = include_str!("outputs.txt");

/// The policies in the order the test reads: catch-up last, as it
/// repairs the database.
const POLICIES: [FreshnessPolicy; 3] = [
    FreshnessPolicy::AnnotateOnly,
    FreshnessPolicy::RefuseStale,
    FreshnessPolicy::CatchUp,
];

/// The text with the parts that change from run to run replaced: the
/// root of the store, the instance id, and each run of 32 or more hex
/// digits (a digest).
fn redact(store: &TestStore, instance: Option<&str>, text: &str) -> String {
    let mut text = text
        .replace(store.root.as_str(), "<root>")
        .replace('\\', "/");
    if let Some(instance) = instance {
        text = text.replace(instance, "<instance>");
    }
    let mut out = String::with_capacity(text.len());
    let mut run = String::new();
    for character in text.chars() {
        if character.is_ascii_hexdigit() {
            run.push(character);
            continue;
        }
        out.push_str(if run.len() >= 32 { "<hex>" } else { &run });
        run.clear();
        out.push(character);
    }
    out.push_str(if run.len() >= 32 { "<hex>" } else { &run });
    out
}

async fn instance(store: &TestStore) -> Option<String> {
    if !store.layout().cache_db_path().exists() {
        return None;
    }
    let cache = open_cache(&store.layout()).await.unwrap();
    let found = sqlx::query_scalar("SELECT instance_id FROM projection_instance")
        .fetch_optional(cache.pool())
        .await
        .ok()
        .flatten();
    cache.close().await.unwrap();
    found
}

/// One line for a catch-up or rebuild report, without its digest.
fn report_line(label: &str, report: anyhow::Result<CatchUpReport>) -> String {
    match report {
        Ok(report) => format!(
            "{label}: serial {} rebuilt {} committed {} units {} families {} rows {} recreated {}",
            report.serial,
            report.rebuilt,
            report.revision_committed,
            report.units_hashed,
            report.families_rederived,
            report.rows_written,
            report.cache_recreated
        ),
        Err(error) => format!("{label}: refused: {error:#}"),
    }
}

/// One line for each policy: the stamp policy, the serial and the
/// freshness error of the answer, or the refusal.
async fn reads(store: &TestStore, label: &str, lines: &mut Vec<String>) {
    for policy in POLICIES {
        let answer = get_through(store, ReadPolicy::with_freshness(policy)).await;
        let instance = instance(store).await;
        let text = match answer {
            Ok(stamped) => format!(
                "{:?} serial {} found {} error {:?}",
                stamped.stamp.policy,
                stamped.stamp.serial,
                stamped.result.found,
                stamped.freshness_error
            ),
            Err(error) => format!("refused: {error:#}"),
        };
        lines.push(redact(
            store,
            instance.as_deref(),
            &format!("{label} {policy:?}: {text}"),
        ));
    }
}

async fn catch_up_line(store: &TestStore, label: &str, lines: &mut Vec<String>) {
    let report = catch_up_state(&store.repository()).await;
    let instance = instance(store).await;
    lines.push(redact(
        store,
        instance.as_deref(),
        &report_line(label, report),
    ));
}

async fn change_cache(store: &TestStore, statements: &[&str]) {
    let cache = open_cache(&store.layout()).await.unwrap();
    for statement in statements {
        sqlx::query(sqlx::AssertSqlSafe(*statement))
            .execute(cache.pool())
            .await
            .unwrap();
    }
    cache.close().await.unwrap();
}

fn add_requirement(store: &TestStore, id: &str) {
    crate::cache::tests::fixtures::create_requirement(
        &store.state_store(),
        &store.scope,
        id,
        RequirementStatus::Active,
    );
}

async fn database_states(lines: &mut Vec<String>) {
    let store = test_stores::seeded_queries();
    reads(&store, "absent", lines).await;
    catch_up_line(&store, "catch-up absent", lines).await;
    catch_up_line(&store, "catch-up current", lines).await;
    reads(&store, "current", lines).await;
    add_requirement(&store, "req_after_the_pass");
    reads(&store, "moved", lines).await;
    catch_up_line(&store, "catch-up moved", lines).await;
    for (label, statements) in [
        (
            "digest",
            &["UPDATE _cache_metadata SET schema_digest = 'incompatible'"][..],
        ),
        ("index", &["DROP INDEX idx_relations_in"][..]),
        ("table", &["DROP TABLE relations"][..]),
        ("metadata", &["DROP TABLE _cache_metadata"][..]),
        (
            "metadata column",
            &[
                "DROP TABLE _cache_metadata",
                "CREATE TABLE _cache_metadata (only_row INTEGER)",
            ][..],
        ),
        (
            "validation",
            &["INSERT OR REPLACE INTO projection_validation VALUES (1, 0)"][..],
        ),
        ("no validation", &["DELETE FROM projection_validation"][..]),
        ("half", &["DELETE FROM projection_family_digests"][..]),
        ("no revision", &["DELETE FROM projection_revision"][..]),
    ] {
        change_cache(&store, statements).await;
        reads(&store, label, lines).await;
        change_cache(&store, statements).await;
        catch_up_line(&store, &format!("catch-up {label}"), lines).await;
    }
    let rebuilt = materialize_state(&store.repository()).await.map(|report| {
        format!(
            "records {} recreated {}",
            report.records_loaded, report.cache_recreated
        )
    });
    lines.push(format!("materialize: {rebuilt:?}"));
    change_cache(&store, &["DROP TABLE relations"]).await;
    let rebuilt = materialize_state(&store.repository()).await.map(|report| {
        format!(
            "records {} recreated {}",
            report.records_loaded, report.cache_recreated
        )
    });
    lines.push(format!("materialize dropped table: {rebuilt:?}"));
    reads(&store, "materialized", lines).await;
}

async fn refused_pass(lines: &mut Vec<String>) {
    let store = test_stores::seeded_queries();
    add_requirement(&store, "req_rule_anchor");
    crate::cache::tests::fixtures::create_rule_of(
        &store.state_store(),
        &store.scope,
        "rule_anchored",
        "req_rule_anchor",
    );
    catch_up_line(&store, "catch-up anchored", lines).await;
    crate::cache::tests::fixtures::rewrite_records(
        &crate::shards::rules_path(&store.layout(), &store.scope),
        |record| record["requirement_ids"] = serde_json::json!([]),
    );
    reads(&store, "refused", lines).await;
    catch_up_line(&store, "catch-up refused", lines).await;
    let rebuilt = materialize_state(&store.repository())
        .await
        .map(|report| report.records_loaded);
    let instance = instance(&store).await;
    lines.push(redact(
        &store,
        instance.as_deref(),
        &format!("materialize refused: {rebuilt:?}"),
    ));
    std::fs::remove_file(store.layout().cache_db_path()).unwrap();
    reads(&store, "refused absent", lines).await;
}

fn compare(lines: &[String]) {
    let actual = lines.join("\n") + "\n";
    if actual != EXPECTED {
        println!("=== FRESHNESS OUTPUTS\n{actual}=== END FRESHNESS OUTPUTS");
        for (index, (left, right)) in actual.lines().zip(EXPECTED.lines()).enumerate() {
            assert_eq!(left, right, "line {}", index + 1);
        }
        assert_eq!(actual.lines().count(), EXPECTED.lines().count());
    }
}

#[tokio::test]
async fn each_policy_gives_the_same_reports_stamps_and_refusals() {
    let mut lines = Vec::new();
    database_states(&mut lines).await;
    refused_pass(&mut lines).await;
    compare(&lines);
}
