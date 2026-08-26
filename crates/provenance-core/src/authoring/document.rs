//! Build-time validation and canonical assembly.
//!
//! `build()` runs the full check union: the TS-parity content checks that
//! never gate the wire, then the engine-side structural set the wire also
//! applies. Canonical ordering applies only here, to kernel-authored
//! documents, never to wire-received ones.

use std::collections::BTreeSet;

use provenance_macros::rule;

use super::builders::{RequirementBuilder, RuleBuilder, SourceBuilder, SpecBuilder};
use super::checks::{reference_violations, DeclarationChecker, RuleChecker};
use super::AuthoringError;
use crate::model::{SourceType, StableId, SUPPORTED_SCHEMA_VERSION};
use crate::protocol::{
    TypedAdoptionTarget, TypedDeclarationKind, TypedImplementationInput, TypedRequirementInput,
    TypedRuleInput, TypedSourceInput, TypedSpecInput,
};

/// One frozen, canonical, kernel-authored document.
#[derive(Debug, Clone)]
pub struct SpecDocument {
    pub(super) spec: String,
    pub(super) adopt_unowned: Vec<TypedAdoptionTarget>,
    pub(super) sources: Vec<TypedSourceInput>,
    pub(super) requirements: Vec<TypedRequirementInput>,
    pub(super) rules: Vec<TypedRuleInput>,
}

impl SpecDocument {
    pub fn spec(&self) -> &str {
        &self.spec
    }

    pub fn sources(&self) -> &[TypedSourceInput] {
        &self.sources
    }

    pub fn requirements(&self) -> &[TypedRequirementInput] {
        &self.requirements
    }

    pub fn rules(&self) -> &[TypedRuleInput] {
        &self.rules
    }

    /// Materializes the existing store document. Total: no I/O, and no
    /// failure beyond what `build()` already rejected.
    pub fn materialize(&self, declared_by: impl Into<String>) -> TypedSpecInput {
        TypedSpecInput {
            schema_version: SUPPORTED_SCHEMA_VERSION.0,
            spec: self.spec.clone(),
            declared_by: declared_by.into(),
            adopt_unowned: self.adopt_unowned.clone(),
            sources: self.sources.clone(),
            requirements: self.requirements.clone(),
            rules: self.rules.clone(),
        }
    }
}

/// Validates and freezes one authored document without ambient access,
/// then orders sources and requirements by key and rules by serialized
/// address in UTF-8 byte order.
#[rule("rule_rust_authoring_kernel_is_pure")]
#[rule("rule_rust_authored_documents_are_canonical")]
pub(super) fn build_document(builder: SpecBuilder) -> Result<SpecDocument, AuthoringError> {
    let mut violations = Vec::new();
    let spec = builder.key;
    if spec.trim().is_empty() {
        violations.push("spec key must not be empty".to_string());
    }
    let mut sources: Vec<TypedSourceInput> = Vec::new();
    let mut requirements: Vec<TypedRequirementInput> = Vec::new();
    let mut rules: Vec<TypedRuleInput> = Vec::new();
    let mut adopt_unowned = Vec::new();
    for requirement in builder.requirements {
        collect_requirement(
            requirement,
            &mut violations,
            &mut sources,
            &mut requirements,
            &mut rules,
            &mut adopt_unowned,
        );
    }
    check_structure(&spec, &sources, &requirements, &mut rules, &mut violations);
    if !violations.is_empty() {
        return Err(AuthoringError::new(violations));
    }
    sources.sort_by(|left, right| left.key.cmp(&right.key));
    requirements.sort_by(|left, right| left.key.cmp(&right.key));
    rules.sort_by_key(serialized_address);
    adopt_unowned.sort();
    adopt_unowned.dedup();
    Ok(SpecDocument {
        spec,
        adopt_unowned,
        sources,
        requirements,
        rules,
    })
}

