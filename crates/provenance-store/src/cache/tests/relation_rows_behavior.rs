//! The derived relation table: one row per declared reference of every
//! owner record, rebuilt with the owner's family.

use super::catch_up_behavior::assert_catch_up_equals_rebuild;
use super::fixtures::*;
use crate::cache::{catch_up_state, materialize_state, open_cache};
use crate::layout::ProvenanceLayout;

type RelationRow = (String, String, String, String, String);

async fn relation_rows(layout: &ProvenanceLayout) -> Vec<RelationRow> {
    let pool = open_cache(layout).await.unwrap();
    let rows = sqlx::query_as(
        "SELECT owner_type, owner_id, relation, target_type, target_id FROM relations \
         ORDER BY owner_type, owner_id, relation, target_id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    pool.close().await;
    rows
}

fn row(owner: (&str, &str), relation: &str, target: (&str, &str)) -> RelationRow {
    (
        owner.0.to_string(),
        owner.1.to_string(),
        relation.to_string(),
        target.0.to_string(),
        target.1.to_string(),
    )
}

#[tokio::test]
async fn materialize_derives_one_row_per_declared_reference() {
    let (_dir, layout, _scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();

    assert_eq!(
        relation_rows(&layout).await,
        vec![
            row(
                ("requirement", "req_schads_overtime"),
                "cites",
                ("source", "source_schads")
            ),
            row(
                ("requirement", "req_schads_overtime"),
                "domain_id",
                ("domain", "domain_payroll")
            ),
            row(
                ("resolution", "res_schads_overtime"),
                "requirement_ids",
                ("requirement", "req_schads_overtime")
            ),
            row(
                ("rule", "rule_schads_pay_001"),
                "requirement_ids",
                ("requirement", "req_schads_overtime")
            ),
            row(
                ("rule", "rule_schads_pay_001"),
                "resolution_ids",
                ("resolution", "res_schads_overtime")
            ),
        ]
    );
}

/// The loader hand-lists the owner kinds it derives rows from, and lists
/// the links of a topic and of a question apart from their declared
/// fields. A kind's `links` rows would satisfy a check that only asks for
/// some row of that kind, so one declared row is pinned per kind and one
/// `links` row per owner that carries links. Deleting any branch of the
/// loader turns this red instead of leaving the table short.
#[tokio::test]
async fn every_owner_kind_and_the_links_derive_relation_rows() {
    let (_dir, layout, _scope) = owner_row_layout();
    materialize_state(&layout).await.unwrap();
    let rows = relation_rows(&layout).await;
    for owner in [
        "source",
        "requirement",
        "resolution",
        "rule",
        "topic",
        "question",
        "boundary",
    ] {
        assert!(
            rows.iter().any(|row| row.0 == owner),
            "no relation rows derive from the {owner} records: {rows:?}"
        );
    }
    assert!(
        rows.iter().any(|row| row.2 == "links"),
        "no rows derive from the topic and question links: {rows:?}"
    );
    let declared = [
        row(
            ("source", "source_schads"),
            "supersedes",
            ("source", "source_award_2019"),
        ),
        row(
            ("requirement", "req_schads_overtime"),
            "cites",
            ("source", "source_schads"),
        ),
        row(
            ("resolution", "res_schads_overtime"),
            "requirement_ids",
            ("requirement", "req_schads_overtime"),
        ),
        row(
            ("rule", "rule_schads_pay_001"),
            "requirement_ids",
            ("requirement", "req_schads_overtime"),
        ),
        row(
            ("topic", "topic_rates"),
            "requirement_id",
            ("requirement", "req_schads_overtime"),
        ),
        row(
            ("question", "question_threshold"),
            "topic_id",
            ("topic", "topic_rates"),
        ),
        row(
            ("boundary", "boundary_no_backpay"),
            "requirement_id",
            ("requirement", "req_schads_overtime"),
        ),
    ];
    for expected in &declared {
        assert!(
            rows.contains(expected),
            "the declared {} row of the {} records is missing: {rows:?}",
            expected.2,
            expected.0
        );
    }
    let linked = [
        row(
            ("topic", "topic_rates"),
            "links",
            ("requirement", "req_schads_overtime"),
        ),
        row(
            ("question", "question_threshold"),
            "links",
            ("rule", "rule_schads_pay_001"),
        ),
    ];
    for expected in &linked {
        assert!(
            rows.contains(expected),
            "the links row of the {} records is missing: {rows:?}",
            expected.0
        );
    }
}

#[tokio::test]
async fn catch_up_rebuilds_the_rows_of_a_scope_whose_owner_family_moved() {
    let (_dir, layout, scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();

    let rules = crate::shards::rules_path(&layout, &scope);
    let edited = std::fs::read_to_string(&rules).unwrap().replace(
        r#""resolution_ids":["res_schads_overtime"]"#,
        r#""resolution_ids":[]"#,
    );
    assert_ne!(edited, std::fs::read_to_string(&rules).unwrap());
    std::fs::write(&rules, edited).unwrap();

    let report = catch_up_state(&layout).await.unwrap();
    assert_eq!(report.families_rederived, 1, "{report:?}");
    assert!(!relation_rows(&layout)
        .await
        .iter()
        .any(|row| row.2 == "resolution_ids"));
    assert_catch_up_equals_rebuild(&layout).await;
}

#[tokio::test]
async fn a_departed_scope_takes_its_rows_with_it() {
    let (_dir, layout, _scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();
    assert!(!relation_rows(&layout).await.is_empty());

    let mut manifest: provenance_core::Manifest =
        serde_json::from_slice(&std::fs::read(layout.manifest_path()).unwrap()).unwrap();
    manifest.scopes.clear();
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();

    catch_up_state(&layout).await.unwrap();
    assert!(relation_rows(&layout).await.is_empty());
    assert_catch_up_equals_rebuild(&layout).await;
}
