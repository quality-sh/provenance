mod relation_decl {
use crate::model::relations::{RelationDecl, RelationFlow, RelationOwner};
use crate::model::{NodeType, SourceReference, StableId};
use provenance_macros::Relations;

fn sid(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

/// A stand-in with every field shape the derive reads: a required single,
/// an optional single, a list, a required list, a citation through a struct,
/// an exempt pointer, and the owner key.
#[derive(Relations)]
struct Question {
    id: StableId,
    #[relation(target = Topic, flow = none)]
    topic_id: StableId,
    #[relation(target = Requirement, flow = target_upstream)]
    refines: Option<StableId>,
    #[relation(target = Requirement, flow = target_downstream)]
    depends_on: Vec<StableId>,
    #[relation(target = Requirement, flow = target_upstream, required)]
    requirement_ids: Vec<StableId>,
    #[relation(target = Source, flow = target_upstream, name = "cites", via = source_id)]
    source_refs: Vec<SourceReference>,
    #[relation(none)]
    origin_thread: Option<StableId>,
    title: String,
}

fn fixture() -> Question {
    Question {
        id: sid("question_threshold"),
        topic_id: sid("topic_rates"),
        refines: Some(sid("req_parent")),
        depends_on: vec![sid("req_first"), sid("req_second")],
        requirement_ids: vec![sid("req_overtime")],
        source_refs: vec![SourceReference {
            source_id: sid("source_award"),
            clause: Some("4.2".into()),
        }],
        origin_thread: Some(sid("thread_shaping")),
        title: "Threshold".into(),
    }
}

#[test]
fn the_table_names_every_declared_field_in_struct_order() {
    let rows: Vec<(&str, NodeType, bool, bool, RelationFlow)> = Question::RELATIONS
        .iter()
        .map(|row| (row.name, row.target, row.list, row.required, row.flow))
        .collect();
    assert_eq!(
        rows,
        [
            ("topic_id", NodeType::Topic, false, true, RelationFlow::None),
            ("refines", NodeType::Requirement, false, false, RelationFlow::TargetUpstream),
            ("depends_on", NodeType::Requirement, true, false, RelationFlow::TargetDownstream),
            ("requirement_ids", NodeType::Requirement, true, true, RelationFlow::TargetUpstream),
            ("cites", NodeType::Source, true, false, RelationFlow::TargetUpstream),
        ]
    );
    assert!(Question::RELATIONS.iter().all(|row| row.owner == NodeType::Question));
    assert_eq!(<Question as RelationOwner>::OWNER, NodeType::Question);
    assert_eq!(Question::relations().len(), 5);
}

#[test]
fn references_walk_every_declared_field_and_skip_exempt_ones() {
    let question = fixture();
    let references: Vec<(&str, &str)> = question
        .references()
        .into_iter()
        .map(|(name, id)| (name, id.as_str()))
        .collect();
    assert_eq!(
        references,
        [
            ("topic_id", "topic_rates"),
            ("refines", "req_parent"),
            ("depends_on", "req_first"),
            ("depends_on", "req_second"),
            ("requirement_ids", "req_overtime"),
            ("cites", "source_award"),
        ]
    );
    assert_eq!(question.id().as_str(), "question_threshold");
    assert_eq!(
        question.origin_thread.as_ref().map(StableId::as_str),
        Some("thread_shaping"),
        "an exempt pointer stays a plain field"
    );
    assert_eq!(question.title, "Threshold");
}

#[test]
fn an_empty_optional_or_list_contributes_no_reference() {
    let question = Question {
        refines: None,
        depends_on: Vec::new(),
        source_refs: Vec::new(),
        ..fixture()
    };
    let names: Vec<&str> = question.references().into_iter().map(|(name, _)| name).collect();
    assert_eq!(names, ["topic_id", "requirement_ids"]);
}

#[test]
fn a_declaration_row_is_plain_data() {
    let row: &RelationDecl = &Question::RELATIONS[4];
    assert_eq!(row.name, "cites");
    assert_eq!(row.target, NodeType::Source);
}
}
