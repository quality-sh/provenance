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
    pub context: Option<serde_json::Value>,
}

impl CategoryReport {
    pub const fn findings(category: Category, findings: Vec<Finding>) -> Self {
        Self {
            category,
            status: Status::Findings,
            findings,
            unavailable_reason: None,
            context: None,
        }
    }

    const fn passed(category: Category) -> Self {
        Self {
            category,
            status: Status::Passed,
            findings: Vec::new(),
            unavailable_reason: None,
            context: None,
        }
    }

    const fn unavailable(category: Category, reason: String) -> Self {
        Self {
            category,
            status: Status::Unavailable,
            findings: Vec::new(),
            unavailable_reason: Some(reason),
            context: None,
        }
    }

    pub fn with_context(
        category: Category,
        findings: Vec<Finding>,
        context: serde_json::Value,
    ) -> Self {
        let mut report = if findings.is_empty() {
            Self::passed(category)
        } else {
            Self::findings(category, findings)
        };
        report.context = Some(context);
        report
    }
}

/// The semantic result of one complete check request.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CheckOutcome {
    pub categories: Vec<CategoryReport>,
}

/// One future returned by an injected check port.
pub type PortFuture<'a> = Pin<Box<dyn Future<Output = Result<Vec<Finding>, String>> + Send + 'a>>;

/// Category computations supplied by a repository-aware caller.
pub trait CheckPort: Send + Sync {
    fn run<'a>(&'a self, category: Category, scope: Option<&'a str>) -> PortFuture<'a>;

    fn context(&self, _category: Category) -> Option<serde_json::Value> {
        None
    }
}

impl CheckPort for Arc<dyn CheckPort> {
    fn run<'a>(&'a self, category: Category, scope: Option<&'a str>) -> PortFuture<'a> {
        self.as_ref().run(category, scope)
    }

    fn context(&self, category: Category) -> Option<serde_json::Value> {
        self.as_ref().context(category)
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
                Ok(findings) => match self.port.context(category) {
                    None => {
                        if findings.is_empty() {
                            CategoryReport::passed(category)
                        } else {
                            CategoryReport::findings(category, findings)
                        }
                    }
                    Some(context) => CategoryReport::with_context(category, findings, context),
                },
                Err(reason) => CategoryReport::unavailable(category, reason),
            };
            categories.push(report);
        }
        CheckOutcome { categories }
    }
}
