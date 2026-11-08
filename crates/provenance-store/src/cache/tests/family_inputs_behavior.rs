//! Each family names every directory that its reader reads, so a catch-up
//! that reads again only the families of the changed directories misses no
//! change.

use super::super::materialize::{affected, input_directories};
use super::super::*;
use super::fixtures::open;
use super::projection_digest_sensitivity::aggregate_layout;
use crate::publication::with_publication_guard;
use crate::state_store::StateStore;
use std::collections::BTreeSet;

/// Implementation aid: guards the input table of the changed-file catch-up.
/// No Rule binds it.
#[test]
fn each_family_reads_only_inside_its_named_directories() {
    let (_dir, layout, scope) = aggregate_layout();
    let repository = open(&layout);
    let scope_dir = format!("{}/scopes/{}/", layout.state_dir(), scope.as_str());
    let mut read_any = BTreeSet::new();
    for family in ProjectionFamily::ALL {
        crate::test_probes::start_recording_reads();
        with_publication_guard(&repository, |guard| {
            family.records_of_validated_scope(&StateStore::under_guard(guard), &scope)
        })
        .unwrap();
        let reads = crate::test_probes::take_recorded_reads();
        assert!(
            !reads.is_empty(),
            "`{}` records no read",
            family.family_name()
        );
        for path in reads {
            let below = path
                .strip_prefix(&scope_dir)
                .unwrap_or_else(|| panic!("`{}` read {path}", family.family_name()));
            let directory = below.split_once('/').map_or(below, |(first, _)| first);
            assert!(
                input_directories(family).contains(&directory),
                "`{}` read {path} outside {:?}",
                family.family_name(),
                input_directories(family)
            );
            read_any.insert(directory.to_owned());
        }
    }
    assert!(read_any.contains("ideation") && read_any.contains("threads"));
}

/// Implementation aid: the families that a changed path selects. No Rule
/// binds it.
#[test]
fn a_changed_file_selects_the_families_that_read_its_directory() {
    let scope = provenance_core::ScopeId::new("default").unwrap();
    let changed = |paths: &[&str]| -> BTreeSet<String> {
        paths.iter().map(|path| (*path).to_owned()).collect()
    };
    let names = |families: Option<Vec<ProjectionFamily>>| {
        families.map(|families| {
            families
                .into_iter()
                .map(ProjectionFamily::family_name)
                .collect::<Vec<_>>()
        })
    };
    assert_eq!(
        names(affected(
            &scope,
            &changed(&["scopes/default/topics/topic.jsonl"])
        )),
        Some(vec!["topics", "discussions"])
    );
    assert_eq!(
        names(affected(
            &scope,
            &changed(&["scopes/default/ideation/landings.jsonl"])
        )),
        Some(vec![
            "contributions",
            "synthesis_packets",
            "proposal_cards",
            "assertion_records",
            "dispositions",
            "withdrawals"
        ])
    );
    for unknown in [
        "scopes/default/notes/one.jsonl",
        "scopes/default/loose.jsonl",
        "scopes/other/topics/topic.jsonl",
    ] {
        assert_eq!(
            names(affected(&scope, &changed(&[unknown]))),
            None,
            "{unknown}"
        );
    }
}
