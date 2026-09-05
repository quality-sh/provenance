//! The fetched `SqlFront` answers the same rows as the in-memory
//! `RecordFront` for every record of every test store, both ways and
//! under both flows, so a traversal cannot tell the two fronts apart.

use super::comparison::test_stores::{self, TestStore};
use crate::cache::read::SqlFront;
use crate::cache::{catch_up_state, open_cache};
use crate::operations::reader::ReadSnapshot;
use provenance_core::model::relations::{flow_neighbors, related_nodes, RecordFront};
use provenance_core::{NodeType, StableId};

fn sid(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

async fn assert_fronts_agree(store: &TestStore) {
    let state = store.state_store();
    let scope = &store.scope;
    let sources = state.list_sources(scope).unwrap();
    let requirements = state.list_requirements(scope).unwrap();
    let resolutions = state.list_resolutions(scope).unwrap();
    let rules = state.list_rules(scope).unwrap();
    let topics = state.list_topics(scope).unwrap();
    let questions = state.list_questions(scope).unwrap();
    let domains = state.list_domains(scope).unwrap();
    let boundaries = state.list_boundaries(scope).unwrap();
    let records = RecordFront {
        sources: &sources,
        requirements: &requirements,
        resolutions: &resolutions,
        rules: &rules,
        topics: &topics,
        questions: &questions,
        domains: &domains,
        boundaries: &boundaries,
    };
    let mut frontier: Vec<(NodeType, StableId)> = Vec::new();
    frontier.extend(sources.iter().map(|r| (NodeType::Source, r.id.clone())));
    frontier.extend(
        requirements
            .iter()
            .map(|r| (NodeType::Requirement, r.id.clone())),
    );
    frontier.extend(
        resolutions
            .iter()
            .map(|r| (NodeType::Resolution, r.id.clone())),
    );
    frontier.extend(rules.iter().map(|r| (NodeType::Rule, r.id.clone())));
    frontier.extend(topics.iter().map(|r| (NodeType::Topic, r.id.clone())));
    frontier.extend(questions.iter().map(|r| (NodeType::Question, r.id.clone())));
    frontier.extend(domains.iter().map(|r| (NodeType::Domain, r.id.clone())));
    frontier.extend(
        boundaries
            .iter()
            .map(|r| (NodeType::Boundary, r.id.clone())),
    );
    assert!(
        !frontier.is_empty(),
        "{}: an empty store proves nothing",
        store.name
    );

    catch_up_state(&store.layout()).await.unwrap();
    let pool = open_cache(&store.layout()).await.unwrap();
    let snapshot = ReadSnapshot::open(&pool, scope)
        .await
        .unwrap()
        .expect("a revision");
    let sql = SqlFront::hop(&snapshot.relations(), &frontier)
        .await
        .unwrap();
    for (kind, id) in &frontier {
        let context = format!("{}: {kind:?} {}", store.name, id.as_str());
        assert_eq!(
            related_nodes(&records, *kind, id),
            related_nodes(&sql, *kind, id),
            "{context}: related nodes differ"
        );
        for downstream in [true, false] {
            assert_eq!(
                flow_neighbors(&records, *kind, id, downstream),
                flow_neighbors(&sql, *kind, id, downstream),
                "{context}: flow neighbours differ (downstream {downstream})"
            );
        }
    }
    drop(snapshot);
    pool.close().await;
}

#[tokio::test]
async fn the_sql_front_agrees_with_the_record_front_over_every_store() {
    assert_fronts_agree(&test_stores::seeded_queries()).await;
    for store in test_stores::cache_fixtures() {
        assert_fronts_agree(&store).await;
    }
    assert_fronts_agree(&TestStore::pinned()).await;
    assert_fronts_agree(&test_stores::repository_state()).await;
}

/// A hop answers the frontier it fetched; a lookup outside it is a
/// programming error, caught in a debug build.
#[tokio::test]
#[cfg_attr(
    debug_assertions,
    should_panic(expected = "outside the fetched frontier")
)]
async fn a_lookup_outside_the_frontier_is_an_invariant_violation() {
    let store = TestStore::pinned();
    catch_up_state(&store.layout()).await.unwrap();
    let pool = open_cache(&store.layout()).await.unwrap();
    let snapshot = ReadSnapshot::open(&pool, &store.scope)
        .await
        .unwrap()
        .expect("a revision");
    let sql = SqlFront::hop(
        &snapshot.relations(),
        &[(NodeType::Requirement, sid("req_overtime"))],
    )
    .await
    .unwrap();
    let _ = related_nodes(&sql, NodeType::Requirement, &sid("req_penalty"));
}

/// The front interns relation names to the declared vocabulary; a row
/// naming a relation no declaration carries refuses the hop.
#[tokio::test]
async fn a_relation_row_with_an_undeclared_name_is_refused() {
    let store = TestStore::pinned();
    catch_up_state(&store.layout()).await.unwrap();
    let pool = open_cache(&store.layout()).await.unwrap();
    sqlx::query(
        "INSERT INTO relations (scope_id, owner_type, owner_id, relation, target_type, target_id) \
         VALUES (?, 'requirement', 'req_overtime', 'befriends', 'source', 'source_schads')",
    )
    .bind(store.scope.as_str())
    .execute(&pool)
    .await
    .unwrap();
    let snapshot = ReadSnapshot::open(&pool, &store.scope)
        .await
        .unwrap()
        .expect("a revision");
    let refused = SqlFront::hop(
        &snapshot.relations(),
        &[(NodeType::Requirement, sid("req_overtime"))],
    )
    .await
    .unwrap_err();
    assert!(refused.to_string().contains("befriends"), "{refused}");
    drop(snapshot);
    pool.close().await;
}
