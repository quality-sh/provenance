//! The check fields: the categories to run, a statement to check in place of the
//! committed statements, and the run options.

use super::Category;
use provenance_macros::rule;
use serde::Deserialize;

/// The format of the bindings scan report file.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReportFileFormat {
    #[default]
    Json,
    Markdown,
}

/// One check request. Each switch field selects one category; a check that selects
/// none runs the default categories.
#[rule("rule_porcelain_check_statement_field_shared")]
// Each bool is one switch field.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CheckInput {
    #[serde(default)]
    pub strict: bool,
    #[serde(default)]
    pub base: Option<String>,
    #[serde(default)]
    pub statement: Option<String>,
    #[serde(default)]
    pub graph: bool,
    #[serde(default)]
    pub statements: bool,
    #[serde(default)]
    pub bindings: bool,
    #[serde(default)]
    pub gaps: bool,
    #[serde(default)]
    pub docs: bool,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub baseline: Option<String>,
    #[serde(default)]
    pub output: Option<String>,
    #[serde(default)]
    pub output_format: Option<ReportFileFormat>,
}

impl CheckInput {
    /// Create a request that selects these categories.
    pub fn new(selectors: impl IntoIterator<Item = Category>) -> Self {
        let mut input = Self::default();
        for category in selectors {
            *input.selector(category) = true;
        }
        input
    }

    const fn selector(&mut self, category: Category) -> &mut bool {
        match category {
            Category::Graph => &mut self.graph,
            Category::Statements => &mut self.statements,
            Category::Bindings => &mut self.bindings,
            Category::Gaps => &mut self.gaps,
            Category::Docs => &mut self.docs,
        }
    }

    /// The categories that the switch fields select, in category order.
    pub fn selected(&self) -> Vec<Category> {
        Category::ALL
            .into_iter()
            .filter(|category| match category {
                Category::Graph => self.graph,
                Category::Statements => self.statements,
                Category::Bindings => self.bindings,
                Category::Gaps => self.gaps,
                Category::Docs => self.docs,
            })
            .collect()
    }

    /// Check this statement instead of the committed statements.
    #[must_use]
    pub fn with_statement(mut self, statement: impl Into<String>) -> Self {
        self.statement = Some(statement.into());
        self
    }

    /// Check only this scope.
    #[must_use]
    pub fn in_scope(mut self, scope: impl Into<String>) -> Self {
        self.scope = Some(scope.into());
        self
    }

    /// Return the categories to run, in run order. A bare check selects the default
    /// categories. A supplied statement selects the statements category, and the
    /// switch fields add to it.
    pub fn categories(&self) -> Vec<Category> {
        let selected = self.selected();
        let mut categories = match (selected.is_empty(), &self.statement) {
            (false, _) => selected,
            (true, Some(_)) => Vec::new(),
            (true, None) => Category::DEFAULT.to_vec(),
        };
        if self.statement.is_some() && !categories.contains(&Category::Statements) {
            categories.push(Category::Statements);
            categories.sort_unstable();
        }
        categories
    }

    /// Whether the request has a field that changes how the categories run.
    pub const fn has_run_options(&self) -> bool {
        self.strict
            || self.base.is_some()
            || self.path.is_some()
            || self.baseline.is_some()
            || self.output.is_some()
            || self.output_format.is_some()
    }

    /// Refuses the bindings fields when the bindings category does not run.
    pub fn validate(&self) -> Result<(), String> {
        let bindings_fields = self.path.is_some()
            || self.baseline.is_some()
            || self.output.is_some()
            || self.output_format.is_some();
        if bindings_fields && !self.categories().contains(&Category::Bindings) {
            return Err(
                "path, baseline, output, and output_format apply only to the \
                        bindings check; add bindings"
                    .to_owned(),
            );
        }
        Ok(())
    }
}
