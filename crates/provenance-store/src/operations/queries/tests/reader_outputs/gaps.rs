//! The reader paths that the first lines do not reach: lists longer than
//! one bound chunk, rows between the record and resource ceilings, an id
//! over the id ceiling in a document, and limits below the row count.

use super::seed::{long_thread_id, WIDE_STATEMENT_BYTES};
use super::{line, sid};
use crate::operations::reader::{Position, ReadSnapshot, SqlFront};
use provenance_core::model::relations::RelationSource;
use provenance_core::{NodeType, Rule, StableId, VerificationBinding};

/// More ids than one bound chunk takes. The values sort before they are
/// chunked, so the padding ids fall between the real ones and each chunk
/// holds real rows.
fn chunked_ids() -> Vec<StableId> {
    let mut ids = vec![sid("rule_twin_record"), sid("rule_over_002")];
    ids.extend((0..600).map(|n| sid(&format!("rule_pad_{n:03}"))));
    ids.extend([
        sid("rule_penalty_001"),
        sid("rule_overtime_001"),
        sid("rule_over_002"),
    ]);
    ids
}

fn ids(records: &[Rule]) -> Vec<&str> {
    records.iter().map(|record| record.id.as_str()).collect()
}

pub(super) async fn chunks(snapshot: &ReadSnapshot, lines: &mut Vec<String>) {
    let rules = snapshot.table::<Rule>();
    let wanted = chunked_ids();
    let words: Vec<&str> = wanted.iter().map(StableId::as_str).collect();
    lines.push(line(
        "chunked by_ids",
        rules
            .by_ids(&wanted)
            .await
            .map(|found| ids(&found).join(",")),
    ));
    lines.push(line(
        "chunked by_field",
        rules
            .by_field("id", &words)
            .await
            .map(|found| ids(&found).join(",")),
    ));
    lines.push(line(
        "chunked checked_by_ids",
        rules
            .checked_by_ids(&wanted)
            .await
            .map(|found| ids(&found).join(",")),
    ));
    for limit in [1, 3, 10] {
        lines.push(line(
            &format!("chunked page_by_ids {limit}"),
            rules
                .page_by_ids(&wanted, limit)
                .await
                .map(|(found, more)| (ids(&found).join(","), more)),
        ));
    }
    lines.push(line(
        "chunked ids_that_count",
        rules.ids_that_count(&wanted).await,
    ));
    let mut frontier = vec![(NodeType::Requirement, sid("req_top"))];
    frontier.extend((0..600).map(|n| (NodeType::Requirement, sid(&format!("req_pad_{n:03}")))));
    frontier.push((NodeType::Requirement, sid("req_overtime")));
    match SqlFront::hop(&snapshot.relations(), &frontier).await {
        Ok(front) => {
            for id in ["req_top", "req_overtime", "req_pad_300"] {
                let id = sid(id);
                let out = front.outgoing(NodeType::Requirement, &id).len();
                let into = front.incoming(NodeType::Requirement, &id).len();
                lines.push(format!(
                    "chunked front {}: {out} out, {into} in",
                    id.as_str()
                ));
            }
        }
        Err(error) => lines.push(format!("chunked front: refused: {error:#}")),
    }
}

pub(super) async fn ceilings(snapshot: &ReadSnapshot, lines: &mut Vec<String>) {
    let rules = snapshot.table::<Rule>();
    let size = |record: Option<Rule>| record.map(|rule| (rule.id, rule.statement.len()));
    lines.push(format!("wide statement bytes: {WIDE_STATEMENT_BYTES}"));
    lines.push(line(
        "wide page_record",
        rules.page_record("rule_wide").await.map(size),
    ));
    lines.push(line(
        "wide resource_record",
        rules.resource_record("rule_wide").await.map(size),
    ));
    lines.push(line(
        "wide record",
        rules.record(&sid("rule_wide")).await.map(size),
    ));
    lines.push(line(
        "wide checked_by_ids",
        rules
            .checked_by_ids(&[sid("rule_penalty_001"), sid("rule_wide")])
            .await
            .map(|found| ids(&found).join(",")),
    ));
    lines.push(line(
        "wide checked_by_field",
        rules
            .checked_by_field("status", &["active"])
            .await
            .map(|found| ids(&found).join(",")),
    ));
    for limit in [1, 2] {
        lines.push(line(
            &format!("wide page_by_ids {limit}"),
            rules
                .page_by_ids(&[sid("rule_wide"), sid("rule_twin_record")], limit)
                .await
                .map(|(found, more)| (ids(&found).join(","), more)),
        ));
    }
    let long = long_thread_id();
    lines.push(line(
        "long thread id document_keys",
        snapshot
            .document_keys("req_bottom", &Position::default(), 50, false, &[])
            .await,
    ));
    lines.push(line(
        "long thread id page_thread",
        snapshot
            .page_thread(&long)
            .await
            .map(|thread| thread.map(|thread| thread.id.as_str().len())),
    ));
}

pub(super) async fn limits(snapshot: &ReadSnapshot, lines: &mut Vec<String>) {
    for limit in [1, 2, 6] {
        lines.push(line(
            &format!("document_keys limit {limit}"),
            snapshot
                .document_keys("req_top", &Position::default(), limit, false, &[])
                .await,
        ));
    }
    let after = Position {
        stage: 2,
        rank: 1,
        counter: 10,
        id: "thread_c".to_owned(),
    };
    lines.push(line(
        "document_keys after thread_c limit 2",
        snapshot
            .document_keys("req_top", &after, 2, false, &[])
            .await,
    ));
    lines.push(line(
        "long thread id document_keys limit 1",
        snapshot
            .document_keys("req_bottom", &Position::default(), 1, false, &[])
            .await,
    ));
    let rules = snapshot.table::<Rule>();
    lines.push(line(
        "rule page_ids after limit 1",
        rules.page_ids("rule_overtime_001", 1, true, None).await,
    ));
    lines.push(line(
        "rule page_ids after limit 2",
        rules.page_ids("rule_over_003", 2, false, None).await,
    ));
    let bindings = snapshot.table::<VerificationBinding>();
    lines.push(line(
        "verification ids limit 2",
        bindings.resource_ids_for_rule("", 2, None).await,
    ));
    lines.push(line(
        "verification ids after limit 1",
        bindings
            .resource_ids_for_rule("bind_ver_a", 1, Some(&sid("rule_overtime_001")))
            .await,
    ));
    lines.push(line(
        "requirement page_by_field limit 1",
        snapshot
            .table::<provenance_core::Requirement>()
            .page_by_field("status", &["active"], "", 1)
            .await
            .map(|(found, more)| {
                (
                    found
                        .iter()
                        .map(|record| record.id.as_str().to_owned())
                        .collect::<Vec<_>>(),
                    more,
                )
            }),
    ));
}
