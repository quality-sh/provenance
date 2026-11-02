//! Composed repository checks for human-facing interfaces.

use provenance_macros::rule;
use serde::{Deserialize, Deserializer, Serialize};
use std::sync::Arc;
use std::{future::Future, pin::Pin};

/// A distinct repository check category.
#[derive(
    Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Graph,
    Statements,
    Bindings,
}

impl Category {
    pub const ALL: [Self; 3] = [Self::Graph, Self::Statements, Self::Bindings];
}

/// The categories selected for one check, and the statement it examines in
/// place of the committed statements.
///
/// The CLI builds this request from its flags and the MCP tool deserializes
/// it, so both surfaces take the one `statement` field.
#[rule("rule_porcelain_check_statement_field_shared")]
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CheckInput {
    #[serde(
        rename = "categories",
        default,
        deserialize_with = "deserialize_categories"
    )]
    #[schemars(extend("uniqueItems" = true))]
    selectors: Vec<Category>,
    /// Check this statement instead of the committed statements.
    #[serde(default)]
    statement: Option<String>,
    #[serde(skip)]
    scope: Option<String>,
}

fn deserialize_categories<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<Category>, D::Error> {
    let mut categories = Vec::<Category>::deserialize(deserializer)?;
    categories.sort_unstable();
    categories.dedup();
    Ok(categories)
}

/// One actionable check finding.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Finding {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<serde_json::Value>,
}

/// Commit selection used by one strict statement check.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StatementContext {
    pub candidate_commit: String,
    pub base_commit: Option<String>,
}

/// Repository policy for Rule binding findings.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BindingPolicy {
    Warning,
    Error,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BindingContext {
    pub policy: BindingPolicy,
}

/// Typed context emitted by category computations.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum CategoryContext {
    Statements(StatementContext),
    Bindings(BindingContext),
}

/// Whether findings in one completed category refuse the command.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Refusal {
    #[default]
    None,
    Findings,
}

/// One atomic, category-specific computation before output status is derived.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CategoryRun {
    Graph {
        findings: Vec<Finding>,
        refusal: Refusal,
    },
    Statements {
        findings: Vec<Finding>,
        context: Option<StatementContext>,
        refusal: Refusal,
    },
    Bindings {
        findings: Vec<Finding>,
        context: BindingContext,
        refusal: Refusal,
    },
}

impl Finding {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            detail: None,
        }
    }

    pub fn with_detail(message: impl Into<String>, detail: serde_json::Value) -> Self {
        Self {
            message: message.into(),
            detail: Some(detail),
        }
    }
}

/// The result state of one selected category.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Passed,
    Findings,
    Unavailable,
}

/// The findings and state of one selected category.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CategoryReport {
    pub category: Category,
    pub status: Status,
    pub findings: Vec<Finding>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unavailable_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<CategoryContext>,
    #[serde(skip)]
    refusal: Refusal,
}

impl CategoryReport {
    pub const fn findings(category: Category, findings: Vec<Finding>) -> Self {
        Self {
            category,
            status: Status::Findings,
            findings,
            unavailable_reason: None,
            context: None,
            refusal: Refusal::None,
        }
    }

    const fn passed(category: Category) -> Self {
        Self {
            category,
            status: Status::Passed,
            findings: Vec::new(),
            unavailable_reason: None,
            context: None,
            refusal: Refusal::None,
        }
    }

    const fn unavailable(category: Category, reason: String) -> Self {
        Self {
            category,
            status: Status::Unavailable,
            findings: Vec::new(),
            unavailable_reason: Some(reason),
            context: None,
            refusal: Refusal::None,
        }
    }

    pub fn from_run(run: CategoryRun) -> Self {
        let (category, findings, context, refusal) = match run {
            CategoryRun::Graph { findings, refusal } => (Category::Graph, findings, None, refusal),
            CategoryRun::Statements {
                findings,
                context,
                refusal,
            } => (
                Category::Statements,
                findings,
                context.map(CategoryContext::Statements),
                refusal,
            ),
            CategoryRun::Bindings {
                findings,
                context,
                refusal,
            } => (
                Category::Bindings,
                findings,
                Some(CategoryContext::Bindings(context)),
                refusal,
            ),
        };
        let mut report = if findings.is_empty() {
            Self::passed(category)
        } else {
            Self::findings(category, findings)
        };
        report.context = context;
        report.refusal = refusal;
        report
    }

    pub fn refuses(&self) -> bool {
        self.status == Status::Unavailable || matches!(self.refusal, Refusal::Findings)
    }
}

/// The semantic result of one complete check request.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CheckOutcome {
    pub categories: Vec<CategoryReport>,
}

