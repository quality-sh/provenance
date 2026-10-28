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
    cases.compile_fail("tests/compile_fail/staged_store_cannot_open_live.rs");
    cases.compile_fail("tests/compile_fail/layout_store_outside_crate.rs");
    cases.compile_fail("tests/compile_fail/read_only_section_outside_crate.rs");
}
