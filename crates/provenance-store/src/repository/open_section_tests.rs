//! The reads of `Repository::open` in the publication section.

use super::Repository;
use crate::cache::tests::fixtures::seeded_layout;
use crate::test_probes;
use provenance_macros::verifies;

/// The probe at the read of the settings file asks whether the publication
/// lock is held while the open runs. The open holds the section, so the
/// lock is held, and a write of a different process cannot replace a state
/// file during the read.
#[test]
#[verifies("rule_open_reads_settings_in_the_section", examples)]
fn open_reads_the_settings_and_the_dictionary_in_the_publication_section() {
    let (_directory, layout, _scope) = seeded_layout();
    let held = std::rc::Rc::new(std::cell::Cell::new(None));
    test_probes::arm("open_settings_read", {
        let held = held.clone();
        let layout = layout.clone();
        move || {
            held.set(Some(test_probes::publication_lock_is_held(&layout)));
            Ok(())
        }
    });

    let opened = Repository::open(layout.root(), "default");
    test_probes::disarm("open_settings_read");

    opened.unwrap();
    assert_eq!(
        held.get(),
        Some(true),
        "the open read the settings in the section"
    );
}
