use crate::operations::reader::{Live, ReadContext};
use provenance_core::protocol::{GraphNode, ResolveSymbolQuery, ResolveSymbolResult};
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
/// sites and the bindings still answer. Binding candidates give up their
/// rule ids alone, and the page is chosen by id, so only the served Rule
/// records decode.
#[rule("rule_resolve_symbol_reads_the_named_file_only")]
pub(super) async fn resolve(
    ctx: &ReadContext,
    request: ResolveSymbolQuery,
) -> anyhow::Result<ResolveSymbolResult> {
    request
        .validate()
        .map_err(provenance_core::protocol::QueryValidation::into_native)?;
    let file = &request.file;
    let symbol = request.symbol.as_deref();
    let snapshot = ctx.snapshot();
    let mut ids = BTreeSet::new();
    let scanned = ctx.live(Live::ScannedSites).scan_file(file)?;
    for site in source_sites(scanned.as_slice()) {
        if symbol.is_none_or(|wanted| site.item_name() == Some(wanted))
            && request.line.is_none_or(|line| site.line() == line)
        {
            ids.insert(site.rule_id().to_string());
        }
    }
    if request.line.is_none() {
        let by_file = file.as_str();
        for rule_id in snapshot
            .table::<ImplementationBinding>()
            .rule_ids_for_file(by_file, symbol)
            .await?
        {
            ids.insert(rule_id);
        }
        for rule_id in snapshot
            .table::<VerificationBinding>()
            .rule_ids_for_file(by_file, symbol)
            .await?
        {
            ids.insert(rule_id);
        }
    }
    let wanted = ids
        .into_iter()
        .filter_map(|id| StableId::new(id).ok())
        .collect::<Vec<_>>();
    let (matched, has_more) = snapshot
        .table::<Rule>()
        .page_by_ids(&wanted, request.limit)
        .await?;
    let rules = matched
        .into_iter()
        .map(|rule| GraphNode::Rule(Box::new(rule)))
        .collect::<Vec<_>>();
    Ok(ResolveSymbolResult {
        file: request.file,
        symbol: request.symbol,
        limit: request.limit,
        has_more,
        rules,
    })
}