/// One future returned by an injected check port.
pub type PortFuture<'a> = Pin<Box<dyn Future<Output = Result<CategoryRun, String>> + Send + 'a>>;

/// Category computations supplied by a repository-aware caller.
pub trait CheckPort: Send + Sync {
    fn run<'a>(&'a self, category: Category, scope: Option<&'a str>) -> PortFuture<'a>;

    /// Run the statements category over one supplied statement. A host that
    /// does not implement this leaves the category unavailable.
    fn check_statement<'a>(&'a self, statement: &'a str, scope: Option<&'a str>) -> PortFuture<'a> {
        let _ = (statement, scope);
        Box::pin(async { Err("this host does not check a supplied statement".to_owned()) })
    }
}

impl CheckPort for Arc<dyn CheckPort> {
    fn run<'a>(&'a self, category: Category, scope: Option<&'a str>) -> PortFuture<'a> {
        self.as_ref().run(category, scope)
    }

    fn check_statement<'a>(&'a self, statement: &'a str, scope: Option<&'a str>) -> PortFuture<'a> {
        self.as_ref().check_statement(statement, scope)
    }
}

impl CheckInput {
    /// Create a request from explicit selectors.
    pub fn new(selectors: impl IntoIterator<Item = Category>) -> Self {
        let mut selectors = selectors.into_iter().collect::<Vec<_>>();
        selectors.sort_unstable();
        selectors.dedup();
        Self {
            selectors,
            statement: None,
            scope: None,
        }
    }

    /// Check this statement instead of the committed statements.
    #[must_use]
    pub fn with_statement(mut self, statement: impl Into<String>) -> Self {
        self.statement = Some(statement.into());
        self
    }

    /// Bind a trusted host-selected scope to this request.
    #[must_use]
    pub fn in_scope(mut self, scope: impl Into<String>) -> Self {
        self.scope = Some(scope.into());
        self
    }

    /// Return selected categories. Without a selector, a supplied statement
    /// selects the statements category and a bare check selects all.
    pub fn categories(&self) -> &[Category] {
        const STATEMENTS: [Category; 1] = [Category::Statements];
        match (self.selectors.is_empty(), &self.statement) {
            (false, _) => &self.selectors,
            (true, Some(_)) => &STATEMENTS,
            (true, None) => &Category::ALL,
        }
    }
}

fn schema<T: schemars::JsonSchema>(contract: schemars::generate::Contract) -> serde_json::Value {
    serde_json::to_value(
        schemars::generate::SchemaSettings::draft2020_12()
            .with(|settings| settings.contract = contract)
            .into_generator()
            .into_root_schema_for::<T>(),
    )
    .expect("typed check schema is JSON")
}

/// The schema of the same request that the check service deserializes.
pub fn input_schema() -> serde_json::Value {
    schema::<CheckInput>(schemars::generate::Contract::Deserialize)
}

/// The schema of the result emitted by the check service.
pub fn output_schema() -> serde_json::Value {
    schema::<CheckOutcome>(schemars::generate::Contract::Serialize)
}

/// Render category results for terminal and MCP readers.
pub fn render_readable(outcome: &CheckOutcome) -> String {
    outcome
        .categories
        .iter()
        .flat_map(|report| {
            let heading =
                format!("{:?}: {:?}", report.category, report.status).to_ascii_lowercase();
            std::iter::once(heading)
                .chain(
                    report
                        .findings
                        .iter()
                        .map(|finding| format!("  - {}", finding.message)),
                )
                .chain(
                    report
                        .unavailable_reason
                        .iter()
                        .map(|reason| format!("  - {reason}")),
                )
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

impl<P: CheckPort> crate::Porcelain<P> {
    /// Run selected categories and keep each semantic result separate. A
    /// supplied statement takes the place of the committed statements.
    #[rule("rule_porcelain_check_defaults_all")]
    #[rule("rule_porcelain_check_selector_union")]
    #[rule("rule_porcelain_check_categories")]
    #[rule("rule_porcelain_check_unavailable")]
    #[rule("rule_porcelain_check_supplied_statement_selects_statements")]
    pub async fn check(&self, input: CheckInput) -> CheckOutcome {
        let scope = input.scope.as_deref();
        let mut categories = Vec::new();
        for &category in input.categories() {
            let run = match (category, input.statement.as_deref()) {
                (Category::Statements, Some(statement)) => {
                    self.port.check_statement(statement, scope).await
                }
                _ => self.port.run(category, scope).await,
            };
            let report = match run {
                Ok(run) => CategoryReport::from_run(run),
                Err(reason) => CategoryReport::unavailable(category, reason),
            };
            categories.push(report);
        }
        CheckOutcome { categories }
    }
}
