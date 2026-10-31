use camino::Utf8PathBuf;
use provenance_core::{NodeType, ScopeId};

use crate::cache::{record_families, ProjectionFamily};
use crate::layout::ProvenanceLayout;

/// The canonical record file of one node kind.
pub fn path_for(layout: &ProvenanceLayout, scope: &ScopeId, kind: NodeType) -> Utf8PathBuf {
    ProjectionFamily::ALL
        .into_iter()
        .find(|family| family.node_type() == Some(kind))
        .expect("each node type has one record family")
        .shard_path(layout, scope)
}

macro_rules! define_shard_paths {
    (
        $(
            $group:ident {
                $(
                    $variant:ident {
                        record: $record:ty,
                        field: $field:ident,
                        path: $path:ident,
                        $($rest:tt)*
                    };
                )*
            }
        )*
    ) => {
        $(
            $(
                pub fn $path(layout: &ProvenanceLayout, scope: &ScopeId) -> Utf8PathBuf {
                    ProjectionFamily::$variant.shard_path(layout, scope)
                }
            )*
        )*
    };
}

record_families!(define_shard_paths);

pub(crate) fn legacy_promotion_decisions_path(
    layout: &ProvenanceLayout,
    scope: &ScopeId,
) -> Utf8PathBuf {
    layout
        .scopes_dir()
        .join(scope.as_str())
        .join("ideation")
        .join("promotion_decisions.jsonl")
}

pub fn ideation_landings_path(layout: &ProvenanceLayout, scope: &ScopeId) -> Utf8PathBuf {
    layout
        .scopes_dir()
        .join(scope.as_str())
        .join("ideation")
        .join("landings.jsonl")
}

/// Whether `key`, a path below the state directory with `/` between its
/// parts, is a file that the engine reads as canonical state: the manifest,
/// or a shard below a scope directory with a well-formed scope id.
pub(crate) fn is_canonical_key(key: &str) -> bool {
    key == "manifest.json" || scope_shard(key).is_some_and(is_shard_suffix)
}

/// Whether the directory at `key` can hold a key that
/// [`is_canonical_key`] accepts.
pub(crate) fn can_hold_canonical_keys(key: &str) -> bool {
    key == "scopes"
        || key.strip_prefix("scopes/").is_some_and(is_scope_name)
        || scope_shard(key).is_some_and(|directory| {
            shard_suffixes().any(|suffix| {
                suffix
                    .split_once('/')
                    .is_some_and(|(first, _)| first == directory)
            })
        })
}

/// Whether `name` is the file name of a month shard of messages.
pub(crate) fn is_message_month_shard_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    bytes.len() == "2026-07.jsonl".len()
        && bytes[0..4].iter().all(u8::is_ascii_digit)
        && bytes[4] == b'-'
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && &bytes[7..] == b".jsonl"
}

/// The part of `key` below its scope directory.
fn scope_shard(key: &str) -> Option<&str> {
    let (scope, shard) = key.strip_prefix("scopes/")?.split_once('/')?;
    is_scope_name(scope).then_some(shard)
}

fn is_scope_name(name: &str) -> bool {
    ScopeId::new(name).is_ok()
}

/// The path of each fixed shard below a scope directory.
fn shard_suffixes() -> impl Iterator<Item = &'static str> {
    ProjectionFamily::ALL
        .into_iter()
        .map(ProjectionFamily::shard_suffix)
        .chain([
            "ideation/promotion_decisions.jsonl",
            "ideation/landings.jsonl",
        ])
}

fn is_shard_suffix(shard: &str) -> bool {
    shard
        .strip_prefix("threads/")
        .is_some_and(is_message_month_shard_name)
        || shard_suffixes().any(|suffix| suffix == shard)
}
