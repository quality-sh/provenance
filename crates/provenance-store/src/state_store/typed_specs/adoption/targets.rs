use super::{CurrentTypedState, TypedSpecInput};
use provenance_core::{
    protocol::{TypedAdoptionTarget, TypedDeclarationKind},
    StableId,
};
use std::collections::BTreeSet;

pub(super) fn validate_targets(
    input: &TypedSpecInput,
    current: &CurrentTypedState,
) -> anyhow::Result<BTreeSet<TypedAdoptionTarget>> {
    let mut targets = BTreeSet::new();
    for target in &input.adopt_unowned {
        StableId::new(&target.id).map_err(|_| {
            anyhow::anyhow!(
                "adoption target id `{}` must use lowercase ASCII letters, digits, '_' or '-'",
                target.id
            )
        })?;
        anyhow::ensure!(
            targets.insert(target.clone()),
            "duplicate adoption target `{}:{}`",
            kind_name(target.kind),
            target.id
        );
    }
    for target in &input.adopt_unowned {
        let (declarations, exact) = match target.kind {
            TypedDeclarationKind::Source => (
                input.sources.len(),
                input
                    .sources
                    .iter()
                    .filter(|value| value.id.as_deref() == Some(&target.id))
                    .count(),
            ),
            TypedDeclarationKind::Requirement => (
                input.requirements.len(),
                input
                    .requirements
                    .iter()
                    .filter(|value| value.id.as_deref() == Some(&target.id))
                    .count(),
            ),
            TypedDeclarationKind::Rule => (
                input.rules.len(),
                input
                    .rules
                    .iter()
                    .filter(|value| value.id.as_deref() == Some(&target.id))
                    .count(),
            ),
        };
        anyhow::ensure!(
            declarations > 0,
            "adoption target `{}:{}` does not name a declaration in this document",
            kind_name(target.kind),
            target.id
        );
        anyhow::ensure!(
            exact == 1,
            "adoption target `{}:{}` must name exactly one declaration with the same explicit id",
            kind_name(target.kind),
            target.id
        );
    }
    for target in &input.adopt_unowned {
        let exists = match target.kind {
            TypedDeclarationKind::Source => current
                .sources
                .iter()
                .any(|value| value.id.as_str() == target.id),
            TypedDeclarationKind::Requirement => current
                .requirements
                .iter()
                .any(|value| value.id.as_str() == target.id),
            TypedDeclarationKind::Rule => current
                .rules
                .iter()
                .any(|value| value.id.as_str() == target.id),
        };
        anyhow::ensure!(
            exists,
            "adoption target `{}:{}` does not exist in canonical state",
            kind_name(target.kind),
            target.id
        );
    }
    Ok(targets)
}

const fn kind_name(kind: TypedDeclarationKind) -> &'static str {
    match kind {
        TypedDeclarationKind::Source => "source",
        TypedDeclarationKind::Requirement => "requirement",
        TypedDeclarationKind::Rule => "rule",
    }
}
