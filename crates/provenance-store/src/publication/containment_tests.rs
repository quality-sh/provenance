//! Property verification for `rule_recovery_stays_in_cache`.
//!
//! The decision is filesystem-bound: it turns on what `canonicalize` and the
//! symlink probes report, so there is no finite input domain to exhaust.
//! Instead these tests build real directory layouts under a temporary
//! directory, planting a symlink, a regular file or nothing at each depth
//! below the cache in turn, and check the rule against an independent
//! restatement of the decision.

use super::*;
use camino::Utf8Component;
use provenance_macros::verifies;

/// What is planted at one step of the path below the cache.
#[derive(Clone, Copy, Debug)]
enum Planted {
    RealDirectory,
    LinkOutOfRepository,
    LinkIntoWorkingTree,
    LinkElsewhereInCache,
    /// A plain file where a directory is expected: it sits exactly where its
    /// name says, so only the kind check can refuse it.
    RegularFile,
    Absent,
}

const PLANTINGS: [Planted; 6] = [
    Planted::RealDirectory,
    Planted::LinkOutOfRepository,
    Planted::LinkIntoWorkingTree,
    Planted::LinkElsewhereInCache,
    Planted::RegularFile,
    Planted::Absent,
];

struct Layouts {
    _directory: tempfile::TempDir,
    layout: ProvenanceLayout,
    /// The cache directory, already canonical: every component of the
    /// temporary root is resolved when the fixture is built.
    cache: Utf8PathBuf,
}

impl Layouts {
    /// Builds a repository with `transactions` planted at
    /// `.provenance/cache/import-transactions` and `leaf` planted at `t`
    /// inside whatever that resolves to.
    fn build(transactions: Planted, leaf: Planted) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let base = canonical(directory.path());
        let repository = base.join("repository");
        let layout = ProvenanceLayout::new(repository.clone());
        for path in [
            base.join("external/transactions"),
            base.join("external/leaf"),
            repository.join("tracked/transactions"),
            repository.join("tracked/leaf"),
            // Where `<transactions>/../../..` lands, so the traversal shape
            // below names something that really exists.
            repository.join("outside/target"),
            layout.cache_dir().join("spare"),
        ] {
            std::fs::create_dir_all(path).unwrap();
        }

        let transactions_dir = layout.import_transactions_dir();
        plant(
            transactions,
            &transactions_dir,
            &base.join("external/transactions"),
            &repository.join("tracked/transactions"),
            &layout.cache_dir().join("spare"),
        );
        if !matches!(transactions, Planted::Absent | Planted::RegularFile) {
            std::fs::create_dir_all(transactions_dir.join("sibling")).unwrap();
            std::fs::create_dir_all(transactions_dir.join("step")).unwrap();
            plant(
                leaf,
                &transactions_dir.join("t"),
                &base.join("external/leaf"),
                &repository.join("tracked/leaf"),
                &transactions_dir.join("sibling"),
            );
            if matches!(leaf, Planted::RealDirectory) {
                std::fs::create_dir_all(transactions_dir.join("t/nested")).unwrap();
            }
        }

        let cache = layout.cache_dir();
        Self {
            _directory: directory,
            layout,
            cache,
        }
    }

    /// Path shapes recovery could be handed, named by what they spell.
    fn candidates(&self, leaf_name: &str) -> Vec<(String, Utf8PathBuf)> {
        let transactions = self.layout.import_transactions_dir();
        [
            leaf_name.to_string(),
            format!("./{leaf_name}"),
            format!("step/../{leaf_name}"),
            "../../../outside/target".to_string(),
            "..".to_string(),
            format!("t/{leaf_name}"),
        ]
        .into_iter()
        .map(|shape| (shape.clone(), transactions.join(shape)))
        .collect()
    }
}

fn plant(
    planted: Planted,
    path: &Utf8Path,
    out_of_repository: &Utf8Path,
    working_tree: &Utf8Path,
    elsewhere_in_cache: &Utf8Path,
) {
    let target = match planted {
        Planted::RealDirectory => {
            std::fs::create_dir_all(path).unwrap();
            return;
        }
        Planted::RegularFile => {
            std::fs::write(path, "not a directory").unwrap();
            return;
        }
        Planted::Absent => return,
        Planted::LinkOutOfRepository => out_of_repository,
        Planted::LinkIntoWorkingTree => working_tree,
        Planted::LinkElsewhereInCache => elsewhere_in_cache,
    };
    std::os::unix::fs::symlink(target, path).unwrap();
}

