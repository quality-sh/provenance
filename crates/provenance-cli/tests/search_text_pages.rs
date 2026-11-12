//! Text search pages through the CLI JSON form on repositories with more than
//! 512 records of one kind: a kind filter, two kinds, a search over many
//! pages, letter case, the characters `%` and `_`, a search with no text, a
//! text across two pieces, and candidates that fail the check of each piece.
//!
//! These tests have no Rule, because no active Rule states that a page holds a
//! record when more records match.

#[path = "support/search_repository.rs"]
mod search_repository;

use search_repository::{
    all_pages, ids, page, plain_sources, repository, requirement, requirement_node, search_json,
    source, source_node, without_stamp,
};
use serde_json::Value;

/// The complete first page of one search, with the stamp and the cursor as
/// null, and whether the page has a cursor.
fn first_page(repo: &str, args: &[&str]) -> (Value, bool) {
    let answer = search_json(repo, args);
    let has_cursor = answer["next_cursor"].is_string();
    (without_stamp(&answer), has_cursor)
}

fn version(repo: &str) -> Value {
    let answer = search_json(repo, &["--kind", "source", "--limit", "1"]);
    answer["protocol_version"].clone()
}

#[test]
fn kind_filter_page_holds_the_matches_after_520_records_of_that_kind() {
    let directory = tempfile::tempdir().unwrap();
    let mut requirements = (0..520)
        .map(|index| {
            requirement(
                &format!("req_a{index:03}"),
                &format!("Plain statement {index:03}."),
            )
        })
        .collect::<Vec<_>>();
    requirements.push(requirement("req_needle_one", "The needle holds one."));
    requirements.push(requirement("req_needle_two", "The needle holds two."));
    let sources = vec![source("source_needle", "Needle record")];
    let repo = repository(directory.path(), sources, requirements);

    let (answer, has_cursor) = first_page(&repo, &["--kind", "requirement", "--text", "needle"]);
    assert!(!has_cursor);
    assert_eq!(
        answer,
        page(
            &version(&repo),
            50,
            false,
            vec![
                requirement_node("req_needle_one", "The needle holds one."),
                requirement_node("req_needle_two", "The needle holds two."),
            ]
        )
    );
}

#[test]
fn page_holds_the_second_kind_matches_after_513_first_kind_records() {
    let directory = tempfile::tempdir().unwrap();
    let requirements = vec![
        requirement("req_needle_one", "The needle holds one."),
        requirement("req_needle_two", "The needle holds two."),
    ];
    let repo = repository(directory.path(), plain_sources(513), requirements);

    let (answer, has_cursor) = first_page(&repo, &["--text", "needle"]);
    assert!(!has_cursor);
    assert_eq!(
        answer,
        page(
            &version(&repo),
            50,
            false,
            vec![
                requirement_node("req_needle_one", "The needle holds one."),
                requirement_node("req_needle_two", "The needle holds two."),
            ]
        )
    );
}

#[test]
fn matches_over_many_pages_come_back_once_each_in_kind_and_id_order() {
    let directory = tempfile::tempdir().unwrap();
    let mut sources = plain_sources(520);
    let mut expected = Vec::new();
    for index in 0..120 {
        let id = format!("source_b{index:03}");
        sources.push(source(&id, &format!("Needle {index:03}")));
        expected.push(id);
        sources.push(source(&format!("source_c{index:03}"), "Plain"));
    }
    let mut requirements = Vec::new();
    for index in 0..3 {
        let id = format!("req_needle_{index}");
        requirements.push(requirement(&id, "The needle holds."));
        expected.push(id);
    }
    let repo = repository(directory.path(), sources, requirements);

    let pages = all_pages(&repo, &["--text", "needle", "--limit", "50"]);
    assert_eq!(ids(&pages), expected);
    let sizes = pages
        .iter()
        .map(|page| page["nodes"].as_array().unwrap().len())
        .collect::<Vec<_>>();
    assert_eq!(sizes, [50, 50, 23]);
}

#[test]
fn upper_case_needle_finds_the_lower_case_text_after_513_records() {
    let directory = tempfile::tempdir().unwrap();
    let mut sources = plain_sources(513);
    sources.push(source("source_needle", "Needle record"));
    let repo = repository(directory.path(), sources, Vec::new());

    let (answer, has_cursor) = first_page(&repo, &["--text", "NEEDLE Rec"]);
    assert!(!has_cursor);
    assert_eq!(
        answer,
        page(
            &version(&repo),
            50,
            false,
            vec![source_node("source_needle", "Needle record")]
        )
    );
}

#[test]
fn percent_and_underscore_in_the_text_are_plain_characters() {
    let directory = tempfile::tempdir().unwrap();
    let mut sources = plain_sources(513);
    sources.push(source("source_rate", "Rate 100% a_b"));
    sources.push(source("source_rate_like", "Rate 100x axb"));
    let repo = repository(directory.path(), sources, Vec::new());

    for text in ["100% a_b", "% a_"] {
        let (answer, has_cursor) = first_page(&repo, &["--text", text]);
        assert!(!has_cursor, "{text}");
        assert_eq!(
            answer,
            page(
                &version(&repo),
                50,
                false,
                vec![source_node("source_rate", "Rate 100% a_b")]
            ),
            "{text}"
        );
    }
}

/// A characterization test: it pins the pages of a search with no text, which
/// the SQL test of the search text keeps.
#[test]
fn search_with_no_text_reads_each_record_of_the_kind_in_id_order() {
    let directory = tempfile::tempdir().unwrap();
    let mut sources = plain_sources(513);
    sources.push(source("source_needle", "Needle record"));
    let repo = repository(directory.path(), sources, Vec::new());

    let (answer, has_cursor) = first_page(&repo, &["--kind", "source"]);
    assert!(has_cursor);
    let nodes = (0..50)
        .map(|index| source_node(&format!("source_a{index:03}"), &format!("Plain {index:03}")))
        .collect();
    assert_eq!(answer, page(&version(&repo), 50, true, nodes));

    let pages = all_pages(&repo, &["--kind", "source", "--limit", "200"]);
    let mut expected = (0..513)
        .map(|index| format!("source_a{index:03}"))
        .collect::<Vec<_>>();
    expected.push("source_needle".to_owned());
    assert_eq!(ids(&pages), expected);
    assert_eq!(pages.len(), 3);
}

/// A characterization test: it pins that a text across two pieces finds no
/// record, which the SQL test of the search text keeps.
#[test]
fn text_across_two_pieces_finds_no_record() {
    let directory = tempfile::tempdir().unwrap();
    let repo = repository(directory.path(), plain_sources(3), Vec::new());

    let (answer, has_cursor) = first_page(&repo, &["--text", "a000\u{1}plain"]);
    assert!(!has_cursor);
    assert_eq!(answer, page(&version(&repo), 50, false, Vec::new()));
}

/// A characterization test: it pins an empty first page with a continuation
/// when 512 candidates fail the check of each piece, which the SQL test of the
/// search text keeps.
#[test]
fn candidates_that_fail_the_piece_check_keep_a_continuation() {
    let directory = tempfile::tempdir().unwrap();
    let mut sources = plain_sources(513);
    sources.push(source("source_needle", "Needle record"));
    let repo = repository(directory.path(), sources, Vec::new());

    // Each plain Source holds this text only across its id and its name.
    let pages = all_pages(&repo, &["--text", "\u{1}plain"]);
    let version = version(&repo);
    assert_eq!(
        pages.iter().map(without_stamp).collect::<Vec<_>>(),
        [
            page(&version, 50, true, Vec::new()),
            page(&version, 50, false, Vec::new()),
        ]
    );
}
