//! The fields of get, search, api, check, trace, prime, and the record actions.
//!
//! A doc names another field by its field name.

use super::{Field, FieldValue, Relation};

pub(super) const VIEWS: &[&str] = &["record", "children", "trace", "impact", "review"];

/// The get fields after the target, in order.
pub(super) const GET_FIELDS: &[Field] = &[
    Field::text("view").values(VIEWS),
    DEPTH,
    KIND,
    LIMIT,
    Field::switch("follow")
        .doc("Stream the current review document and pushed changes as JSON lines."),
    Field::switch("review_link").doc("Print a link to this record in the running review host."),
    Field::text("notify")
        .values(&["codex", "off"])
        .writes()
        .doc("Queue review changes into a Codex thread, or end that registration with off."),
    Field::text("thread")
        .writes()
        .doc("The Codex thread UUID for notify."),
    Field::switch("no_open").doc("Print the review link and do not open it in a browser."),
];

const DEPTH: Field = Field::text("depth")
    .of(FieldValue::Count)
    .doc("The number of levels to walk. The default is 1.");
const KIND: Field = Field::repeated("kind")
    .of(FieldValue::RecordKind)
    .doc("Record kinds to return.");
const LIMIT: Field = Field::text("limit")
    .of(FieldValue::Count)
    .doc("The page size. The default is 50.");

pub(super) const GET_RELATIONS: &[Relation] = &[
    Relation::Requires("follow", "view"),
    Relation::Conflicts("follow", "review_link"),
    Relation::Conflicts("review_link", "view"),
    Relation::Conflicts("review_link", "depth"),
    Relation::Conflicts("review_link", "kind"),
    Relation::Conflicts("review_link", "limit"),
    Relation::Requires("notify", "view"),
    Relation::Conflicts("notify", "follow"),
    Relation::Conflicts("notify", "review_link"),
    Relation::Requires("thread", "notify"),
    Relation::Requires("no_open", "review_link"),
];

/// The trace fields after the target: the get traversal bounds.
pub(super) const TRACE_FIELDS: &[Field] = &[DEPTH, KIND, LIMIT];

pub(super) const SEARCH_FIELDS: &[Field] = &[
    Field::text("text")
        .leading_hyphen()
        .doc("Text to find in record ids and text."),
    Field::text("cursor")
        .leading_hyphen()
        .doc("The continuation from an earlier page of the same search."),
    KIND,
    Field::text("limit")
        .of(FieldValue::Count)
        .doc("The page size. The default is 50."),
    Field::switch("include_terminal").doc("Include archived and other terminal records."),
    Field::text("file")
        .leading_hyphen()
        .doc("Return the Rules bound in this repository-relative source file."),
    Field::text("symbol")
        .leading_hyphen()
        .doc("Put the Rules bound to this symbol first. Needs file."),
    Field::text("line")
        .of(FieldValue::Count)
        .doc("Put the Rules bound at this line first. Needs file."),
];

pub(super) const API_FIELDS: &[Field] = &[
    Field::text("path").doc("The public API path. Without a path, the catalog routes show."),
    Field::text("method").of(FieldValue::ApiMethod),
    Field::text("input")
        .of(FieldValue::Object)
        .leading_hyphen()
        .doc("The JSON body: the data object, without the HTTP {\"data\": ...} wrapper."),
    Field::repeated("header")
        .of(FieldValue::Pairs(':'))
        .doc("A request header, as NAME: VALUE."),
    Field::repeated("query")
        .of(FieldValue::Pairs('='))
        .doc("A query parameter, as NAME=VALUE."),
    Field::text("filter")
        .doc("Show only catalog routes whose method, path, or description contains this text."),
    Field::text("limit")
        .of(FieldValue::CatalogLimit)
        .doc("Show at most this many catalog routes."),
];

pub(super) const CHECK_FIELDS: &[Field] = &[
    Field::switch("strict")
        .doc("Reject findings in statements committed at Git HEAD, and every bindings finding."),
    Field::text("base").doc("Compare Git HEAD with this commit instead of its first parent."),
    Field::text("statement").doc("Check this statement instead of the committed statements."),
    Field::switch("graph")
        .doc("Run only graph validity checks. Combine with other check selectors."),
    Field::switch("statements")
        .doc("Run only statement quality checks. Combine with other check selectors."),
    Field::switch("bindings")
        .doc("Run only Rule binding checks. The scan covers the repository root; path limits it."),
    Field::switch("gaps").doc(
        "Run only the gaps check: report the unfinished parts of the graph and each Rule \
         without a Source. These findings do not fail the check.",
    ),
    Field::switch("docs").doc(
        "Run only the docs check: report each Markdown link that names a page the docs site \
         does not publish.",
    ),
    Field::text("scope")
        .doc("Check only this scope. The default is every scope that the caller can read."),
    Field::text("path").doc(
        "Bindings check only: scan the files below this path. The default is the repository root.",
    ),
    Field::text("baseline").doc(
        "Bindings check only: resolve evidence anchors against this earlier scan report. \
         Needs scope.",
    ),
    Field::text("output").doc(
        "Bindings check only: write the scan report of the selected scope to this file. \
         Needs scope.",
    ),
    Field::text("output_format").values(&["json", "markdown"]),
];

pub(super) const PRIME_FIELDS: &[Field] = &[
    Field::text("repo")
        .default(".")
        .doc("Deprecated: accepted for compatibility; guidance does not use a repository"),
    Field::text("scope")
        .default("default")
        .doc("Deprecated: accepted for compatibility; guidance does not use a scope"),
    Field::text("format")
        .values(&["json", "markdown"])
        .default("markdown"),
    Field::switch("include_threads")
        .doc("Deprecated: accepted for compatibility; guidance does not include project threads"),
];

pub(super) const CREATE_FIELDS: &[Field] = &[Field::text("type").of(FieldValue::RecordKind)];

pub(super) const UPDATE_FIELDS: &[Field] = &[
    Field::text("replace_parent")
        .doc("Replace this Requirement parent and preserve its other parents"),
    Field::text("with_parent").doc("Add this parent before the selected parent is removed"),
];

pub(super) const UPDATE_RELATIONS: &[Relation] = &[
    Relation::Requires("replace_parent", "with_parent"),
    Relation::Requires("with_parent", "replace_parent"),
];

pub(super) const CHECK_RELATIONS: &[Relation] = &[
    Relation::Requires("base", "strict"),
    Relation::Conflicts("statement", "base"),
    Relation::Requires("baseline", "scope"),
    Relation::Requires("output", "scope"),
    Relation::Requires("output_format", "output"),
];

#[cfg(test)]
mod tests {
    /// Implementation aid: the view values of the get fields are the get views.
    #[test]
    fn the_get_view_field_lists_every_view() {
        assert_eq!(
            super::VIEWS,
            crate::get::View::ALL.map(crate::get::View::as_str)
        );
    }
}
