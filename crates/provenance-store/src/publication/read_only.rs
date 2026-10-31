use crate::layout::ProvenanceLayout;
use crate::state_store::StateStore;
use provenance_core::{NodeType, ScopeId};
use provenance_macros::rule;

/// Runs `operation` with a read-only store over `layout`. The store takes no
/// lock, runs no recovery, creates no path and refuses record writes. Live
/// access through it still refuses while a stage body of the repository runs
/// on this thread.
pub fn with_read_only_validation<R>(
    layout: &ProvenanceLayout,
    operation: impl FnOnce(&StateStore) -> anyhow::Result<R>,
) -> anyhow::Result<R> {
    operation(&StateStore::read_only(layout.clone()))
}

/// The record kinds that hold `id` in one scope, read with no lock and no
/// recovery. The caller gets the kinds and not a store, so nothing outside the
/// store crate reads through a read-only store.
///
/// The answer is best-effort while a publication runs: with no lock, the read
/// can see the live state before or after the publication, or no live state
/// while the publication swaps it. The only production caller is the CLI ID
/// help, which uses the answer only to select the help of one record kind.
#[rule("rule_read_only_section_stays_inside_the_store")]
pub fn read_only_record_kinds(
    layout: &ProvenanceLayout,
    scope: &ScopeId,
    id: &str,
) -> anyhow::Result<Vec<NodeType>> {
    with_read_only_validation(layout, |store| {
        let mut kinds = Vec::new();
        for kind in NodeType::ALL {
            if holds(store, scope, kind, id)? {
                kinds.push(kind);
            }
        }
        Ok(kinds)
    })
}

fn holds(store: &StateStore, scope: &ScopeId, kind: NodeType, id: &str) -> anyhow::Result<bool> {
    macro_rules! any_id {
        ($list:ident) => {
            store
                .$list(scope)?
                .iter()
                .any(|record| record.id.as_str() == id)
        };
    }
    Ok(match kind {
        NodeType::Source => any_id!(list_sources),
        NodeType::Requirement => any_id!(list_requirements),
        NodeType::Resolution => any_id!(list_resolutions),
        NodeType::Rule => any_id!(list_rules),
        NodeType::Topic => any_id!(list_topics),
        NodeType::Question => any_id!(list_questions),
        NodeType::Domain => any_id!(list_domains),
        NodeType::Boundary => any_id!(list_boundaries),
    })
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::publication::{guard, staged};
    use camino::Utf8PathBuf;
    use provenance_macros::verifies;

    pub(in crate::publication) fn scope() -> ScopeId {
        ScopeId::new("default").unwrap()
    }

    /// A repository with one Source, and a copy of its state with no cache.
    pub(in crate::publication) fn record_kind_repositories(
    ) -> (tempfile::TempDir, ProvenanceLayout, ProvenanceLayout) {
        let directory = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(directory.path().to_path_buf()).unwrap();
        let live = ProvenanceLayout::new(root.join("live"));
        crate::cache::tests::fixtures::initialized(live.root().as_std_path());
        crate::cache::tests::fixtures::create_source(
            &crate::test_support::store(&live),
            &scope(),
            "source_help",
        );
        let copy = ProvenanceLayout::new(root.join("copy"));
        staged::copy_directory(&live.state_dir(), &copy.state_dir()).unwrap();
        (directory, live, copy)
    }

    #[test]
    #[verifies("rule_read_only_section_stays_inside_the_store", examples)]
    fn read_only_record_kinds_answer_while_another_handle_holds_the_lock() {
        let (_directory, live, _copy) = record_kind_repositories();
        let _held = guard::LockedPublicationFile::acquire(&live.publication_lock_path()).unwrap();
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            sender
                .send(read_only_record_kinds(&live, &scope(), "source_help").unwrap())
                .unwrap();
        });
        let kinds = receiver
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("the read takes no publication lock");
        assert_eq!(kinds, vec![NodeType::Source]);
    }

    #[test]
    #[verifies("rule_read_only_section_stays_inside_the_store", examples)]
    fn read_only_record_kinds_create_no_path() {
        let (_directory, _live, copy) = record_kind_repositories();
        assert_eq!(
            read_only_record_kinds(&copy, &scope(), "source_help").unwrap(),
            vec![NodeType::Source]
        );
        assert_eq!(
            read_only_record_kinds(&copy, &scope(), "source_missing").unwrap(),
            Vec::new()
        );
        assert!(!copy.cache_dir().exists(), "the read creates no cache");
    }

    // Implementation aid: checks alias identity during validation.
    #[cfg(unix)]
    #[test]
    fn read_only_validation_through_an_alias_takes_no_lock() {
        let (directory, _live, copy) = record_kind_repositories();
        let alias = directory.path().join("alias");
        std::os::unix::fs::symlink(copy.root(), &alias).unwrap();
        let alias = ProvenanceLayout::new(alias.to_str().unwrap());
        let manifest = with_read_only_validation(&alias, StateStore::manifest).unwrap();
        assert_eq!(manifest.scopes[0].id, scope());
        assert!(!copy.publication_lock_path().exists());
        assert!(!copy.import_transactions_dir().exists());
    }

    // Implementation aid: init validates planned state before its repository exists.
    #[test]
    fn read_only_validation_accepts_a_planned_repository() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("planned/repository");
        let layout = ProvenanceLayout::new(root.to_str().unwrap());
        let sources = with_read_only_validation(&layout, |store| store.list_sources(&scope()));
        assert_eq!(sources.unwrap(), [] as [provenance_core::Source; 0]);
        assert!(!root.exists());
    }
}
