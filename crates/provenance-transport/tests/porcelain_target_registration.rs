use provenance_store::operations::catalog::{self, TargetAction};
use std::collections::BTreeSet;

#[test]
fn target_actions_are_declared_once_by_canonical_registrations() {
    let actual = catalog::definitions()
        .iter()
        .filter_map(|definition| {
            definition.registration.target.map(|target| {
                (
                    target.action.as_str(),
                    target.kind.as_str(),
                    definition.name,
                )
            })
        })
        .collect::<BTreeSet<_>>();
    let mut expected = BTreeSet::new();
    for kind in [
        "source",
        "requirement",
        "resolution",
        "rule",
        "topic",
        "question",
        "domain",
        "boundary",
    ] {
        expected.insert(("create", kind, format!("create-{kind}")));
        expected.insert(("update", kind, format!("update-{kind}")));
    }
    expected.extend([
        ("answer", "question", "answer-question".to_owned()),
        ("claim", "topic", "claim-topic".to_owned()),
        ("release", "topic", "release-topic".to_owned()),
        (
            "submit",
            "requirement",
            "submit-requirement-review".to_owned(),
        ),
    ]);
    let actual = actual
        .into_iter()
        .map(|(action, kind, name)| (action, kind, name.to_owned()))
        .collect::<BTreeSet<_>>();
    assert_eq!(actual, expected);

    for action in TargetAction::ALL {
        assert!(actual.iter().any(|entry| entry.0 == action.as_str()));
    }
}

#[test]
fn each_target_action_and_kind_resolves_to_one_registration() {
    for action in TargetAction::ALL {
        for kind in provenance_core::NodeType::ALL {
            let matches = catalog::definitions()
                .iter()
                .filter(|definition| {
                    definition
                        .registration
                        .target
                        .is_some_and(|target| target.action == action && target.kind == kind)
                })
                .count();
            assert!(matches <= 1, "{} {}", action.as_str(), kind.as_str());
        }
    }
}
