use std::collections::BTreeSet;

use super::super::{definitions, HttpMethod};

const REVIEW_KINDS: [(&str, &str); 8] = [
    ("sources", "source"),
    ("requirements", "requirement"),
    ("resolutions", "resolution"),
    ("rules", "rule"),
    ("domains", "domain"),
    ("boundaries", "boundary"),
    ("topics", "topic"),
    ("questions", "question"),
];

#[test]
fn every_review_kind_has_actions_history_and_evidence() {
    let routes = definitions();
    let paths = routes
        .iter()
        .map(|route| (route.method, route.path))
        .collect::<BTreeSet<_>>();

    for (plural, _) in REVIEW_KINDS {
        for (method, suffix) in [
            (HttpMethod::Post, "/{id}/submit"),
            (
                HttpMethod::Post,
                "/{id}/submissions/{proposal_id}/decide",
            ),
            (
                HttpMethod::Post,
                "/{id}/submissions/{proposal_id}/withdraw",
            ),
            (HttpMethod::Get, "/{id}/history"),
            (HttpMethod::Get, "/{id}/history/{entry_id}"),
            (
                HttpMethod::Get,
                "/{id}/history/{entry_id}/evidence/{side}",
            ),
        ] {
            let expected = format!("/{plural}{suffix}");
            assert!(
                paths.contains(&(method, expected.as_str())),
                "missing {method:?} {expected}"
            );
        }
    }
}

#[test]
fn generated_review_operation_ids_are_kind_specific() {
    let routes = definitions();
    for (_, singular) in REVIEW_KINDS {
        let title = title_case(singular);
        for expected in [
            format!("submit{title}Review"),
            format!("decide{title}Review"),
            format!("withdraw{title}Review"),
            format!("list{title}History"),
            format!("get{title}HistoryEntry"),
            format!("get{title}HistoryEvidence"),
        ] {
            assert!(
                routes.iter().any(|route| route.operation_id == expected),
                "missing generated {title} operation {expected}"
            );
        }
    }
}

#[test]
fn only_discussion_parent_kinds_have_discussion_routes() {
    let routes = definitions();
    for (plural, _) in REVIEW_KINDS {
        let path = format!("/{plural}/{{id}}/discussions");
        let count = routes.iter().filter(|route| route.path == path).count();
        let expected = usize::from(!matches!(plural, "domains" | "boundaries")) * 2;
        assert_eq!(count, expected, "unexpected Discussion routes for {plural}");
    }
}

fn title_case(value: &str) -> String {
    let mut chars = value.chars();
    chars
        .next()
        .map(char::to_uppercase)
        .into_iter()
        .flatten()
        .chain(chars)
        .collect()
}
