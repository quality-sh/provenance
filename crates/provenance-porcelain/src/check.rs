//! Composed repository checks for human-facing interfaces.

use provenance_macros::rule;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::{future::Future, pin::Pin};

mod input;
pub use input::{CheckInput, ReportFileFormat};

/// A distinct repository check category.
#[derive(
    Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Graph,
    Statements,
    Bindings,
    /// The unfinished parts of the graph and the Rules without a Source. It runs only
    /// when the caller selects it, and its findings never refuse the command.
    Gaps,
    /// The Markdown links of the docs site that name a page the site does not publish.
    /// It runs only when the caller selects it.
    Docs,
}

impl Category {
    pub const ALL: [Self; 5] = [
        Self::Graph,
        Self::Statements,
        Self::Bindings,
        Self::Gaps,
        Self::Docs,
    ];
    /// The categories of a check that selects none.
    pub const DEFAULT: [Self; 3] = [Self::Graph, Self::Statements, Self::Bindings];
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

/// The graph counts of each scope that one gaps run read, also with zero findings.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GapsContext {
    pub scopes: Vec<ScopeCounts>,
}

/// The graph counts of one scope.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScopeCounts {
    pub scope_id: String,
    pub requirements: usize,
    pub source_linked_requirements: usize,
    pub resolved_requirements: usize,
    pub requirements_with_rules: usize,
    pub rules_total: usize,
    pub rules_with_complete_traceability: usize,
}

/// Typed context emitted by category computations.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum CategoryContext {
    Statements(StatementContext),
    Bindings(BindingContext),
    Gaps(GapsContext),
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
    /// Gaps findings never refuse, so the run has no refusal.
    Gaps {
        findings: Vec<Finding>,
        context: GapsContext,
    },
    Docs {
        findings: Vec<Finding>,
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
            CategoryRun::Gaps { findings, context } => (
                Category::Gaps,
                findings,
                Some(CategoryContext::Gaps(context)),
                Refusal::None,
            ),
            CategoryRun::Docs { findings, refusal } => (Category::Docs, findings, None, refusal),
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

    /// This port with the run options of one check: `strict`, `base`, `scope`, and the
    /// bindings fields. `None` keeps this port. A port without run options refuses them.
    fn with_options(&self, input: &CheckInput) -> Result<Option<Arc<dyn CheckPort>>, String> {
        if input.has_run_options() {
            Err("this host does not take the strict, base, path, baseline, or output fields".into())
        } else {
            Ok(None)
        }
    }
}

impl CheckPort for Arc<dyn CheckPort> {
    fn run<'a>(&'a self, category: Category, scope: Option<&'a str>) -> PortFuture<'a> {
        self.as_ref().run(category, scope)
    }

    fn check_statement<'a>(&'a self, statement: &'a str, scope: Option<&'a str>) -> PortFuture<'a> {
        self.as_ref().check_statement(statement, scope)
    }

    fn with_options(&self, input: &CheckInput) -> Result<Option<Arc<dyn CheckPort>>, String> {
        self.as_ref().with_options(input)
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

/// Render category results as readable text.
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
    #[rule("rule_check_reports_gaps")]
    pub async fn check(&self, input: CheckInput) -> CheckOutcome {
        let configured = match self.port.with_options(&input) {
            Ok(configured) => configured,
            Err(reason) => {
                let categories = input
                    .categories()
                    .into_iter()
                    .map(|category| CategoryReport::unavailable(category, reason.clone()))
                    .collect();
                return CheckOutcome { categories };
            }
        };
        let port: &dyn CheckPort = configured.as_deref().unwrap_or(&self.port);
        let scope = input.scope.as_deref();
        let mut categories = Vec::new();
        for category in input.categories() {
            let run = match (category, input.statement.as_deref()) {
                (Category::Statements, Some(statement)) => {
                    port.check_statement(statement, scope).await
                }
                _ => port.run(category, scope).await,
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
