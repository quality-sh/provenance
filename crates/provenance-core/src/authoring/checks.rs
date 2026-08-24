//! Document-decidable structural checks.
//!
//! These are the engine-side checks that gate the wire. The store calls
//! them per declaration at its existing pipeline points, so a document
//! with more than one defect keeps today's first error. Identity
//! resolution stays in the store; nothing here reads repository state.

use std::collections::BTreeSet;

use super::addresses::rule_address;
use crate::model::DeclarationAddress;
use crate::protocol::{TypedRequirementInput, TypedRuleInput};

/// Admits source or requirement declarations one at a time, in document
/// order: a non-empty key, no duplicate key, then the declaration address.
pub struct DeclarationChecker {
    kind: &'static str,
    seen: BTreeSet<String>,
}

impl DeclarationChecker {
    pub const fn new(kind: &'static str) -> Self {
        Self {
            kind,
            seen: BTreeSet::new(),
        }
    }

    pub fn admit(&mut self, spec: &str, key: &str) -> anyhow::Result<DeclarationAddress> {
        let kind = self.kind;
        anyhow::ensure!(!key.trim().is_empty(), "{kind} key must not be empty");
        anyhow::ensure!(
            self.seen.insert(key.to_string()),
            "duplicate {kind} key `{key}`"
        );
        DeclarationAddress::new([spec, kind, key])
    }
}

/// Admits rule declarations one at a time, in document order: a non-empty
/// key, a legal address no other declaration resolved to, and no repeated
/// (requirement, key) relationship.
#[derive(Default)]
pub struct RuleChecker {
    addresses: BTreeSet<DeclarationAddress>,
    relationships: BTreeSet<(String, String)>,
}

impl RuleChecker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn admit(
        &mut self,
        spec: &str,
        declaration: &TypedRuleInput,
    ) -> anyhow::Result<DeclarationAddress> {
        anyhow::ensure!(
            !declaration.key.trim().is_empty(),
            "rule key must not be empty"
        );
        let address = rule_address(spec, declaration)?;
        anyhow::ensure!(
            !self.addresses.contains(&address),
            "distinct rule declarations resolve to address `{}`",
            address.segments().join("/")
        );
        for requirement in &declaration.requirements {
            anyhow::ensure!(
                self.relationships
                    .insert((requirement.clone(), declaration.key.clone())),
                "distinct rule declarations with key `{}` collide under requirement `{requirement}`",
                declaration.key
            );
        }
        self.addresses.insert(address.clone());
        Ok(address)
    }
}

/// Folds the legacy singular `requirement` field into `requirements`,
/// refuses empty or repeated requirement keys, and leaves the list in
/// `BTreeSet` order.
pub fn normalize_rule_relationships(declarations: &mut [TypedRuleInput]) -> anyhow::Result<()> {
    for declaration in declarations {
        anyhow::ensure!(
            declaration.requirement.is_none() || declaration.requirements.is_empty(),
            "rule `{}` cannot set both `requirement` and `requirements`",
            declaration.key
        );
        if let Some(requirement) = declaration.requirement.take() {
            declaration.requirements.push(requirement);
        }
        anyhow::ensure!(
            !declaration.requirements.is_empty(),
            "rule `{}` must refine at least one requirement",
            declaration.key
        );
        let mut unique = BTreeSet::new();
        for requirement in &declaration.requirements {
            anyhow::ensure!(
                !requirement.trim().is_empty(),
                "rule `{}` has an empty requirement key",
                declaration.key
            );
            anyhow::ensure!(
                unique.insert(requirement.clone()),
                "rule `{}` repeats requirement `{requirement}`",
                declaration.key
            );
        }
        declaration.requirements = unique.into_iter().collect();
    }
    Ok(())
}

/// Refuses references to sources or requirements the document does not
/// declare. This reads only the document's own declared key sets.
pub fn validate_references(
    requirements: &[TypedRequirementInput],
    rules: &[TypedRuleInput],
    source_declared: impl Fn(&str) -> bool,
    requirement_declared: impl Fn(&str) -> bool,
) -> anyhow::Result<()> {
    for requirement in requirements {
        for source in &requirement.sources {
            anyhow::ensure!(
                source_declared(source),
                "requirement `{}` references undeclared source `{source}`",
                requirement.key
            );
        }
    }
    for rule in rules {
        for requirement in &rule.requirements {
            anyhow::ensure!(
                requirement_declared(requirement),
                "rule `{}` references undeclared requirement `{requirement}`",
                rule.key
            );
        }
    }
    Ok(())
}