fn canonical(path: &std::path::Path) -> Utf8PathBuf {
    Utf8PathBuf::from_path_buf(dunce::canonicalize(path).unwrap()).unwrap()
}

/// Independent restatement of the decision, used as the oracle below: walk the
/// candidate as written from the canonical cache, requiring every step to be a
/// real directory, and require those steps to spell exactly one name inside
/// `import-transactions`. Deliberately implemented by probing each step rather
/// than by resolving the whole path, so that it does not repeat the rule.
///
/// `probe_leaf` is false when recovery only clears a marker for a directory
/// that is already gone: there is nothing on disk to probe at the last step.
fn every_step_is_a_real_directory_in_the_cache(
    canonical_cache: &Utf8Path,
    candidate: &Utf8Path,
    probe_leaf: bool,
) -> bool {
    let Ok(relative) = candidate.strip_prefix(canonical_cache) else {
        return false;
    };
    let steps: Vec<Utf8Component> = relative.components().collect();
    let mut written = canonical_cache.to_path_buf();
    let mut spelled = Utf8PathBuf::new();
    for (index, step) in steps.iter().enumerate() {
        written.push(step.as_str());
        if probe_leaf || index + 1 < steps.len() {
            match std::fs::symlink_metadata(&written) {
                Ok(metadata) if metadata.is_dir() && !metadata.is_symlink() => {}
                _ => return false,
            }
        } else {
            // The leaf may be gone, but a symlink lingering there is still a
            // symlink component of the written path.
            if std::fs::symlink_metadata(&written).is_ok_and(|metadata| metadata.is_symlink()) {
                return false;
            }
        }
        match step {
            Utf8Component::CurDir => {}
            Utf8Component::ParentDir => {
                if !spelled.pop() {
                    return false;
                }
            }
            Utf8Component::Normal(name) => spelled.push(name),
            Utf8Component::Prefix(_) | Utf8Component::RootDir => return false,
        }
    }
    spelled.parent() == Some(Utf8Path::new("import-transactions"))
}

#[test]
#[verifies("rule_recovery_stays_in_cache", property)]
fn recovery_touches_only_real_directories_inside_the_cache() {
    for transactions in PLANTINGS {
        for leaf in PLANTINGS {
            let layouts = Layouts::build(transactions, leaf);
            let contained = layouts.cache.join("import-transactions");
            for (shape, candidate) in layouts.candidates("t") {
                let decision = validated_transaction_dir(&layouts.layout, &candidate);
                assert_eq!(
                    decision.is_ok(),
                    every_step_is_a_real_directory_in_the_cache(&layouts.cache, &candidate, true),
                    "transactions {transactions:?}, leaf {leaf:?}, shape {shape}: \
                     the rule answered {decision:?}"
                );
                if let Ok(resolved) = decision {
                    assert_eq!(
                        resolved.parent(),
                        Some(contained.as_path()),
                        "the rule returned a path outside the transactions directory"
                    );
                }
            }
        }
    }
}

#[test]
#[verifies("rule_recovery_stays_in_cache", property)]
fn clearing_a_marker_checks_the_place_the_directory_claimed() {
    for transactions in PLANTINGS {
        for leaf in PLANTINGS {
            let layouts = Layouts::build(transactions, leaf);
            for (shape, candidate) in layouts.candidates("t") {
                let decision = validate_missing_transaction_dir(&layouts.layout, &candidate);
                assert_eq!(
                    decision.is_ok(),
                    every_step_is_a_real_directory_in_the_cache(&layouts.cache, &candidate, false),
                    "transactions {transactions:?}, leaf {leaf:?}, shape {shape}: \
                     the rule answered {decision:?}"
                );
            }
        }
    }
}

#[test]
#[verifies("rule_recovery_stays_in_cache", property)]
fn every_refusal_keeps_the_marker_message() {
    for transactions in PLANTINGS {
        for leaf in PLANTINGS {
            let layouts = Layouts::build(transactions, leaf);
            for (shape, candidate) in layouts.candidates("t") {
                let Err(error) = validated_transaction_dir(&layouts.layout, &candidate) else {
                    continue;
                };
                let message = error.to_string();
                assert!(
                    message.contains("outside the repository cache")
                        || message.contains("is not a directory")
                        || message.contains("symlink component")
                        || error.downcast_ref::<std::io::Error>().is_some(),
                    "transactions {transactions:?}, leaf {leaf:?}, shape {shape}: {message}"
                );
            }
        }
    }
}
