use crate::operations::reader::{Live, ReadContext};
use provenance_core::protocol::{
    GraphNode, ResolveSymbolMatch, ResolveSymbolMatchKind, ResolveSymbolQuery, ResolveSymbolResult,
    ResolveSymbolRole,
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
    let mut matches = Vec::new();
    let scanned = ctx.live(Live::ScannedSites).scan_file(file)?;
    for site in source_sites(scanned.as_slice()) {
        if !request.line.is_none_or(|line| site.line() == line) {
            continue;
        }
        ids.insert(site.rule_id().to_string());
        let (role, verification_method) = match site.role() {
            provenance_scanner::SourceSiteRole::Implementation => {
                (ResolveSymbolRole::Implementation, None)
            }
            provenance_scanner::SourceSiteRole::Verification(method) => {
                (ResolveSymbolRole::Verification, Some(method.to_string()))
            }
        };
        matches.push(ResolveSymbolMatch {
            rule_id: StableId::new(site.rule_id())?,
            role,
            line: Some(site.line()),
            item_name: site.item_name().map(str::to_string),
            verification_method,
            match_kind: if symbol.is_some_and(|wanted| site.item_name() == Some(wanted)) {
                ResolveSymbolMatchKind::Symbol
            } else {
                ResolveSymbolMatchKind::File
            },
        });
    }
    if request.line.is_none() {
        let by_file = file.as_str();
        for rule_id in snapshot
            .table::<ImplementationBinding>()
            .rule_ids_for_file(by_file, None)
            .await?
        {
            ids.insert(rule_id);
        }
        for rule_id in snapshot
            .table::<VerificationBinding>()
            .rule_ids_for_file(by_file, None)
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
    let served = rules.iter().map(GraphNode::id).collect::<BTreeSet<_>>();
    matches.retain(|site| served.contains(&site.rule_id));
    matches.sort();
    Ok(ResolveSymbolResult {
        file: request.file,
        symbol: request.symbol,
        limit: request.limit,
        has_more,
        rules,
        matches,
    })
}
