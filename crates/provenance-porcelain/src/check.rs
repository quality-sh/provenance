//! Composed repository checks for human-facing interfaces.

use provenance_macros::rule;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::{future::Future, pin::Pin};

/// A distinct repository check category.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Graph,
    Statements,
    Bindings,
}

const ALL_CATEGORIES: &[Category] = &[Category::Graph, Category::Statements, Category::Bindings];

/// The categories selected for one check.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CheckInput {
    selectors: Vec<Category>,
    scope: Option<String>,
}

/// One actionable check finding.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Finding {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<serde_json::Value>,
}

/// Commit selection used by one strict statement check.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StatementContext {
    pub candidate_commit: String,
    pub base_commit: Option<String>,
}

/// Repository policy for Rule binding findings.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BindingPolicy {
    Warning,
    Error,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BindingContext {
    pub policy: BindingPolicy,
}

/// Typed context emitted by category computations.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
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

/// One atomic category computation before output status is derived.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CategoryRun {
    pub category: Category,
    pub findings: Vec<Finding>,
    pub context: Option<CategoryContext>,
    pub refusal: Refusal,
}

impl CategoryRun {
    pub const fn new(category: Category, findings: Vec<Finding>) -> Self {
        Self {
            category,
            findings,
            context: None,
            refusal: Refusal::None,
        }
    }

    #[must_use]
    pub fn with_context(mut self, context: CategoryContext) -> Self {
        self.context = Some(context);
        self
    }

    #[must_use]
    pub const fn with_refusal(mut self, refusal: Refusal) -> Self {
        self.refusal = refusal;
        self
    }
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
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Passed,
    Findings,
    Unavailable,
}

/// The findings and state of one selected category.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
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
        let mut report = if run.findings.is_empty() {
            Self::passed(run.category)
        } else {
            Self::findings(run.category, run.findings)
        };
        report.context = run.context;
        report.refusal = run.refusal;
        report
    }

    pub fn refuses(&self) -> bool {
        self.status == Status::Unavailable || matches!(self.refusal, Refusal::Findings)
    }
}

/// The semantic result of one complete check request.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CheckOutcome {
    pub categories: Vec<CategoryReport>,
}

/// One future returned by an injected check port.
pub type PortFuture<'a> = Pin<Box<dyn Future<Output = Result<CategoryRun, String>> + Send + 'a>>;

/// Category computations supplied by a repository-aware caller.
pub trait CheckPort: Send + Sync {
    fn run<'a>(&'a self, category: Category, scope: Option<&'a str>) -> PortFuture<'a>;
}

impl CheckPort for Arc<dyn CheckPort> {
    fn run<'a>(&'a self, category: Category, scope: Option<&'a str>) -> PortFuture<'a> {
        self.as_ref().run(category, scope)
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
            scope: None,
        }
    }

    /// Bind a trusted host-selected scope to this request.
    #[must_use]
    pub fn in_scope(mut self, scope: impl Into<String>) -> Self {
        self.scope = Some(scope.into());
        self
    }

    /// Return selected categories, or all categories when none were selected.
    pub fn categories(&self) -> &[Category] {
        if self.selectors.is_empty() {
            ALL_CATEGORIES
        } else {
            &self.selectors
        }
    }
}

impl<P: CheckPort> crate::Porcelain<P> {
    /// Run selected categories and keep each semantic result separate.
    #[rule("rule_porcelain_check_defaults_all")]
    #[rule("rule_porcelain_check_selector_union")]
    #[rule("rule_porcelain_check_categories")]
    #[rule("rule_porcelain_check_unavailable")]
    pub async fn check(&self, input: CheckInput) -> CheckOutcome {
        let mut categories = Vec::new();
        for &category in input.categories() {
            let report = match self.port.run(category, input.scope.as_deref()).await {
                Ok(run) => CategoryReport::from_run(run),
                Err(reason) => CategoryReport::unavailable(category, reason),
            };
            categories.push(report);
        }
        CheckOutcome { categories }
    }
}
