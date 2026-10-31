//! `resolve_symbol` ranks symbol matches first, and its cursor gives the
//! rest of the ranked order, a Rule cut by the page budget included. A
//! changed file or graph, or a cursor from another query, refuses it.

use super::comparison::test_stores::TestStore;
use super::symbols::{resolve, resolve_named, rule_ids, store_with_rule};
use crate::cache::tests::fixtures::create_rule_of;
use crate::operations::queries;
use crate::operations::read_policy::ReadPolicy;
use provenance_core::protocol::read_failure::ReadFailure;
use provenance_core::protocol::{
    QueryResponse, ResolveSymbolQuery, ResolveSymbolResult, SearchQuery, Stamped,
    QUERY_RESPONSE_BYTES,
};
use provenance_macros::verifies;
use serde_json::json;
use std::fmt::Write as _;

/// Every page of the answer, from the first page to the page with no
/// cursor. Each page that has more gives a cursor.
async fn pages(
    store: &TestStore,
    mut request: ResolveSymbolQuery,
) -> Vec<Stamped<ResolveSymbolResult>> {
    let mut pages = Vec::new();
    loop {
        let page =
            queries::resolve_symbol(&store.repository(), ReadPolicy::default(), request.clone())
                .await
                .unwrap();
        let next = page.result.next_cursor.clone();
        assert_eq!(page.result.has_more, next.is_some());
        pages.push(page);
        match next {
            Some(cursor) => request.cursor = Some(cursor),
            None => return pages,
        }
    }
}

#[tokio::test]
#[verifies("rule_resolve_symbol_ranks_symbol_matches_first", examples)]
#[verifies("rule_resolve_symbol_cursor_continues_the_page", examples)]
async fn a_symbol_match_ranks_before_file_matches_on_every_page() {
    let store = store_with_rule();
    for id in ["rule_audit", "rule_bonus"] {
        create_rule_of(&store.state_store(), &store.scope, id, "req_overtime");
    }
    std::fs::write(
        store.root.join("src/pay.rs"),
        concat!(
            "#[rule(\"rule_audit\")]\n",
            "fn audit() {}\n",
            "#[rule(\"rule_overtime\")]\n",
            "fn pay() {}\n",
            "#[verifies(\"rule_bonus\", examples)]\n",
            "fn checks_pay() {}\n",
        ),
    )
    .unwrap();

    let pages = pages(
        &store,
        ResolveSymbolQuery {
            limit: 1,
            ..resolve_named("src/pay.rs", "pay")
        },
    )
    .await;

    let rules = pages
        .iter()
        .map(|page| rule_ids(&page.result.rules))
        .collect::<Vec<_>>();
    assert_eq!(
        rules,
        [["rule_overtime"], ["rule_audit"], ["rule_bonus"]],
        "the symbol match comes first, then the file matches by id"
    );
    let matches = pages
        .iter()
        .flat_map(|page| page.result.matches.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        serde_json::to_value(matches).unwrap(),
        json!([
            {
                "rule_id": "rule_overtime",
                "role": "implementation",
                "line": 3,
                "item_name": "pay",
                "match_kind": "symbol"
            },
            {
                "rule_id": "rule_audit",
                "role": "implementation",
                "line": 1,
                "item_name": "audit",
                "match_kind": "file"
            },
            {
                "rule_id": "rule_bonus",
                "role": "verification",
                "line": 5,
                "item_name": "checks_pay",
                "verification_method": "examples",
                "match_kind": "file"
            }
        ])
    );
}

