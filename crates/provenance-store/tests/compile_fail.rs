//! Compile-fail fixtures. Each case pins its exact diagnostic.

use provenance_macros::verifies;

#[test]
#[verifies("rule_store_under_guard_takes_no_second_lock", examples)]
#[verifies("rule_guarded_reads_use_guard_repository", examples)]
fn capability_refusals_hold_at_compile_time() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/forged_guard.rs");
    cases.compile_fail("tests/compile_fail/guarded_store_outlives_guard.rs");
    cases.compile_fail("tests/compile_fail/guarded_store_write.rs");
    cases.compile_fail("tests/compile_fail/guarded_store_wrong_repository.rs");
    cases.compile_fail("tests/compile_fail/read_after_stamp.rs");
}

#[test]
#[verifies("rule_staged_write_keeps_live_repository_identity", examples)]
#[verifies("rule_staged_store_lives_only_inside_stage", examples)]
#[verifies("rule_read_only_section_stays_inside_the_store", examples)]
fn staged_and_read_only_capabilities_hold_at_compile_time() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/staged_store_clone.rs");
    cases.compile_fail("tests/compile_fail/staged_store_outlives_stage.rs");
    cases.compile_fail("tests/compile_fail/staged_store_returned_from_stage.rs");
    cases.compile_fail("tests/compile_fail/staged_store_cannot_open_live.rs");
    cases.compile_fail("tests/compile_fail/layout_store_outside_crate.rs");
    cases.compile_fail("tests/compile_fail/read_only_section_outside_crate.rs");
}

#[test]
#[verifies("rule_store_has_no_wire_contract", examples)]
fn store_has_no_wire_contract() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/no_wire_contract.rs");
}

// Accepts only the narrow row, so a store-only column in `record_family_catalog!` does not
// compile here.
macro_rules! narrow_rows {
    ($rows:ident; $($group:ident {
        $($variant:ident {
            record: $record:ty,
            order: $order:tt,
            node: [$($node:tt)*],
            catalog: [$($catalog:tt)*],
            route: [$($route:tt)*]
            $(, review: $review:ident)?
        };)*
    })*) => {
        let $rows = [$($(
            stringify!($variant $record $order $($node)* $($catalog)* $($route)*),
        )*)*];
    };
}

#[test]
fn record_family_catalog_leaves_out_store_only_columns() {
    provenance_store::record_family_catalog!(narrow_rows, rows);
    assert!(rows.iter().any(|row| row.starts_with("Sources")));
    for row in rows {
        // Shard files, readers, graph exports and import budgets stay in the store.
        for store_only in [".jsonl", "list_sources", "source_pinned", "node_budget"] {
            assert!(!row.contains(store_only), "{row} names {store_only}");
        }
    }
}
