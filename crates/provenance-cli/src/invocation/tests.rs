use crate::cli::Cli;
use clap::{CommandFactory as _, Parser as _};
use provenance_core::RESERVED_RECORD_IDS;
use provenance_porcelain::{action::Action, get::View};
use provenance_store::operations::catalog;

#[test]
fn reserved_ids_cover_declared_root_commands_collections_and_actions() {
    for command in Cli::command().get_subcommands() {
        assert!(
            RESERVED_RECORD_IDS.contains(&command.get_name()),
            "root command {} is not reserved",
            command.get_name()
        );
    }
    assert!(RESERVED_RECORD_IDS.contains(&"search"));
    assert!(RESERVED_RECORD_IDS.contains(&"get"));
    for action in Action::ALL {
        assert!(RESERVED_RECORD_IDS.contains(&action.as_str()));
    }
    for definition in catalog::definitions() {
        let collection = definition.path.split('/').nth(1).unwrap();
        assert!(
            RESERVED_RECORD_IDS.contains(&collection),
            "{collection} is not reserved"
        );
    }
}

#[test]
fn discussion_flags_follow_the_typed_input_contracts() {
    let command = crate::catalog_cli::target_command().unwrap();
    let declared = command
        .get_arguments()
        .filter_map(|argument| argument.get_long())
        .collect::<std::collections::BTreeSet<_>>();
    for action in Action::DISCUSSION {
        let schema = provenance_porcelain::discussion::input_schema(action);
        for field in schema["properties"].as_object().unwrap().keys() {
            if ["parent", "discussion_id", "declared_by"].contains(&field.as_str()) {
                continue;
            }
            let flag = field.replace('_', "-");
            assert!(
                declared.contains(flag.as_str()),
                "{action:?} lacks --{flag}"
            );
        }
    }
    let root = super::grammar::discussions_command().unwrap();
    let root_flags = root
        .get_arguments()
        .filter_map(|argument| argument.get_long())
        .collect::<std::collections::BTreeSet<_>>();
    for action in [Action::Discussions, Action::Discussion] {
        let schema = provenance_porcelain::discussion::input_schema(action);
        for field in schema["properties"].as_object().unwrap().keys() {
            if ["parent", "discussion_id"].contains(&field.as_str()) {
                continue;
            }
            let flag = field.replace('_', "-");
            assert!(root_flags.contains(flag.as_str()), "root lacks --{flag}");
        }
    }
}

#[test]
fn target_grammar_uses_shared_action_and_view_values() {
    for action in Action::ALL {
        assert!(
            super::grammar::TargetArgs::try_parse_from(["provenance", "req_live", action.as_str()])
                .is_ok(),
            "{}",
            action.as_str()
        );
    }
    for view in View::ALL {
        assert!(
            super::grammar::TargetArgs::try_parse_from([
                "provenance",
                "req_live",
                "get",
                "--view",
                view.as_str()
            ])
            .is_ok(),
            "{}",
            view.as_str()
        );
    }
    assert!(
        super::grammar::TargetArgs::try_parse_from(["provenance", "req_live", "invented"]).is_err()
    );
    assert!(super::grammar::TargetArgs::try_parse_from([
        "provenance",
        "req_live",
        "get",
        "--view",
        "invented"
    ])
    .is_err());
}
