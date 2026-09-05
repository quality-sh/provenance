//! `impact` follows each declared relation in its flow direction and
//! never adds a step; its file scan stops at the configured count and the
//! answer says so.

use super::comparison::test_stores::{self, TestStore};
use crate::operations::queries::{self, impact};
use crate::operations::read_policy::{FreshnessPolicy, ReadPolicy};
use crate::operations::reader;
use provenance_core::protocol::{ImpactQuery, ImpactResult, Stamped, SDK_PROTOCOL_VERSION};
use provenance_macros::verifies;

fn query(id: &str) -> ImpactQuery {
    ImpactQuery {
        protocol_version: Some(SDK_PROTOCOL_VERSION),
        id: id.into(),
        node_type: None,
        include_retired: false,
        limit: 50,
    }
}

/// `res_overtime` is named by `rule_overtime_001` alone. The requirement
/// it answers, `req_overtime`, has eight more rules, and none of them is
/// reached: the resolution's requirement list points upstream.
#[tokio::test]
#[verifies("rule_impact_follows_declared_flow", examples)]
async fn a_resolution_reaches_the_rules_that_name_it_only() {
    let store = TestStore::pinned();
    let answer = queries::impact(
        Some(store.root.clone()),
        &store.scope,
        query("res_overtime"),
    )
    .await
    .unwrap();
    let rules: Vec<&str> = answer
        .result
        .affected_rules
        .iter()
        .map(|rule| rule.id.as_str())
        .collect();
    assert_eq!(rules, ["rule_overtime_001"]);
    assert!(!answer.result.has_more);
    assert_eq!(
        answer.stamp.attested,
        [
            "implementation_bindings",
            "relations",
            "requirements",
            "resolutions",
            "rules",
            "sources",
            "verification_bindings"
        ],
        "the kind probe reads sources and requirements before it finds the resolution"
    );
    assert_eq!(answer.stamp.live, ["scanned_sites"]);
}

/// One `impact` through the reader under a scan limit, over a store that
/// is already caught up.
async fn impact_under(
    store: &TestStore,
    scan_limit: usize,
) -> anyhow::Result<Stamped<ImpactResult>> {
    let policy = ReadPolicy {
        freshness: FreshnessPolicy::AnnotateOnly,
        scan_limit,
    };
    reader::answer(&store.root, &store.scope, policy, move |ctx| {
        Box::pin(async move { impact::impact(ctx, query("req_overtime")).await })
    })
    .await
}

/// The seeded store's working tree holds one source file with a rule
/// site. A limit the tree fits under reads it; a limit of zero cuts the
/// scan before it, and the answer says so.
#[tokio::test]
#[verifies("rule_impact_reports_a_cut_scan", examples)]
async fn impact_says_when_the_scan_was_cut() {
    let store = test_stores::seeded_queries();
    crate::cache::tests::fixtures::create_rule_of(
        &store.state_store(),
        &store.scope,
        "rule_overtime",
        "req_overtime",
    );
    crate::cache::catch_up_state(&store.layout()).await.unwrap();
    crate::test_probes::set_test_scan(None);

    let whole = impact_under(&store, usize::MAX).await.unwrap();
    assert!(!whole.result.scan_cut);
    let sites: Vec<String> = whole.result.affected_rules[0]
        .implementations
        .iter()
        .map(|site| site.file.to_string())
        .collect();
    assert_eq!(sites, ["src/pay.rs"]);

    let cut = impact_under(&store, 0).await.unwrap();
    assert!(cut.result.scan_cut, "a limit of zero cuts before the file");
    assert!(cut.result.affected_rules[0].implementations.is_empty());
    assert_eq!(cut.result.affected_rules[0].id.as_str(), "rule_overtime");
    assert_eq!(cut.stamp.live, ["scanned_sites"]);
    assert_eq!(cut.stamp.serial, whole.stamp.serial);
}
