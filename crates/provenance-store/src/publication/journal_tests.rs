use super::journal::{
    drain_window, journal_head, normalize_head, prune_through, record_events, JournalEvent,
};
use crate::layout::ProvenanceLayout;

fn test_layout() -> (tempfile::TempDir, ProvenanceLayout) {
    let directory = tempfile::tempdir().unwrap();
    let root = camino::Utf8PathBuf::from_path_buf(directory.path().to_path_buf()).unwrap();
    (directory, ProvenanceLayout::new(root))
}

fn event(scope: &str, family: &str, record_id: &str) -> JournalEvent {
    JournalEvent {
        sequence: 0,
        scope: scope.to_string(),
        family: family.to_string(),
        record_id: record_id.to_string(),
        operation: "upsert".to_string(),
    }
}

#[test]
fn appended_events_carry_increasing_sequences() {
    let (_dir, layout) = test_layout();
    record_events(&layout, &[event("default", "requirements", "req_a")]).unwrap();
    record_events(&layout, &[event("default", "edges", "edge_b")]).unwrap();

    let events = drain_window(&layout, 0, u64::MAX).unwrap();
    let sequences = events
        .iter()
        .map(|event| event.sequence)
        .collect::<Vec<_>>();
    assert_eq!(sequences, vec![1, 2]);
    assert_eq!(journal_head(&layout).unwrap(), 3);
}

#[test]
fn normalization_after_head_loss_never_reuses_a_number() {
    let (_dir, layout) = test_layout();
    record_events(&layout, &[event("default", "requirements", "req_a")]).unwrap();
    std::fs::remove_file(layout.journal_head_path()).unwrap();

    let head = normalize_head(&layout, 0).unwrap();

    assert_eq!(
        head, 2,
        "head must be one above the surviving tail high-water"
    );
    record_events(&layout, &[event("default", "rules", "rule_b")]).unwrap();
    let sequences = drain_window(&layout, 0, u64::MAX)
        .unwrap()
        .into_iter()
        .map(|event| event.sequence)
        .collect::<Vec<_>>();
    assert_eq!(sequences, vec![1, 2]);
}

#[test]
fn normalization_floors_head_at_the_stored_revision_serial() {
    let (_dir, layout) = test_layout();

    let head = normalize_head(&layout, 5).unwrap();

    assert_eq!(head, 6);
}

#[test]
fn normalization_takes_the_max_of_tail_and_head_record() {
    let (_dir, layout) = test_layout();
    record_events(&layout, &[event("default", "requirements", "req_a")]).unwrap();
    // Simulate a crash between append and head advance: the tail holds
    // sequence 1 but the head record still says 1 (next = 1).
    std::fs::write(layout.journal_head_path(), r#"{"next_sequence":1}"#).unwrap();

    let head = normalize_head(&layout, 0).unwrap();

    assert_eq!(head, 2);
}

#[test]
fn drain_window_returns_only_the_events_above_the_stored_serial() {
    let (_dir, layout) = test_layout();
    record_events(&layout, &[event("default", "requirements", "req_a")]).unwrap();
    record_events(&layout, &[event("default", "rules", "rule_b")]).unwrap();
    record_events(&layout, &[event("default", "domains", "domain_c")]).unwrap();

    let drained = drain_window(&layout, 2, 4).unwrap();

    let families = drained
        .iter()
        .map(|event| event.family.as_str())
        .collect::<Vec<_>>();
    assert_eq!(families, vec!["domains"]);
}

#[test]
fn prune_through_removes_drained_events_and_keeps_the_rest() {
    let (_dir, layout) = test_layout();
    record_events(&layout, &[event("default", "requirements", "req_a")]).unwrap();
    record_events(&layout, &[event("default", "rules", "rule_b")]).unwrap();

    prune_through(&layout, 1).unwrap();

    let remaining = drain_window(&layout, 0, u64::MAX).unwrap();
    let families = remaining
        .iter()
        .map(|event| event.family.as_str())
        .collect::<Vec<_>>();
    assert_eq!(families, vec!["rules"]);
}

#[test]
fn journal_loss_with_the_database_alive_floors_the_head_at_stored_plus_one() {
    let (_dir, layout) = test_layout();
    record_events(&layout, &[event("default", "requirements", "req_a")]).unwrap();
    std::fs::remove_file(layout.journal_events_path()).unwrap();

    let head = normalize_head(&layout, 7).unwrap();

    assert_eq!(head, 8);
}
