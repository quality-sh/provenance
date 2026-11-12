//! What each row reader behind the snapshot handles gives over one fixed
//! store: the rows, their order, and the refusals. The store is the pinned
//! store plus discussion, payload and refusal rows written straight into
//! the read database. The expected lines are in `reader_outputs.txt`; on a
//! difference the test prints every line it read.

use super::comparison::test_stores::TestStore;
use crate::cache::read::payloads::PayloadRow;
use crate::cache::{catch_up_state, open_cache, CacheConnection};
use crate::operations::reader::{kind_of, Position, ReadSnapshot, SqlFront};
use provenance_core::model::relations::RelationSource;
use provenance_core::{
    DispositionRecord, ImplementationBinding, NodeType, Requirement, Rule, Source, StableId,
    VerificationBinding,
};
use serde::Serialize;
use sqlx::SqlitePool;
use std::fmt::Debug;

const EXPECTED: &str = include_str!("reader_outputs.txt");

const DISPOSITION: &str = r#"{"scope_id":"default","id":"ID","proposal_id":"PROPOSAL","decision":"accepted","actor":{"identity_type":"human","id":"reader"}}"#;

fn sid(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

/// One line of the answer: the value as compact JSON, or the refusal.
fn line<T: Serialize>(label: &str, result: anyhow::Result<T>) -> String {
    match result {
        Ok(value) => format!("{label}: {}", serde_json::to_string(&value).unwrap()),
        Err(error) => format!("{label}: refused: {error:#}"),
    }
}

fn debug_line<T: Debug>(label: &str, result: anyhow::Result<T>) -> String {
    match result {
        Ok(value) => format!("{label}: {value:?}"),
        Err(error) => format!("{label}: refused: {error:#}"),
    }
}

async fn run(pool: &SqlitePool, sql: &str, values: &[&str]) {
    let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
    for value in values {
        query = query.bind(*value);
    }
    query.execute(pool).await.unwrap();
}

/// Rows the pinned store has none of, and rows that make a reader refuse.
async fn seed(pool: &SqlitePool) {
    let thread =
        "INSERT INTO threads (scope_id, id, parent_type, parent_id, status, created_at, payload) \
                  VALUES ('default', ?, 'requirement', ?, ?, ?, '{}')";
    run(pool, thread, &["thread_b", "req_top", "active", "20"]).await;
    run(pool, thread, &["thread_a", "req_top", "resolved", "20"]).await;
    run(pool, thread, &["thread_c", "req_left", "archived", "10"]).await;
    let message =
        "INSERT INTO messages (scope_id, id, thread_id, role, body, created_at, payload) \
                   VALUES ('default', ?, ?, ?, ?, ?, '{}')";
    run(
        pool,
        message,
        &["message_b", "thread_a", "user", "first", "5"],
    )
    .await;
    run(
        pool,
        message,
        &["message_a", "thread_b", "assistant", "second", "5"],
    )
    .await;
    run(
        pool,
        message,
        &["message_c", "thread_a", "system", "third", "1"],
    )
    .await;
    let large = "x".repeat(70_000);
    run(
        pool,
        message,
        &["message_large", "thread_c", "user", &large, "30"],
    )
    .await;
    let disposition = "INSERT INTO dispositions (scope_id, id, proposal_id, decision, rationale, actor, payload) \
                       VALUES ('default', ?, ?, 'accepted', '', 'reader', ?)";
    for (id, proposal) in [
        ("disposition_b", "proposal_one"),
        ("disposition_a", "proposal_two"),
        ("disposition_c", "proposal_one"),
    ] {
        let payload = DISPOSITION
            .replace("\"ID\"", &format!("\"{id}\""))
            .replace("PROPOSAL", proposal);
        run(pool, disposition, &[id, proposal, &payload]).await;
    }
    let huge = format!("{{\"padding\":\"{}\"}}", "y".repeat(1_100_000));
    run(
        pool,
        disposition,
        &["disposition_huge", "proposal_one", &huge],
    )
    .await;
    let long_id = format!("disposition_z{}", "z".repeat(1100));
    run(pool, disposition, &[&long_id, "proposal_two", "{}"]).await;
    run(
        pool,
        "UPDATE sources SET name = CAST(name AS BLOB) WHERE id = 'source_award_2019'",
        &[],
    )
    .await;
}

async fn snapshot_of(store: &TestStore) -> (CacheConnection, ReadSnapshot) {
    catch_up_state(&store.repository()).await.unwrap();
    let pool = open_cache(&store.layout()).await.unwrap();
    seed(pool.pool()).await;
    let snapshot = ReadSnapshot::open(
        crate::read_database::file::begin(pool.pool())
            .await
            .unwrap(),
        &store.scope,
    )
    .await
    .unwrap()
    .expect("a revision");
    (pool, snapshot)
}

async fn records(snapshot: &ReadSnapshot, lines: &mut Vec<String>) {
    let rules = snapshot.table::<Rule>();
    let sources = snapshot.table::<Source>();
    let requirements = snapshot.table::<Requirement>();
    let wanted = [
        sid("rule_penalty_001"),
        sid("rule_none"),
        sid("rule_overtime_001"),
    ];
    lines.push(line(
        "rule record",
        rules.record(&sid("rule_overtime_001")).await,
    ));
    lines.push(line(
        "rule record absent",
        rules.record(&sid("rule_none")).await,
    ));
    lines.push(line(
        "source record blob",
        sources.record(&sid("source_award_2019")).await,
    ));
    lines.push(line("rule by_ids", rules.by_ids(&wanted).await));
    lines.push(line(
        "requirement by_field",
        requirements.by_field("status", &["active", "none"]).await,
    ));
    lines.push(line(
        "rule checked_by_ids",
        rules.checked_by_ids(&wanted).await,
    ));
    lines.push(line(
        "rule page_by_ids 1",
        rules.page_by_ids(&wanted, 1).await,
    ));
    lines.push(line(
        "requirement page_by_field",
        requirements
            .page_by_field("status", &["active"], "id <> 'req_left'", 2)
            .await,
    ));
    lines.push(line(
        "rule ids_that_count",
        rules.ids_that_count(&wanted).await,
    ));
    lines.push(line(
        "rule live",
        rules.live(&sid("rule_overtime_001")).await,
    ));
    lines.push(line(
        "rule live absent",
        rules.live(&sid("rule_none")).await,
    ));
    lines.push(line("rule search", rules.search("OVER").await));
    lines.push(line("source search blob", sources.search("award").await));
    let sites = snapshot
        .table::<ImplementationBinding>()
        .implementation_sites_for_file("src/pay.rs")
        .await
        .map(|sites| {
            sites
                .into_iter()
                .map(|site| (site.rule_id, site.symbol))
                .collect::<Vec<_>>()
        });
    lines.push(line("implementation sites", sites));
    let sites = snapshot
        .table::<VerificationBinding>()
        .verification_sites_for_file("src/pay.rs")
        .await
        .map(|sites| {
            sites
                .into_iter()
                .map(|site| (site.rule_id, site.symbol, site.method))
                .collect::<Vec<_>>()
        });
    lines.push(line("verification sites", sites));
    for id in [
        "source_schads",
        "req_top",
        "res_overtime",
        "rule_overtime_001",
        "topic_rates",
    ] {
        lines.push(debug_line(
            &format!("kind_of {id}"),
            kind_of(snapshot, &sid(id)).await,
        ));
    }
    lines.push(debug_line(
        "kind_of absent",
        kind_of(snapshot, &sid("none")).await,
    ));
}

async fn pages(snapshot: &ReadSnapshot, lines: &mut Vec<String>) {
    let rules = snapshot.table::<Rule>();
    lines.push(line(
        "rule page_record",
        rules.page_record("rule_overtime_001").await,
    ));
    lines.push(line(
        "rule resource_record",
        rules.resource_record("rule_penalty_001").await,
    ));
    lines.push(line(
        "rule resource_record absent",
        rules.resource_record("none").await,
    ));
    lines.push(line(
        "rule search_ids",
        rules.search_ids("", 3, false).await,
    ));
    lines.push(line(
        "rule search_ids after",
        rules.search_ids("rule_overtime_001", 50, true).await,
    ));
    let bindings = snapshot.table::<VerificationBinding>();
    lines.push(line(
        "verification ids",
        bindings.resource_ids_for_rule("", 50, None).await,
    ));
    lines.push(line(
        "verification ids for rule",
        bindings
            .resource_ids_for_rule("", 50, Some(&sid("rule_overtime_001")))
            .await,
    ));
    let payloads = snapshot.payloads::<DispositionRecord>();
    lines.push(line(
        "disposition record",
        payloads.record("disposition_a").await,
    ));
    lines.push(line(
        "disposition record absent",
        payloads.record("none").await,
    ));
    lines.push(line(
        "disposition record huge",
        payloads.record("disposition_huge").await,
    ));
    lines.push(line("disposition ids 3", payloads.ids("", 3).await));
    lines.push(line("disposition ids all", payloads.ids("", 50).await));
    lines.push(line(
        "disposition proposal_record",
        payloads
            .proposal_record(&sid("proposal_one"), "disposition_c")
            .await,
    ));
    lines.push(line(
        "disposition proposal_record other",
        payloads
            .proposal_record(&sid("proposal_two"), "disposition_c")
            .await,
    ));
    lines.push(line(
        "disposition proposal_ids",
        payloads.proposal_ids(&sid("proposal_one"), "", 50).await,
    ));
    lines.push(format!("disposition table: {}", DispositionRecord::TABLE));
}

async fn graph(snapshot: &ReadSnapshot, lines: &mut Vec<String>) {
    let frontier = [
        (NodeType::Requirement, sid("req_overtime")),
        (NodeType::Source, sid("source_schads")),
        (NodeType::Rule, sid("rule_overtime_001")),
    ];
    match SqlFront::hop(&snapshot.relations(), &frontier).await {
        Ok(front) => {
            for (kind, id) in &frontier {
                let out = front.outgoing(*kind, id);
                let into = front.incoming(*kind, id);
                lines.push(format!("front {} out: {out:?}", id.as_str()));
                lines.push(format!("front {} in: {into:?}", id.as_str()));
            }
        }
        Err(error) => lines.push(format!("front: refused: {error:#}")),
    }
    for exclude in [false, true] {
        lines.push(line(
            &format!("document_keys exclude {exclude}"),
            snapshot
                .document_keys("req_top", &Position::default(), 50, exclude, &[])
                .await,
        ));
    }
    let after = Position {
        stage: 1,
        ..Position::default()
    };
    let hidden = [(NodeType::Requirement, sid("req_left"))];
    lines.push(line(
        "document_keys after hidden",
        snapshot
            .document_keys("req_top", &after, 50, false, &hidden)
            .await,
    ));
    for (kind, id) in [
        (NodeType::Rule, "rule_overtime_001"),
        (NodeType::Source, "source_schads"),
        (NodeType::Requirement, "req_left"),
    ] {
        lines.push(line(
            &format!("document roots {id}"),
            snapshot.containing_document_roots(kind, &sid(id)).await,
        ));
    }
}

async fn discussions(snapshot: &ReadSnapshot, lines: &mut Vec<String>) {
    lines.push(line("threads", snapshot.threads().await));
    let messages = snapshot.messages().await.map(|messages| {
        messages
            .into_iter()
            .map(|mut message| {
                message.body.truncate(10);
                message
            })
            .collect::<Vec<_>>()
    });
    lines.push(line("messages", messages));
    lines.push(line("page_thread", snapshot.page_thread("thread_a").await));
    lines.push(line(
        "page_thread absent",
        snapshot.page_thread("none").await,
    ));
    lines.push(line(
        "page_message",
        snapshot.page_message("message_c").await,
    ));
    lines.push(line(
        "page_message large",
        snapshot.page_message("message_large").await,
    ));
}

/// The readers give the same rows, in the same order, with the same
/// refusals, as the expected lines hold.
#[tokio::test]
async fn each_reader_gives_the_expected_rows_and_refusals() {
    let store = TestStore::pinned();
    let (pool, snapshot) = snapshot_of(&store).await;
    let mut lines = Vec::new();
    records(&snapshot, &mut lines).await;
    pages(&snapshot, &mut lines).await;
    graph(&snapshot, &mut lines).await;
    discussions(&snapshot, &mut lines).await;
    let (_, attested) = snapshot.finish();
    lines.push(format!("attested: {attested:?}"));
    pool.close().await.unwrap();
    let actual = lines.join("\n") + "\n";
    if actual != EXPECTED {
        println!("=== READER OUTPUTS\n{actual}=== END READER OUTPUTS");
        for (index, (left, right)) in actual.lines().zip(EXPECTED.lines()).enumerate() {
            assert_eq!(left, right, "line {} differs", index + 1);
        }
        panic!("the reader outputs differ in their line count");
    }
}