#[tokio::test]
#[verifies("rule_resolve_symbol_cursor_continues_the_page", examples)]
async fn a_rule_cut_by_the_page_budget_continues_on_the_next_page() {
    let store = store_with_rule();
    let name = "x".repeat(400);
    let mut source = String::new();
    for index in 0..2_500 {
        writeln!(
            source,
            "#[verifies(\"rule_overtime\", examples)]\nfn checks_{index}_{name}() {{}}"
        )
        .unwrap();
    }
    std::fs::write(store.root.join("src/many.rs"), source).unwrap();

    let pages = pages(
        &store,
        ResolveSymbolQuery {
            limit: 1,
            ..resolve("src/many.rs", None)
        },
    )
    .await;

    assert!(pages.len() > 1, "2,500 sites do not fit on one page");
    assert_eq!(rule_ids(&pages[0].result.rules), ["rule_overtime"]);
    for page in &pages {
        let envelope = QueryResponse::new(
            "resolve-symbol",
            Stamped {
                result: &page.result,
                stamp: page.stamp.clone(),
                freshness_error: page.freshness_error.clone(),
            },
        );
        assert!(serde_json::to_vec(&envelope).unwrap().len() <= QUERY_RESPONSE_BYTES);
    }
    for page in &pages[1..] {
        assert!(
            page.result.rules.is_empty(),
            "the Rule record went out on the first page"
        );
    }
    let sites = pages
        .iter()
        .flat_map(|page| page.result.matches.iter())
        .map(|site| (site.line, site.item_name.clone()))
        .collect::<Vec<_>>();
    let expected = (0..2_500)
        .map(|index| (Some(2 * index + 1), Some(format!("checks_{index}_{name}"))))
        .collect::<Vec<_>>();
    assert_eq!(
        sites, expected,
        "each page holds the next sites in order, and each site comes once"
    );
}

const TWO_RULES: &str = concat!(
    "#[rule(\"rule_audit\")]\n",
    "fn audit() {}\n",
    "#[rule(\"rule_overtime\")]\n",
    "fn pay() {}\n",
);

/// A store with two Rules bound in `src/pay.rs`, so a page of one Rule
/// gives a cursor.
fn store_with_two_rules() -> TestStore {
    let store = store_with_rule();
    create_rule_of(
        &store.state_store(),
        &store.scope,
        "rule_audit",
        "req_overtime",
    );
    std::fs::write(store.root.join("src/pay.rs"), TWO_RULES).unwrap();
    store
}

async fn page(
    store: &TestStore,
    file: &str,
    cursor: Option<String>,
) -> anyhow::Result<Stamped<ResolveSymbolResult>> {
    queries::resolve_symbol(
        &store.repository(),
        ReadPolicy::default(),
        ResolveSymbolQuery {
            limit: 1,
            cursor,
            ..resolve(file, None)
        },
    )
    .await
}

async fn first_cursor(store: &TestStore) -> String {
    page(store, "src/pay.rs", None)
        .await
        .unwrap()
        .result
        .next_cursor
        .expect("two Rules do not fit on a page of one")
}

fn refusal(result: anyhow::Result<Stamped<ResolveSymbolResult>>) -> ReadFailure {
    result
        .unwrap_err()
        .downcast::<ReadFailure>()
        .expect("a read refusal")
}

#[tokio::test]
#[verifies("rule_resolve_symbol_cursor_continues_the_page", examples)]
async fn a_changed_file_refuses_the_cursor() {
    let store = store_with_two_rules();
    let cursor = first_cursor(&store).await;
    std::fs::write(
        store.root.join("src/pay.rs"),
        format!("{TWO_RULES}#[verifies(\"rule_overtime\", examples)]\nfn checks_pay() {{}}\n"),
    )
    .unwrap();

    let refused = refusal(page(&store, "src/pay.rs", Some(cursor)).await);

    assert_eq!(refused, ReadFailure::CursorRevisionChanged);
}

#[tokio::test]
#[verifies("rule_resolve_symbol_cursor_continues_the_page", examples)]
async fn a_changed_graph_refuses_the_cursor() {
    let store = store_with_two_rules();
    let cursor = first_cursor(&store).await;
    create_rule_of(
        &store.state_store(),
        &store.scope,
        "rule_bonus",
        "req_overtime",
    );

    let refused = refusal(page(&store, "src/pay.rs", Some(cursor)).await);

    assert_eq!(refused, ReadFailure::CursorRevisionChanged);
}

#[tokio::test]
#[verifies("rule_resolve_symbol_cursor_continues_the_page", examples)]
async fn a_cursor_from_another_query_is_invalid() {
    let store = store_with_two_rules();
    let search = queries::search(
        &store.repository(),
        ReadPolicy::default(),
        serde_json::from_value::<SearchQuery>(json!({"text": "statement", "limit": 1})).unwrap(),
    )
    .await
    .unwrap()
    .result
    .next_cursor
    .expect("two Rule statements do not fit on a page of one");
    let other_file = first_cursor(&store).await;

    for (file, cursor) in [("src/pay.rs", search), ("src/other.rs", other_file)] {
        let refused = refusal(page(&store, file, Some(cursor)).await);

        assert_eq!(
            refused,
            ReadFailure::CursorInvalid,
            "a cursor sent to {file}"
        );
    }
}