fn collect_requirement(
    requirement: RequirementBuilder,
    violations: &mut Vec<String>,
    sources: &mut Vec<TypedSourceInput>,
    requirements: &mut Vec<TypedRequirementInput>,
    rules: &mut Vec<TypedRuleInput>,
    adopt_unowned: &mut Vec<TypedAdoptionTarget>,
) {
    if text_missing(requirement.statement.as_deref()) {
        violations.push(format!(
            "Requirement `{}` statement must not be empty",
            requirement.key
        ));
    }
    if requirement
        .description
        .as_deref()
        .is_some_and(|description| description.trim().is_empty())
    {
        violations.push("requirement description must not be empty".to_string());
    }
    validate_explicit_id(
        "requirement",
        &requirement.key,
        requirement.explicit_id.as_deref(),
        violations,
    );
    collect_adoption(
        TypedDeclarationKind::Requirement,
        requirement.adopt_unowned,
        requirement.explicit_id.as_deref(),
        adopt_unowned,
    );
    let mut cited = Vec::new();
    for source in requirement.sources {
        cited.push(source.key.clone());
        collect_source(source, violations, sources, adopt_unowned);
    }
    cited.sort();
    cited.dedup();
    for rule in requirement.rules {
        collect_rule(rule, &requirement.key, violations, rules, adopt_unowned);
    }
    requirements.push(TypedRequirementInput {
        key: requirement.key,
        id: requirement.explicit_id,
        statement: requirement.statement.unwrap_or_default(),
        description: requirement.description,
        sources: cited,
    });
}

fn collect_source(
    source: SourceBuilder,
    violations: &mut Vec<String>,
    sources: &mut Vec<TypedSourceInput>,
    adopt_unowned: &mut Vec<TypedAdoptionTarget>,
) {
    validate_explicit_id(
        "source",
        &source.key,
        source.explicit_id.as_deref(),
        violations,
    );
    collect_adoption(
        TypedDeclarationKind::Source,
        source.adopt_unowned,
        source.explicit_id.as_deref(),
        adopt_unowned,
    );
    if source
        .name
        .as_deref()
        .is_some_and(|name| name.trim().is_empty())
    {
        violations.push("source name must not be empty".to_string());
    }
    // The reference is optional. Only `document` sets one, and a
    // reference that is set must hold text.
    if source
        .reference
        .as_deref()
        .is_some_and(|reference| reference.trim().is_empty())
    {
        violations.push("document reference must not be empty".to_string());
    }
    let source_type = source.source_type.unwrap_or_else(|| {
        violations.push(format!(
            "source `{}` must declare a source type",
            source.key
        ));
        SourceType::Document
    });
    let declaration = TypedSourceInput {
        name: source.name.unwrap_or_else(|| source.key.clone()),
        key: source.key,
        id: source.explicit_id,
        kind: source_type.as_str().to_string(),
        url: None,
        reference: source.reference,
    };
    if sources
        .iter()
        .any(|existing| same_source(existing, &declaration))
    {
        return;
    }
    sources.push(declaration);
}

fn collect_rule(
    rule: RuleBuilder,
    owner: &str,
    violations: &mut Vec<String>,
    rules: &mut Vec<TypedRuleInput>,
    adopt_unowned: &mut Vec<TypedAdoptionTarget>,
) {
    if text_missing(rule.statement.as_deref()) {
        violations.push(format!("Rule `{}` statement must not be empty", rule.key));
    }
    validate_explicit_id("rule", &rule.key, rule.explicit_id.as_deref(), violations);
    collect_adoption(
        TypedDeclarationKind::Rule,
        rule.adopt_unowned,
        rule.explicit_id.as_deref(),
        adopt_unowned,
    );
    let mut owners = rule.requirements;
    owners.push(owner.to_string());
    owners.sort();
    owners.dedup();
    let declaration = TypedRuleInput {
        key: rule.key,
        id: rule.explicit_id,
        address: None,
        requirement: None,
        requirements: owners,
        statement: rule.statement.unwrap_or_default(),
        name: rule.name,
        description: rule.description,
        implementation: rule
            .implementation
            .map(|(file, symbol)| TypedImplementationInput { file, symbol }),
    };
    if let Some(existing) = rules.iter_mut().find(|existing| {
        existing.key == declaration.key && same_rule_content(existing, &declaration)
    }) {
        // A matching declaration merges only when it brings a new owner;
        // a same-owner repeat stays separate so the wire checker rejects
        // it exactly as ingestion would.
        if declaration
            .requirements
            .iter()
            .any(|owner| !existing.requirements.contains(owner))
        {
            existing.requirements.extend(declaration.requirements);
            existing.requirements.sort();
            existing.requirements.dedup();
            return;
        }
    }
    rules.push(declaration);
}

