//! By-value builders for kernel authors.
//!
//! Every builder consumes `self` and returns a new value; nothing is
//! checked until `build()`, which reports every violation at once.

use super::document::build_document;
use super::{AuthoringError, SpecDocument};
use crate::model::SourceType;

/// Starts one spec document.
pub fn spec(key: impl Into<String>) -> SpecBuilder {
    SpecBuilder {
        key: key.into(),
        requirements: Vec::new(),
    }
}

/// Starts one source declaration. Its name defaults to its key.
pub fn source(key: impl Into<String>) -> SourceBuilder {
    SourceBuilder {
        key: key.into(),
        explicit_id: None,
        adopt_unowned: false,
        name: None,
        source_type: None,
        reference: None,
    }
}

/// Starts one requirement declaration.
pub fn requirement(key: impl Into<String>) -> RequirementBuilder {
    RequirementBuilder {
        key: key.into(),
        explicit_id: None,
        adopt_unowned: false,
        statement: None,
        description: None,
        sources: Vec::new(),
        rules: Vec::new(),
    }
}

/// Starts one rule declaration.
pub fn rule(key: impl Into<String>) -> RuleBuilder {
    RuleBuilder {
        key: key.into(),
        statement: None,
        name: None,
        description: None,
        implementation: None,
        explicit_id: None,
        adopt_unowned: false,
        requirements: Vec::new(),
    }
}

#[derive(Debug, Clone)]
pub struct SpecBuilder {
    pub(super) key: String,
    pub(super) requirements: Vec<RequirementBuilder>,
}

impl SpecBuilder {
    #[must_use]
    pub fn requirements(
        mut self,
        requirements: impl IntoIterator<Item = RequirementBuilder>,
    ) -> Self {
        self.requirements.extend(requirements);
        self
    }

    /// Validates the document and freezes it in canonical order.
    pub fn build(self) -> Result<SpecDocument, AuthoringError> {
        build_document(self)
    }
}

#[derive(Debug, Clone)]
pub struct SourceBuilder {
    pub(super) key: String,
    pub(super) explicit_id: Option<String>,
    pub(super) adopt_unowned: bool,
    pub(super) name: Option<String>,
    pub(super) source_type: Option<SourceType>,
    pub(super) reference: Option<String>,
}

impl SourceBuilder {
    /// Selects an existing canonical identity without changing ownership.
    #[must_use]
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.explicit_id = Some(id.into());
        self.adopt_unowned = false;
        self
    }

    /// Selects one explicit identity for one-time unowned adoption.
    #[must_use]
    pub fn adopt_unowned(mut self, id: impl Into<String>) -> Self {
        self.explicit_id = Some(id.into());
        self.adopt_unowned = true;
        self
    }

    /// Declares the source as a document at `reference`. This is the
    /// short form of `kind(SourceType::Document)` that also gives the
    /// reference.
    #[must_use]
    pub fn document(mut self, reference: impl Into<String>) -> Self {
        self.source_type = Some(SourceType::Document);
        self.reference = Some(reference.into());
        self
    }

    /// Selects the canonical source type. It adds no optional URL or
    /// reference metadata.
    #[must_use]
    pub const fn kind(mut self, source_type: SourceType) -> Self {
        self.source_type = Some(source_type);
        self
    }

    #[must_use]
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
}

#[derive(Debug, Clone)]
pub struct RequirementBuilder {
    pub(super) key: String,
    pub(super) explicit_id: Option<String>,
    pub(super) adopt_unowned: bool,
    pub(super) statement: Option<String>,
    pub(super) description: Option<String>,
    pub(super) sources: Vec<SourceBuilder>,
    pub(super) rules: Vec<RuleBuilder>,
}

impl RequirementBuilder {
    /// Selects an existing canonical identity without changing ownership.
    #[must_use]
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.explicit_id = Some(id.into());
        self.adopt_unowned = false;
        self
    }

    /// Selects one explicit identity for one-time unowned adoption.
    #[must_use]
    pub fn adopt_unowned(mut self, id: impl Into<String>) -> Self {
        self.explicit_id = Some(id.into());
        self.adopt_unowned = true;
        self
    }

    #[must_use]
    pub fn statement(mut self, statement: impl Into<String>) -> Self {
        self.statement = Some(statement.into());
        self
    }

    #[must_use]
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Cites one declared source.
    #[must_use]
    pub fn from(mut self, source: SourceBuilder) -> Self {
        self.sources.push(source);
        self
    }

    #[must_use]
    pub fn rules(mut self, rules: impl IntoIterator<Item = RuleBuilder>) -> Self {
        self.rules.extend(rules);
        self
    }
}

#[derive(Debug, Clone)]
pub struct RuleBuilder {
    pub(super) key: String,
    pub(super) statement: Option<String>,
    pub(super) name: Option<String>,
    pub(super) description: Option<String>,
    pub(super) implementation: Option<(camino::Utf8PathBuf, String)>,
    pub(super) explicit_id: Option<String>,
    pub(super) adopt_unowned: bool,
    pub(super) requirements: Vec<String>,
}

impl RuleBuilder {
    #[must_use]
    pub fn statement(mut self, statement: impl Into<String>) -> Self {
        self.statement = Some(statement.into());
        self
    }

    #[must_use]
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    #[must_use]
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Records the primary implementation site.
    #[must_use]
    pub fn implemented_at(
        mut self,
        file: impl Into<camino::Utf8PathBuf>,
        symbol: impl Into<String>,
    ) -> Self {
        self.implementation = Some((file.into(), symbol.into()));
        self
    }

    /// The explicit-id escape hatch for identity migration.
    #[must_use]
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.explicit_id = Some(id.into());
        self.adopt_unowned = false;
        self
    }

    /// Selects one explicit identity for one-time unowned adoption.
    #[must_use]
    pub fn adopt_unowned(mut self, id: impl Into<String>) -> Self {
        self.explicit_id = Some(id.into());
        self.adopt_unowned = true;
        self
    }

    /// Names every requirement this rule refines.
    ///
    /// The requirement the rule is attached under is always included. A
    /// rule with several requirements takes the shared address shape.
    #[must_use]
    pub fn requirements(
        mut self,
        requirements: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.requirements
            .extend(requirements.into_iter().map(Into::into));
        self
    }
}
