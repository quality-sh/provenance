use crate::cli::Cli;
use clap::{CommandFactory as _, Parser as _};
use provenance_core::RESERVED_RECORD_IDS;
use provenance_porcelain::{action::Action, get::View};
use provenance_porcelain::discussion::DiscussionAction;
use provenance_store::operations::catalog::{self, TargetAction};

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
    for action in TargetAction::ALL {
        assert!(RESERVED_RECORD_IDS.contains(&action.as_str()));
    }
    for action in DiscussionAction::ALL {
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
