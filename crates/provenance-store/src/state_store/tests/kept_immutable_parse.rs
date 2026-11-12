//! Implementation aid: a catch-up pass keeps each parse by path and record
//! type. A list read keeps the rows of the proposal file with their lines and
//! does not check the ids. The scope check over the kept rows does not skip
//! its check. No Rule binds it: the test pins how the kept parses are keyed,
//! and the public answer is in `disagreeing_ideation_ids`.

use crate::cache::tests::fixtures::open;
use crate::state_store::readers::StateLineInvalid;
use crate::state_store::{GuardedStore, SharedParses, ViewStore};
use std::sync::Arc;

#[test]
fn a_list_read_with_no_check_does_not_skip_the_check_of_the_scope() {
    let (_dir, store, scope) = super::initialized_store();
    let path = crate::shards::proposal_cards_path(&store.layout, &scope);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let row = serde_json::json!({
        "scope_id":"default", "id":"proposal_a", "proposal_key":"proposal-a",
        "proposal_type":"requirement_candidate", "title":"Candidate", "summary":"Candidate",
        "traceability":{"target":{"artifact_type":"requirement","artifact_id":"req_a"},
            "source_ids":[], "evidence_references":[], "supporting_claim_ids":[]},
        "promotion_state":"proposed"
    });
    std::fs::write(&path, format!("{row}\n{row}\n")).unwrap();
    let repository = open(&store.layout);
    let view = Arc::new(repository.storage().read_view().unwrap());
    let shared = GuardedStore::of_view(ViewStore::sharing(&repository, &SharedParses::new(view)));
    let listed = shared.list_proposal_definitions(&scope).unwrap();
    assert_eq!(listed.len(), 2);
    for _ in 0..2 {
        let error = shared.validate_ideation_scope(&scope).unwrap_err();
        let located = StateLineInvalid::find(&error).expect("a located refusal");
        assert_eq!(
            (located.file(), located.line, located.message.as_str()),
            (
                "scopes/default/ideation/proposal_cards.jsonl".to_owned(),
                2,
                "duplicate immutable proposal id proposal_a at line 1 and at line 2"
            ),
        );
    }
}
