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
    for (plural, _) in REVIEW_KINDS {
        for (method, suffix) in [
            (HttpMethod::Post, "/{id}/submit"),
            (HttpMethod::Post, "/{id}/submissions/{proposal_id}/decide"),
            (HttpMethod::Post, "/{id}/submissions/{proposal_id}/withdraw"),
            (HttpMethod::Get, "/{id}/history"),
            (HttpMethod::Get, "/{id}/history/{entry_id}"),
            (HttpMethod::Get, "/{id}/history/{entry_id}/evidence/{side}"),
        ] {
            let expected = format!("/{plural}{suffix}");
            assert!(
                routes
                    .iter()
                    .any(|route| route.method == method && route.path == expected),
                "missing {method:?} {expected}"
            );
        }
    }
}

#[cfg(feature = "schema")]
#[test]
fn every_review_kind_member_read_carries_edit_and_decision_state() {
    let routes = definitions();
    for (plural, _) in REVIEW_KINDS {
        let path = format!("/{plural}/{{id}}");
        let route = routes
            .iter()
            .find(|route| route.method == HttpMethod::Get && route.path == path)
            .unwrap();
        assert_eq!(
            route.registration.handler.operation,
            "get-reviewed-resource"
        );
        let schema = &route.registration.response.schema;
        let mut data = &schema["properties"]["data"];
        if let Some(reference) = data["$ref"].as_str() {
            let name = reference.strip_prefix("#/$defs/").unwrap();
            data = &schema["$defs"][name];
        }
        for field in ["edit", "decision"] {
            assert!(
                data["properties"].get(field).is_some(),
                "{path} omits {field}: {schema}"
            );
        }
    }
}

#[test]
fn review_actions_do_not_accept_idempotency_headers() {
    for name in [
        "submit-record-review",
        "decide-record-review",
        "withdraw-record-review",
    ] {
        let definitions = definitions()
            .iter()
            .filter(|entry| entry.registration.handler.operation == name)
            .collect::<Vec<_>>();
        assert_eq!(definitions.len(), 8, "{name} route count");
        for definition in definitions {
            assert!(
                definition.registration.controls.headers.is_empty(),
                "{} exposes a client request identity",
                definition.name
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
fn each_review_route_binds_its_record_kind() {
    let routes = definitions();
    for (plural, singular) in REVIEW_KINDS {
        for suffix in [
            "/{id}/submit",
            "/{id}/submissions/{proposal_id}/decide",
            "/{id}/submissions/{proposal_id}/withdraw",
            "/{id}/history",
            "/{id}/history/{entry_id}",
            "/{id}/history/{entry_id}/evidence/{side}",
        ] {
            let path = format!("/{plural}{suffix}");
            let route = routes.iter().find(|route| route.path == path).unwrap();
            assert!(route
                .registration
                .request
                .fixed
                .iter()
                .any(|binding| binding.field == "record_kind" && binding.value == singular));
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