/// Runs the engine-side structural set with the same checkers the wire
/// uses, collecting instead of stopping at the first defect. Rule
/// addresses are filled in on success.
fn check_structure(
    spec: &str,
    sources: &[TypedSourceInput],
    requirements: &[TypedRequirementInput],
    rules: &mut [TypedRuleInput],
    violations: &mut Vec<String>,
) {
    let mut source_keys = BTreeSet::new();
    let mut source_checker = DeclarationChecker::new("source");
    for source in sources {
        match source_checker.admit(spec, &source.key) {
            Ok(_) => {
                source_keys.insert(source.key.clone());
            }
            Err(error) => violations.push(error.to_string()),
        }
    }
    let mut requirement_keys = BTreeSet::new();
    let mut requirement_checker = DeclarationChecker::new("requirement");
    for requirement in requirements {
        match requirement_checker.admit(spec, &requirement.key) {
            Ok(_) => {
                requirement_keys.insert(requirement.key.clone());
            }
            Err(error) => violations.push(error.to_string()),
        }
    }
    let mut rule_checker = RuleChecker::new();
    for rule in rules.iter_mut() {
        match rule_checker.admit(spec, rule) {
            Ok(address) => rule.address = Some(address),
            Err(error) => violations.push(error.to_string()),
        }
    }
    violations.extend(reference_violations(
        requirements,
        rules,
        |key| source_keys.contains(key),
        |key| requirement_keys.contains(key),
    ));
}

/// A build-time content check accepts text only when characters remain
/// after trimming, the same test TypeScript's requireText applies.
#[rule("rule_rust_build_text_checks_trim")]
fn text_missing(text: Option<&str>) -> bool {
    text.is_none_or(|text| text.trim().is_empty())
}

fn validate_explicit_id(kind: &str, key: &str, id: Option<&str>, violations: &mut Vec<String>) {
    let Some(id) = id else {
        return;
    };
    if id.trim().is_empty() {
        violations.push(format!("{kind} id must not be empty"));
    } else if StableId::new(id).is_err() {
        violations.push(format!(
            "{kind} `{key}` id `{id}` must use lowercase ASCII letters, digits, '_' or '-'"
        ));
    }
}

fn collect_adoption(
    kind: TypedDeclarationKind,
    adopt: bool,
    id: Option<&str>,
    targets: &mut Vec<TypedAdoptionTarget>,
) {
    if adopt {
        targets.push(TypedAdoptionTarget {
            kind,
            id: id.unwrap_or_default().to_string(),
        });
    }
}

fn same_source(left: &TypedSourceInput, right: &TypedSourceInput) -> bool {
    left.key == right.key
        && left.id == right.id
        && left.name == right.name
        && left.kind == right.kind
        && left.reference == right.reference
}

fn same_rule_content(left: &TypedRuleInput, right: &TypedRuleInput) -> bool {
    left.id == right.id
        && left.statement == right.statement
        && left.name == right.name
        && left.description == right.description
        && implementation_pair(left) == implementation_pair(right)
}

fn implementation_pair(rule: &TypedRuleInput) -> Option<(&camino::Utf8Path, &str)> {
    rule.implementation.as_ref().map(|implementation| {
        (
            implementation.file.as_path(),
            implementation.symbol.as_str(),
        )
    })
}

/// The canonical rule sort key: the JSON-serialized address, compared
/// by bytes. Keys with characters JSON escapes order by their escaped
/// form; the requirement is a deterministic, locale-free order.
fn serialized_address(rule: &TypedRuleInput) -> String {
    rule.address.as_ref().map_or_else(String::new, |address| {
        serde_json::to_string(address.segments()).unwrap_or_default()
    })
}
