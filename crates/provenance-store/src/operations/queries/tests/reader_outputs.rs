//! What each row reader behind the snapshot handles gives over one fixed
//! store: the rows, their order, and the refusals. The store is the pinned
//! store plus discussion, payload and refusal rows written straight into
//! the read database. The expected lines are in `reader_outputs.txt`; on a
//! difference the test prints every line it read.

use super::comparison::test_stores::TestStore;
use crate::cache::read::payloads::PayloadRow;
use crate::cache::{catch_up_state, open_cache, CacheConnection};
use crate::operations::reader::{kind_of, Position, ReadSnapshot, SqlFront};
use crate::read_database::contract::OtherTransaction;
use provenance_core::model::relations::RelationSource;
use provenance_core::{
    DispositionRecord, ImplementationBinding, NodeType, Requirement, Rule, Source, StableId,
    VerificationBinding,
};
use serde::Serialize;
use std::fmt::Debug;

mod gaps;
mod seed;

const EXPECTED: &str = include_str!("reader_outputs.txt");

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

/// The snapshot of the seeded store. With `port_only`, its transaction is
/// behind another type, so a reader that reaches for `SQLx` fails.
async fn snapshot_of(store: &TestStore, port_only: bool) -> (CacheConnection, ReadSnapshot) {
    catch_up_state(&store.repository()).await.unwrap();
    let pool = open_cache(&store.layout()).await.unwrap();
    seed::seed(pool.pool()).await;
    let mut transaction = crate::read_database::file::begin(pool.pool())
        .await
        .unwrap();
    if port_only {
        transaction = OtherTransaction::forward(transaction);
    }
    let snapshot = ReadSnapshot::open(transaction, &store.scope)
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
        "rule page_ids",
        rules.page_ids("", 3, false, None).await,
    ));
    lines.push(line(
        "rule page_ids after",
        rules.page_ids("rule_overtime_001", 50, true, None).await,
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

/// Every line that the readers give over the snapshot, in order.
async fn lines_of(snapshot: &ReadSnapshot) -> Vec<String> {
    let mut lines = Vec::new();
    records(snapshot, &mut lines).await;
    pages(snapshot, &mut lines).await;
    graph(snapshot, &mut lines).await;
    discussions(snapshot, &mut lines).await;
    gaps::chunks(snapshot, &mut lines).await;
    gaps::ceilings(snapshot, &mut lines).await;
    gaps::limits(snapshot, &mut lines).await;
    lines
}

/// Compares the lines with the expected file, and prints them all on a
/// difference.
fn compare(lines: &[String]) {
    let actual = lines.join("\n") + "\n";
    if actual != EXPECTED {
        println!("=== READER OUTPUTS\n{actual}=== END READER OUTPUTS");
        for (index, (left, right)) in actual.lines().zip(EXPECTED.lines()).enumerate() {
            assert_eq!(left, right, "line {} differs", index + 1);
        }
        panic!("the reader outputs differ in their line count");
    }
}

/// The lines over a snapshot of the seeded store, with its attested set.
async fn read_all(port_only: bool) -> Vec<String> {
    let store = TestStore::pinned();
    let (pool, snapshot) = snapshot_of(&store, port_only).await;
    let mut lines = lines_of(&snapshot).await;
    let (_, attested) = snapshot.finish();
    lines.push(format!("attested: {attested:?}"));
    pool.close().await.unwrap();
    lines
}

/// The readers give the same rows, in the same order, with the same
/// refusals, as the expected lines hold.
#[tokio::test]
async fn each_reader_gives_the_expected_rows_and_refusals() {
    compare(&read_all(false).await);
}

/// Over a transaction that is not the file's own type, every reader gives
/// the same lines: no reader needs `SQLx`.
#[tokio::test]
async fn each_reader_gives_the_same_lines_through_the_port_alone() {
    compare(&read_all(true).await);
}

/// The readers name no `SQLx` item, so only the port runs their
/// statements: not `cache/read.rs`, and no file under `cache/read` at any
/// depth.
#[test]
fn the_row_readers_name_no_sqlx() {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut pending = vec![source.join("cache/read.rs"), source.join("cache/read")];
    let mut named = Vec::new();
    while let Some(path) = pending.pop() {
        if path.is_dir() {
            for entry in std::fs::read_dir(&path).unwrap() {
                pending.push(entry.unwrap().path());
            }
        } else if std::fs::read_to_string(&path).unwrap().contains("sqlx") {
            named.push(path.strip_prefix(&source).unwrap().display().to_string());
        }
    }
    assert_eq!(named, Vec::<String>::new());
}
