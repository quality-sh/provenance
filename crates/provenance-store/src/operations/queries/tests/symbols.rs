//! `resolve_symbol` scans the named file alone, so it never meets the
//! scan limit and cannot miss the file; a file the scanner cannot read
//! still answers its bindings.

use super::comparison::test_stores::{self, TestStore};
use crate::cache::tests::fixtures::{append_record, create_rule_of};
use crate::operations::queries::{self, impact, symbols};
use crate::operations::read_policy::{FreshnessPolicy, ReadPolicy};
use crate::operations::reader;
use crate::shards;
use provenance_core::protocol::{ImpactQuery, ResolveSymbolQuery, SDK_PROTOCOL_VERSION};
use provenance_core::SUPPORTED_SCHEMA_VERSION;
use provenance_macros::verifies;
use serde_json::json;

fn resolve(file: &str, line: Option<usize>) -> ResolveSymbolQuery {
    ResolveSymbolQuery {
        protocol_version: Some(SDK_PROTOCOL_VERSION),
        file: file.into(),
        symbol: None,
        line,
        include_retired: false,
        limit: 50,
    }
}

/// The seeded store with a rule record behind the source file's site.
fn store_with_rule() -> TestStore {
    let store = test_stores::seeded_queries();
    create_rule_of(
        &store.state_store(),
        &store.scope,
        "rule_overtime",
        "req_overtime",
    );
    store
}

fn bind(store: &TestStore, id: &str, file: &str) {
    append_record(
        &shards::implementation_bindings_path(&store.layout(), &store.scope),
        &json!({
            "schema_version": SUPPORTED_SCHEMA_VERSION.0,
            "scope_id": store.scope.as_str(),
            "id": id,
            "rule_id": "rule_overtime",
            "declared_by": "spec://test",
            "retired": false,
            "file": file,
            "symbol": "pay",
        }),
    );
}

fn rule_ids(rules: &[provenance_core::protocol::GraphNode]) -> Vec<&str> {
    rules.iter().map(|node| node.id().as_str()).collect()
}

/// Under a scan limit of zero the tree scan reads nothing and `impact`
/// says so; `resolve_symbol` on the same file still answers the site,
/// so it did not go through the tree scan.
#[tokio::test]
#[verifies("rule_resolve_symbol_reads_the_named_file_only", examples)]
async fn resolve_symbol_reads_the_named_file_only() {
    let store = store_with_rule();
    crate::cache::catch_up_state(&store.layout()).await.unwrap();
    crate::test_probes::set_test_scan(None);
    let policy = ReadPolicy {
        freshness: FreshnessPolicy::AnnotateOnly,
        scan_limit: 0,
    };
    let cut = reader::answer(&store.root, &store.scope, policy, |ctx| {
        Box::pin(async move {
            impact::impact(
                ctx,
                ImpactQuery {
                    protocol_version: Some(SDK_PROTOCOL_VERSION),
                    id: "req_overtime".into(),
                    node_type: None,
                    include_retired: false,
                    limit: 50,
                },
            )
            .await
        })
    })
    .await
    .unwrap();
    assert!(cut.result.scan_cut);

    let answer = reader::answer(&store.root, &store.scope, policy, |ctx| {
        Box::pin(async move { symbols::resolve(ctx, resolve("src/pay.rs", Some(1))).await })
    })
    .await
    .unwrap();
    assert_eq!(rule_ids(&answer.result.rules), ["rule_overtime"]);
    assert_eq!(answer.stamp.attested, ["rules"]);
    assert_eq!(answer.stamp.live, ["scanned_sites"]);
}

#[tokio::test]
#[verifies("rule_resolve_symbol_reads_the_named_file_only", examples)]
async fn resolve_symbol_on_an_unscanned_extension_answers_bindings_only() {
    let store = store_with_rule();
    bind(&store, "bind_md", "docs/pay.md");
    let note = store.root.join("docs/pay.md");
    std::fs::create_dir_all(note.parent().unwrap()).unwrap();
    std::fs::write(note, "#[rule(\"rule_from_prose\")]\nfn pay() {}\n").unwrap();
    let answer = queries::resolve_symbol(
        Some(store.root.clone()),
        &store.scope,
        ReadPolicy::default(),
        resolve("docs/pay.md", None),
    )
    .await
    .unwrap();
    assert_eq!(
        rule_ids(&answer.result.rules),
        ["rule_overtime"],
        "the binding answers; the prose is not scanned"
    );
    assert_eq!(
        answer.stamp.attested,
        ["implementation_bindings", "rules", "verification_bindings"]
    );
    assert_eq!(answer.stamp.live, ["scanned_sites"]);
}

#[tokio::test]
#[verifies("rule_resolve_symbol_reads_the_named_file_only", examples)]
async fn resolve_symbol_on_a_missing_file_answers_bindings_only() {
    let store = store_with_rule();
    bind(&store, "bind_gone", "src/gone.rs");
    let answer = queries::resolve_symbol(
        Some(store.root.clone()),
        &store.scope,
        ReadPolicy::default(),
        resolve("src/gone.rs", None),
    )
    .await
    .unwrap();
    assert_eq!(rule_ids(&answer.result.rules), ["rule_overtime"]);
    assert!(!answer.result.has_more);
}

/// Invalid source paths refuse before bindings can form a successful answer.
#[tokio::test]
#[verifies("rule_resolve_symbol_reads_the_named_file_only", examples)]
async fn resolve_symbol_scans_only_a_repository_relative_path() {
    let store = store_with_rule();
    bind(&store, "bind_dotted", "./src/pay.rs");
    // A sibling directory beside the store, gone with the test.
    let beside = tempfile::tempdir_in(store.root.parent().unwrap()).unwrap();
    std::fs::write(
        beside.path().join("outside_pay.rs"),
        "#[rule(\"rule_overtime\")]\nfn outside() {}\n",
    )
    .unwrap();
    let climbing = format!(
        "../{}/outside_pay.rs",
        beside.path().file_name().unwrap().to_str().unwrap()
    );
    let absolute = store.root.join("src/pay.rs");
    for file in [
        "./src/pay.rs",
        absolute.as_str(),
        climbing.as_str(),
        "src/../src/pay.rs",
    ] {
        for line in [None, Some(1)] {
            let Err(error) = queries::resolve_symbol(
                Some(store.root.clone()),
                &store.scope,
                ReadPolicy::default(),
                resolve(file, line),
            )
            .await
            else {
                panic!("unsafe source paths refuse");
            };
            assert!(matches!(
                error.downcast_ref::<crate::operations::files::FileAccessRefusal>(),
                Some(crate::operations::files::FileAccessRefusal::Denied)
            ));
        }
    }
}
