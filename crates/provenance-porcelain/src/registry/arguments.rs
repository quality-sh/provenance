//! The typed get and check inputs that the get and check fields make.

use crate::check::Category;
use crate::get::View;
use provenance_core::NodeType;
use serde::Deserialize;

/// The get fields after the target: the view, its bounds, and the review host.
#[derive(Clone, Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GetArguments {
    pub view: Option<View>,
    pub depth: Option<usize>,
    #[serde(default)]
    pub kind: Vec<NodeType>,
    pub limit: Option<usize>,
    /// Stream the current review document and pushed changes as JSON lines.
    #[serde(default)]
    pub follow: bool,
    /// Print a link to this record in the running review host.
    #[serde(default)]
    pub review_link: bool,
    /// Queue review changes into a Codex thread, or end that registration with off.
    pub notify: Option<String>,
    /// The Codex thread UUID for --notify. The default is `CODEX_THREAD_ID`.
    pub thread: Option<String>,
    /// Print the review link and do not open it in a browser.
    #[serde(default)]
    pub no_open: bool,
}

/// The one structured format of the check result.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CheckFormat {
    Json,
}

/// The check fields. Each bool is one switch field.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CheckArguments {
    pub repo: String,
    /// Reject findings in statements committed at Git HEAD, and every bindings finding.
    #[serde(default)]
    pub strict: bool,
    /// Compare Git HEAD with this commit instead of its first parent.
    pub base: Option<String>,
    /// Check this statement instead of the committed statements.
    pub statement: Option<String>,
    /// Run only graph validity checks. Combine with other check selectors.
    #[serde(default)]
    pub graph: bool,
    /// Run only statement quality checks. Combine with other check selectors.
    #[serde(default)]
    pub statements: bool,
    /// Run only Rule binding checks. The scan covers the repository root; `path` limits it.
    #[serde(default)]
    pub bindings: bool,
    /// Run only the gaps check: the unfinished parts of the graph and each Rule without a Source.
    #[serde(default)]
    pub gaps: bool,
    /// Run only the docs check: each Markdown link that names a page the docs site does not publish.
    #[serde(default)]
    pub docs: bool,
    /// Check only this scope. The default is every declared scope.
    pub scope: Option<String>,
    /// Bindings check only: scan the files below this path.
    pub path: Option<String>,
    /// Bindings check only: an earlier scan report. Needs `--scope`.
    pub baseline: Option<String>,
    /// Bindings check only: the file for the scan report of the selected scope.
    pub output: Option<String>,
    pub output_format: Option<ReportFileFormat>,
    pub format: Option<CheckFormat>,
}

/// The format of the bindings scan report file.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReportFileFormat {
    #[default]
    Json,
    Markdown,
}

impl CheckArguments {
    /// The categories that the selector flags name, in category order.
    pub fn categories(&self) -> Vec<Category> {
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
}

pub(super) fn get_schema() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(GetArguments)).expect("schema is JSON")
}

pub(super) fn check_schema() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(CheckArguments)).expect("schema is JSON")
}

#[cfg(test)]
mod tests {
    /// Implementation aid: the view values of the get fields are the get views.
    #[test]
    fn the_get_view_field_lists_every_view() {
        assert_eq!(
            super::super::porcelain_fields::VIEWS,
            crate::get::View::ALL.map(crate::get::View::as_str)
        );
    }
}
