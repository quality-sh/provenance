use camino::Utf8Path;
use provenance_core::protocol::{
    ensure_limit, ensure_protocol_version, take_page, ResolveSymbolQuery, ResolveSymbolResult,
};
use provenance_core::{NodeType, ScopeId, StableId};
use provenance_scanner::source_sites;
use provenance_store::state_store::StateStore;
use std::collections::BTreeSet;

use super::super::sites::relative;
use super::{bindings::Bindings, records};

/// Names the Rules bound to one code site.
///
/// Scanner sites carry a line and a symbol; canonical bindings carry a
/// symbol only. A request that names a line therefore reads scanned sites,
/// and a request that names only a file reads both.
pub(super) fn resolve(
    repo: &Utf8Path,
    store: &StateStore,
    scope: &ScopeId,
    request: ResolveSymbolQuery,
) -> anyhow::Result<ResolveSymbolResult> {
    ensure_protocol_version(request.protocol_version)?;
    ensure_limit(request.limit)?;
    let file = &request.file;
    let symbol = request.symbol.as_deref();
    let bindings = Bindings::load(store, scope, request.include_retired)?;
    let scans = provenance_scanner::scan_path(repo)?;
    let mut ids = BTreeSet::new();
    for site in source_sites(&scans) {
        if relative(repo, site.file_path()) == *file
            && symbol.is_none_or(|wanted| site.item_name() == Some(wanted))
            && request.line.is_none_or(|line| site.line() == line)
        {
            ids.insert(site.rule_id().to_string());
        }
    }
    if request.line.is_none() {
        for binding in &bindings.implementations {
            if binding.file == *file && symbol.is_none_or(|wanted| binding.symbol == wanted) {
                ids.insert(binding.rule_id.as_str().to_string());
            }
        }
        for binding in &bindings.verifications {
            if binding.file == *file
                && symbol.is_none_or(|wanted| binding.symbol.as_deref() == Some(wanted))
            {
                ids.insert(binding.rule_id.as_str().to_string());
            }
        }
    }
    let nodes = records::load(store, scope, request.include_retired)?;
    let matched = ids
        .into_iter()
        .filter_map(|id| {
            StableId::new(id)
                .ok()
                .and_then(|id| records::find(&nodes, Some(NodeType::Rule), &id))
        })
        .take(request.limit + 1)
        .cloned()
        .collect::<Vec<_>>();
    let (rules, has_more) = take_page(matched, request.limit);
    Ok(ResolveSymbolResult {
        file: request.file,
        symbol: request.symbol,
        limit: request.limit,
        has_more,
        rules,
    })
}
