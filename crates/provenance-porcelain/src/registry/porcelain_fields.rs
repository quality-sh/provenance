//! The fields of get, search, api, check, trace, prime, and the record actions.

use super::{Field, FieldValue, Relation};

pub(super) const VIEWS: &[&str] = &["record", "children", "trace", "impact", "review"];

/// The get flags after the target, in help order.
pub(super) const GET_FIELDS: &[Field] = &[
    Field::text("view").values(VIEWS),
    Field::text("depth").of(FieldValue::Count),
    Field::repeated("kind").of(FieldValue::RecordKind),
    Field::text("limit").of(FieldValue::Count),
    Field::switch("follow")
        .doc("Stream the current review document and pushed changes as JSON lines."),
    Field::switch("review_link").doc("Print a link to this record in the running review host."),
    Field::text("notify")
        .values(&["codex", "off"])
        .doc("Queue review changes into a Codex thread, or end that registration with off."),
    Field::text("thread")
        .doc("The Codex thread UUID for --notify. The default is CODEX_THREAD_ID."),
    Field::switch("no_open").doc("Print the review link and do not open it in a browser."),
];

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

/// The trace flags after the target: the get traversal bounds.
pub(super) const TRACE_FIELDS: &[Field] = &[
    Field::text("depth").of(FieldValue::Count),
    Field::repeated("kind").of(FieldValue::RecordKind),
    Field::text("limit").of(FieldValue::Count),
];

pub(super) const SEARCH_FIELDS: &[Field] = &[
    Field::text("text").leading_hyphen(),
    Field::text("cursor").leading_hyphen(),
    Field::repeated("kind").of(FieldValue::RecordKind),
    Field::text("limit").of(FieldValue::Count),
    Field::switch("include_terminal").doc("Include archived and other terminal records"),
    Field::text("file")
        .leading_hyphen()
        .doc("Return the Rules bound in this repository-relative source file"),
    Field::text("symbol")
        .leading_hyphen()
        .doc("Put the Rules bound to this symbol first; needs --file"),
    Field::text("line")
        .of(FieldValue::Count)
        .doc("Put the Rules bound at this line first; needs --file"),
];

pub(super) const API_FIELDS: &[Field] = &[
    Field::text("path"),
    Field::text("method").of(FieldValue::ApiMethod),
    Field::text("input").leading_hyphen().doc(
        "Read the JSON body from this file, or from standard input with \"-\". The body is the \
         data object, without the HTTP {\"data\": ...} wrapper",
    ),
    Field::repeated("header"),
    Field::repeated("query"),
    Field::text("filter")
        .doc("Show only catalog routes whose method, path, or description contains this text"),
    Field::text("limit")
        .of(FieldValue::CatalogLimit)
        .doc("Show at most this many catalog routes. The default is 50"),
];

pub(super) const CHECK_FIELDS: &[Field] = &[
    Field::text("repo").default("."),
    Field::switch("strict")
        .doc("Reject findings in statements committed at Git HEAD, and every bindings finding"),
    Field::text("base").doc("Compare Git HEAD with this commit instead of its first parent"),
    Field::text("statement").doc("Check this statement instead of the committed statements"),
    Field::switch("graph")
        .doc("Run only graph validity checks. Combine with other check selectors"),
    Field::switch("statements")
        .doc("Run only statement quality checks. Combine with other check selectors"),
    Field::switch("bindings")
        .doc("Run only Rule binding checks. The scan covers the repository root; --path limits it"),
    Field::switch("gaps").doc(
        "Run only the gaps check: report the unfinished parts of the graph and each Rule \
         without a Source. These findings do not fail the check",
    ),
    Field::switch("docs").doc(
        "Run only the docs check: report each Markdown link that names a page the docs site \
         does not publish",
    ),
    Field::text("scope").doc("Check only this scope. The default is every declared scope"),
    Field::text("path").doc(
        "Bindings check only: scan the files below this path. The default is the repository root",
    ),
    Field::text("baseline").doc(
        "Bindings check only: resolve evidence anchors against this earlier scan report. \
         Needs --scope",
    ),
    Field::text("output").doc(
        "Bindings check only: write the scan report of the selected scope to this file. \
         Needs --scope",
    ),
    Field::text("output_format").values(&["json", "markdown"]),
    Field::text("format").values(&["json"]),
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
