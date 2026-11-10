//! Tests for `rule_row_counted_records_hold_no_real_column`.
//!
//! These tests run with or without `host`. Each record kind that the write
//! check counts column by column is built with every field set, so a new
//! field fails to compile here until the test sets it. With every number set,
//! the row holds no REAL column and the write check counts it with no `SQLite`.
//! A REAL column itself counts only in the host build.

use crate::state_store::read_budget::{column_bytes, ReadBudget};
use provenance_core::model::projection_row::ColumnValue;
use provenance_core::model::ProjectionRow;
use provenance_core::{
    ArchivedStamp, ArtifactLink, ArtifactLinkTargetType, DeclarationAddress, ImplementationBinding,
    Question, QuestionStatus, Requirement, RequirementStatus, Resolution, ResolutionInput,
    ResolutionInputType, ResolutionMethod, ResolutionStatus, Rule, RuleSeverity, RuleStatus,
    ScopeId, Source, SourceReference, SourceType, StableId, Stamp, Topic, TopicStatus,
    VerificationBinding, VerificationMethod,
};
use provenance_macros::verifies;

const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

fn scope() -> ScopeId {
    ScopeId::new("default").unwrap()
}

fn id(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

fn stamp() -> Stamp {
    Stamp {
        commit: COMMIT.into(),
        at: "2026-11-10T10:00:00Z".into(),
    }
}

fn archived() -> ArchivedStamp {
    ArchivedStamp {
        commit: COMMIT.into(),
        at: Some("2026-11-10T10:00:00Z".into()),
    }
}

fn address() -> DeclarationAddress {
    DeclarationAddress::new(["spec", "entry"]).unwrap()
}

fn link() -> Vec<ArtifactLink> {
    vec![ArtifactLink {
        target_type: ArtifactLinkTargetType::Source,
        target_id: id("source_one"),
    }]
}

fn assert_counted_without_real<T: ProjectionRow + ReadBudget>(record: &T) {
    let row = record.row().unwrap();
    assert!(
        !row.iter()
            .any(|column| matches!(column, ColumnValue::Real(_))),
        "{} has a REAL column",
        std::any::type_name::<T>()
    );
    record.read_bytes().unwrap();
}

#[test]
#[verifies("rule_row_counted_records_hold_no_real_column", conformance)]
fn source_and_requirement_rows_hold_no_real_column() {
    assert_counted_without_real(&Source {
        scope_id: scope(),
        id: id("source_one"),
        created: Some(stamp()),
        updated: Some(stamp()),
        declared_by: Some("spec".into()),
        declaration_address: Some(address()),
        name: "Source".into(),
        source_type: SourceType::Document,
        url: Some("https://example.com".into()),
        reference: Some("section 1".into()),
        commit_pin: Some(COMMIT.into()),
        supersedes: vec![id("source_old")],
        origin_thread: Some(id("thread_one")),
        origin_message: Some(id("message_one")),
    });
    assert_counted_without_real(&Requirement {
        scope_id: scope(),
        id: id("req_one"),
        created: Some(stamp()),
        updated: Some(stamp()),
        declared_by: Some("spec".into()),
        declaration_address: Some(address()),
        statement: "Statement".into(),
        description: Some("Description".into()),
        status: RequirementStatus::Active,
        archived_in_commit: Some(archived()),
        source_refs: vec![SourceReference {
            source_id: id("source_one"),
            clause: Some("clause".into()),
        }],
        refines: vec![id("req_parent")],
        depends_on: vec![id("req_other")],
        supersedes: vec![id("req_old")],
        spawned_by: Some(id("req_spawner")),
        origin_thread: Some(id("thread_one")),
        origin_message: Some(id("message_one")),
    });
}

#[test]
#[verifies("rule_row_counted_records_hold_no_real_column", conformance)]
fn shaping_rows_hold_no_real_column() {
    assert_counted_without_real(&Topic {
        scope_id: scope(),
        id: id("topic_one"),
        requirement_id: id("req_one"),
        title: "Topic".into(),
        status: TopicStatus::Open,
        archived_in_commit: Some(archived()),
        claimed_by: Some("agent".into()),
        claimed_at: Some(1_762_768_800),
        links: link(),
    });
    assert_counted_without_real(&Question {
        scope_id: scope(),
        id: id("question_one"),
        topic_id: id("topic_one"),
        requirement_id: id("req_one"),
        question: "Question?".into(),
        resolution_method: ResolutionMethod::Ask,
        depends_on: vec![id("question_other")],
        status: QuestionStatus::Answered,
        archived_in_commit: Some(archived()),
        claimed_by: Some("agent".into()),
        claimed_at: Some(1_762_768_800),
        answer: Some("Answer".into()),
        links: link(),
        resolution_id: Some(id("res_one")),
        contradicts: Some(id("req_other")),
    });
    assert_counted_without_real(&Resolution {
        scope_id: scope(),
        id: id("res_one"),
        created: Some(stamp()),
        updated: Some(stamp()),
        title: "Resolution".into(),
        position: "Position".into(),
        rationale: "Rationale".into(),
        status: ResolutionStatus::Approved,
        archived_in_commit: Some(archived()),
        context: Some("Context".into()),
        inputs: vec![ResolutionInput {
            input_type: ResolutionInputType::Benchmark,
            reference: "source_one".into(),
            summary: "Summary".into(),
        }],
        made_by: Some("agent".into()),
        approved_by: Some("owner".into()),
        approved_at: Some(1_762_768_800),
        requirement_ids: vec![id("req_one")],
        supersedes: vec![id("res_old")],
        origin_thread: Some(id("thread_one")),
        origin_message: Some(id("message_one")),
    });
}

#[test]
#[verifies("rule_row_counted_records_hold_no_real_column", conformance)]
fn rule_and_binding_rows_hold_no_real_column() {
    assert_counted_without_real(&Rule {
        scope_id: scope(),
        id: id("rule_one"),
        created: Some(stamp()),
        updated: Some(stamp()),
        declared_by: Some("spec".into()),
        declaration_address: Some(address()),
        name: Some("Rule".into()),
        description: Some("Description".into()),
        statement: "Statement".into(),
        status: RuleStatus::Active,
        archived_in_commit: Some(archived()),
        severity: RuleSeverity::High,
        requirement_ids: vec![id("req_one")],
        resolution_ids: vec![id("res_one")],
        source_document: Some("plan".into()),
        source_section: Some("Phase 1".into()),
        origin_thread: Some(id("thread_one")),
        origin_message: Some(id("message_one")),
    });
    assert_counted_without_real(&ImplementationBinding {
        scope_id: scope(),
        id: id("impl_one"),
        rule_id: id("rule_one"),
        declared_by: "rust".into(),
        file: "src/lib.rs".into(),
        symbol: "column_bytes".into(),
    });
    assert_counted_without_real(&VerificationBinding {
        scope_id: scope(),
        id: id("verify_one"),
        rule_id: id("rule_one"),
        key: "key".into(),
        method: VerificationMethod::Conformance,
        declared_by: "rust".into(),
        file: "src/tests.rs".into(),
        symbol: Some("rows_hold_no_real_column".into()),
    });
}

#[cfg(not(feature = "host"))]
#[test]
#[verifies("rule_row_counted_records_hold_no_real_column", examples)]
fn real_column_is_refused_without_host() {
    let error = column_bytes(&ColumnValue::Real(0.5)).unwrap_err();
    assert_eq!(
        error.to_string(),
        "only the host build counts the bytes of a REAL column"
    );
}

#[cfg(feature = "host")]
#[tokio::test]
#[verifies("rule_row_counted_records_hold_no_real_column", conformance)]
async fn real_column_counts_as_sqlite_casts_it() {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    for real in [0.5, -0.1, 1.0, 1e300, f64::MIN_POSITIVE, 123_456.789] {
        let expected: i64 = sqlx::query_scalar("SELECT length(CAST(? AS BLOB))")
            .bind(real)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            column_bytes(&ColumnValue::Real(real)).unwrap(),
            usize::try_from(expected).unwrap(),
            "{real}"
        );
    }
    pool.close().await;
}
