#![allow(clippy::wildcard_imports)]

use super::*;
use crate::operations::catalog as operation;

pub(super) fn register(out: &mut Vec<Definition>) {
    crate::cache::family_table::record_family_rows!(register_review_subresources, out);
}

fn routes_for_kind(
    out: &mut Vec<Definition>,
    plural: &'static str,
    singular: &'static str,
    title: &'static str,
    kind: NodeType,
) {
    history_for_kind(out, plural, singular, title, kind);
    if !matches!(kind, NodeType::Domain | NodeType::Boundary) {
        super::subresources::discussions(out, plural, singular);
    }
}

fn history_for_kind(
    out: &mut Vec<Definition>,
    plural: &'static str,
    singular: &'static str,
    title: &'static str,
    kind: NodeType,
) {
    let base = leaked(format!("/{plural}/{{id}}/history"));
    out.push(
        backed::<operation::ReviewHistory>(
            leaked(format!("list-{singular}-history")),
            leaked(format!("list{title}History")),
            HttpMethod::Get,
            base,
            "List immutable outcomes for one record.",
            ResponseKind::Items,
            vec![schema::path("id"), limit(), cursor()],
        )
        .path_field("id", "record_id")
        .fixed("record_kind", kind.as_str())
        .items_field("entries")
        .pagination(),
    );
    let entry = leaked(format!("{base}/{{entry_id}}"));
    out.push(
        backed::<operation::ReviewHistoryEntry>(
            leaked(format!("get-{singular}-history-entry")),
            leaked(format!("get{title}HistoryEntry")),
            HttpMethod::Get,
            entry,
            "Read one immutable record outcome.",
            ResponseKind::Result,
            vec![schema::path("id"), schema::path("entry_id")],
        )
        .path_field("id", "record_id")
        .fixed("record_kind", kind.as_str()),
    );
    out.push(
        backed::<operation::ReviewEvidence>(
            leaked(format!("get-{singular}-history-evidence")),
            leaked(format!("get{title}HistoryEvidence")),
            HttpMethod::Get,
            leaked(format!("{entry}/evidence/{{side}}")),
            "Read the before or after evidence for one record outcome.",
            ResponseKind::Result,
            vec![
                schema::path("id"),
                schema::path("entry_id"),
                schema::path("side"),
                schema::query("field", json!({"type":"string"})),
                schema::query("offset", json!({"type":"integer","minimum":0})),
            ],
        )
        .path_field("id", "record_id")
        .fixed("record_kind", kind.as_str())
        .result(),
    );
}

macro_rules! review_subresources_for_row {
    ($out:ident, [$kind:ident], [requirements], [$review:ident]) => {
        routes_for_kind(
            $out,
            "requirements",
            "requirement",
            "Requirement",
            NodeType::$kind,
        );
    };
    (
        $out:ident,
        [$kind:ident],
        [writable {
            mode: $mode:ident,
            plural: $plural:literal,
            singular: $singular:literal,
            singular_id: $singular_id:literal,
            plural_id: $plural_id:literal,
            create: $create:ident,
            update: $update:ident,
            create_defaults: $create_defaults:ident,
            create_aliases: $create_aliases:ident,
            update_defaults: $update_defaults:ident,
            update_aliases: $update_aliases:ident,
            nullable: $nullable:expr,
            target: $target:expr
        }],
        [$review:ident]
    ) => {
        routes_for_kind($out, $plural, $singular, $singular_id, NodeType::$kind);
    };
    ($out:ident, [$($kind:tt)*], [$($route:tt)*], []) => {};
}

macro_rules! register_review_subresources {
    (
        $out:ident;
        $($group:ident {
            $($variant:ident {
                record: $record:ty,
                field: $field:ident,
                path: $path:ident,
                meta: $meta:tt,
                node: [$($node:tt)*],
                reader: { open: $reader:ident, closed: [$($closed:tt)*], strategy: $strategy:ident },
                id: $id:ident,
                loader: [$($loader:tt)*],
                graph: [$($graph:tt)*],
                import: [$($import:tt)*],
                catalog: [$($catalog:tt)*],
                route: [$($route:tt)*]
                $(, review: $review:ident)?
            };)*
        })*
    ) => {
        $($(review_subresources_for_row!(
            $out, [$($node)*], [$($route)*], [$($review)?]
        );)*)*
    };
}

fn limit() -> Parameter {
    schema::query("limit", json!({"type":"integer","minimum":1,"maximum":200}))
}

fn cursor() -> Parameter {
    schema::query("cursor", json!({"type":"string"}))
}

fn leaked(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}
