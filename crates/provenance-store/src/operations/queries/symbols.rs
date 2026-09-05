use crate::operations::reader::{Live, ReadContext};
use provenance_core::protocol::{
    ensure_limit, ensure_protocol_version, take_page, GraphNode, ResolveSymbolQuery,
    ResolveSymbolResult,
};
use provenance_core::{ImplementationBinding, Rule, StableId, VerificationBinding};
use provenance_macros::rule;
use provenance_scanner::source_sites;
use std::collections::BTreeSet;

/// Names the Rules bound to one code site.
///
/// Scanner sites carry a line and a symbol; bindings carry a symbol only.
/// A request that names a line therefore reads scanned sites, and a
/// request that names only a file reads both. The scanner reads the named
/// file alone, so the tree's file count never applies and the file cannot
/// be missed; a file it has no language for, or cannot read, yields no
/// sites and the bindings still answer. Rule records come from the
/// projection.
#[rule("rule_resolve_symbol_reads_the_named_file_only")]
pub(super) async fn resolve(
    ctx: &ReadContext,
    request: ResolveSymbolQuery,
) -> anyhow::Result<ResolveSymbolResult> {
    ensure_protocol_version(request.protocol_version)?;
    ensure_limit(request.limit)?;
    let file = &request.file;
    let symbol = request.symbol.as_deref();
    let include_retired = request.include_retired;
    let snapshot = ctx.snapshot();
    let mut ids = BTreeSet::new();
    let scanned = ctx.live(Live::ScannedSites).scan_file(file);
    for site in source_sites(scanned.as_slice()) {
        if symbol.is_none_or(|wanted| site.item_name() == Some(wanted))
            && request.line.is_none_or(|line| site.line() == line)
        {
            ids.insert(site.rule_id().to_string());
        }
    }
    if request.line.is_none() {
        let by_file = [file.as_str()];
        for binding in snapshot
            .table::<ImplementationBinding>()
            .by_field("file", &by_file, include_retired)
            .await?
        {
            if symbol.is_none_or(|wanted| binding.symbol == wanted) {
                ids.insert(binding.rule_id.as_str().to_string());
            }
        }
        for binding in snapshot
            .table::<VerificationBinding>()
            .by_field("file", &by_file, include_retired)
            .await?
        {
            if symbol.is_none_or(|wanted| binding.symbol.as_deref() == Some(wanted)) {
                ids.insert(binding.rule_id.as_str().to_string());
            }
        }
    }
    let wanted = ids
        .into_iter()
        .filter_map(|id| StableId::new(id).ok())
        .collect::<Vec<_>>();
    let matched = snapshot
        .table::<Rule>()
        .by_ids(&wanted, include_retired)
        .await?
        .into_iter()
        .map(|rule| GraphNode::Rule(Box::new(rule)))
        .take(request.limit + 1)
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
