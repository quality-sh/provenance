use crate::operations::reader::{Cursor, Live, Position, ReadContext, ReadSnapshot, PAGE_BYTES};
use provenance_core::protocol::read_failure::ReadFailure;
use provenance_core::protocol::{
    GraphNode, ResolveSymbolMatch, ResolveSymbolMatchKind, ResolveSymbolQuery, ResolveSymbolResult,
    ResolveSymbolRole,
};
use provenance_core::{ImplementationBinding, Rule, StableId, VerificationBinding};
use provenance_macros::rule;
use provenance_scanner::source_sites;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// Names the Rules bound to one code site.
///
/// Scanner sites carry a line and a symbol; bindings carry a symbol only.
/// A request that names a line therefore reads scanned sites, and a
/// request that names only a file reads both. The scanner reads the named
/// file alone, so the tree's file count never applies and the file cannot
/// be missed; a file it has no language for, or cannot read, yields no
/// sites and the bindings still answer. Binding rows give up their rule
/// id, symbol and method columns alone, and only the Rule records the page
/// serves decode.
#[rule("rule_resolve_symbol_reads_the_named_file_only")]
pub(super) async fn resolve(
    ctx: &ReadContext,
    request: ResolveSymbolQuery,
) -> anyhow::Result<ResolveSymbolResult> {
    request
        .validate()
        .map_err(provenance_core::protocol::QueryValidation::into_native)?;
    let symbol = request.symbol.as_deref();
    let scanned = ctx.live(Live::ScannedSites).scan_file(&request.file)?;
    let mut sites = scanned_matches(scanned.as_slice(), &request);
    let (cursor, position) = Cursor::open_live(
        ctx,
        "resolve-symbol",
        &(&request.file, symbol, request.line, request.limit),
        request.cursor.as_deref(),
        &scan_digest(&sites)?,
    )?;
    if request.line.is_none() {
        sites.extend(bound_matches(ctx.snapshot(), request.file.as_str(), symbol).await?);
    }
    let by_rule = group_by_rule(sites);
    let ranked = rank(ctx.snapshot(), &by_rule).await?;
    let page = fit_page(ctx.snapshot(), &ranked, &by_rule, &position, request.limit).await?;
    Ok(ResolveSymbolResult {
        file: request.file,
        symbol: request.symbol,
        next_cursor: page.next.map(|next| cursor.encode(ctx, next)).transpose()?,
        limit: request.limit,
        has_more: page.has_more,
        rules: page.rules,
        matches: page.matches,
    })
}

/// A continuation is bound to the scanned sites of the first page, so a
/// file that changed between pages refuses the cursor.
fn scan_digest(sites: &[ResolveSymbolMatch]) -> anyhow::Result<String> {
    Ok(hex::encode(Sha256::digest(serde_json::to_vec(sites)?)))
}

async fn bound_matches(
    snapshot: &ReadSnapshot,
    file: &str,
    symbol: Option<&str>,
) -> anyhow::Result<Vec<ResolveSymbolMatch>> {
    let mut sites = Vec::new();
    for site in snapshot
        .table::<ImplementationBinding>()
        .implementation_sites_for_file(file)
        .await?
    {
        sites.push(ResolveSymbolMatch {
            match_kind: match_kind(symbol, Some(&site.symbol)),
            rule_id: StableId::new(site.rule_id)?,
            role: ResolveSymbolRole::Implementation,
            line: None,
            item_name: Some(site.symbol),
            verification_method: None,
        });
    }
    for site in snapshot
        .table::<VerificationBinding>()
        .verification_sites_for_file(file)
        .await?
    {
        sites.push(ResolveSymbolMatch {
            match_kind: match_kind(symbol, site.symbol.as_deref()),
            rule_id: StableId::new(site.rule_id)?,
            role: ResolveSymbolRole::Verification,
            line: None,
            item_name: site.symbol,
            verification_method: Some(site.method),
        });
    }
    Ok(sites)
}

/// The sites of each Rule, in a fixed order, without repeats.
fn group_by_rule(sites: Vec<ResolveSymbolMatch>) -> BTreeMap<String, Vec<ResolveSymbolMatch>> {
    let mut by_rule = BTreeMap::<String, Vec<ResolveSymbolMatch>>::new();
    for site in sites {
        by_rule
            .entry(site.rule_id.as_str().to_string())
            .or_default()
            .push(site);
    }
    for sites in by_rule.values_mut() {
        sites.sort_by(|left, right| {
            left.role
                .cmp(&right.role)
                .then(left.line.cmp(&right.line))
                .then(left.item_name.cmp(&right.item_name))
                .then(left.verification_method.cmp(&right.verification_method))
                .then(left.match_kind.cmp(&right.match_kind))
        });
        sites.dedup();
    }
    by_rule
}

/// The Rules with a record, each with its tier: zero when one of its sites
/// is a symbol match, one when it has file matches only. The order is by
/// tier, then by id, so another Rule in the file never hides a symbol match.
#[rule("rule_resolve_symbol_ranks_symbol_matches_first")]
async fn rank(
    snapshot: &ReadSnapshot,
    by_rule: &BTreeMap<String, Vec<ResolveSymbolMatch>>,
) -> anyhow::Result<Vec<(u8, String)>> {
    let candidates = by_rule
        .keys()
        .map(StableId::new)
        .collect::<Result<Vec<_>, _>>()?;
    let mut ranked = snapshot
        .table::<Rule>()
        .ids_that_count(&candidates)
        .await?
        .into_iter()
        .map(|id| {
            let id = id.as_str().to_string();
            let symbol = by_rule[&id]
                .iter()
                .any(|site| site.match_kind == ResolveSymbolMatchKind::Symbol);
            (u8::from(!symbol), id)
        })
        .collect::<Vec<_>>();
    ranked.sort();
    Ok(ranked)
}

struct Page {
    rules: Vec<GraphNode>,
    matches: Vec<ResolveSymbolMatch>,
    has_more: bool,
    next: Option<Position>,
}

/// Keep the answer inside one page and inside `limit` Rules, starting at
/// the cursor position. The position names the tier and id of the next
/// Rule; stage one says the Rule record went out on an earlier page and
/// `counter` of its sites went with it. A Rule whose sites do not all fit
/// continues on the next page with its remaining sites only.
#[rule("rule_resolve_symbol_cursor_continues_the_page")]
async fn fit_page(
    snapshot: &ReadSnapshot,
    ranked: &[(u8, String)],
    by_rule: &BTreeMap<String, Vec<ResolveSymbolMatch>>,
    position: &Position,
    limit: usize,
) -> anyhow::Result<Page> {
    let window = ranked
        .iter()
        .skip_while(|(tier, id)| (*tier, id.as_str()) < (position.rank, position.id.as_str()))
        .collect::<Vec<_>>();
    let continued = window.first().is_some_and(|(tier, id)| {
        position.stage == 1 && *tier == position.rank && *id == position.id
    });
    let wanted = window
        .iter()
        .skip(usize::from(continued))
        .take(limit)
        .map(|(_, id)| StableId::new(id))
        .collect::<Result<Vec<_>, _>>()?;
    let (records, _) = snapshot.table::<Rule>().page_by_ids(&wanted, limit).await?;
    let mut records = records
        .into_iter()
        .map(|rule| (rule.id.as_str().to_string(), rule))
        .collect::<BTreeMap<_, _>>();
    let mut page = Page {
        rules: Vec::new(),
        matches: Vec::new(),
        has_more: false,
        next: None,
    };
    let mut bytes = 0;
    for (index, (tier, id)) in window.into_iter().enumerate() {
        let skip = if index == 0 && continued {
            usize::try_from(position.counter)?
        } else {
            let at_rule = Position {
                stage: 0,
                rank: *tier,
                counter: 0,
                id: id.clone(),
            };
            if page.rules.len() == limit {
                return page.cut(at_rule);
            }
            let rule = GraphNode::Rule(Box::new(
                records
                    .remove(id)
                    .ok_or_else(|| anyhow::anyhow!("ranked Rule left the snapshot"))?,
            ));
            let size = serde_json::to_vec(&rule)?.len() + 1;
            if bytes + size > PAGE_BYTES {
                return page.cut(at_rule);
            }
            bytes += size;
            page.rules.push(rule);
            0
        };
        for (counter, site) in by_rule[id].iter().enumerate().skip(skip) {
            let size = serde_json::to_vec(site)?.len() + 1;
            if bytes + size > PAGE_BYTES {
                return page.cut(Position {
                    stage: 1,
                    rank: *tier,
                    counter: i64::try_from(counter)?,
                    id: id.clone(),
                });
            }
            bytes += size;
            page.matches.push(site.clone());
        }
    }
    Ok(page)
}

impl Page {
    /// End the page before the item at `next`. A page that holds nothing
    /// cannot move forward, so it refuses.
    fn cut(mut self, next: Position) -> anyhow::Result<Self> {
        if self.rules.is_empty() && self.matches.is_empty() {
            return Err(ReadFailure::PageRecordTooLarge.into());
        }
        self.has_more = true;
        self.next = Some(next);
        Ok(self)
    }
}

fn scanned_matches(
    scans: &[provenance_scanner::FileScan],
    request: &ResolveSymbolQuery,
) -> Vec<ResolveSymbolMatch> {
    let mut sites = Vec::new();
    for site in source_sites(scans) {
        if request.line.is_some_and(|line| site.line() != line) {
            continue;
        }
        let Ok(rule_id) = StableId::new(site.rule_id()) else {
            continue;
        };
        let (role, verification_method) = match site.role() {
            provenance_scanner::SourceSiteRole::Implementation => {
                (ResolveSymbolRole::Implementation, None)
            }
            provenance_scanner::SourceSiteRole::Verification(method) => {
                (ResolveSymbolRole::Verification, Some(method.to_string()))
            }
        };
        sites.push(ResolveSymbolMatch {
            rule_id,
            role,
            line: Some(site.line()),
            item_name: site.item_name().map(str::to_string),
            verification_method,
            match_kind: match_kind(request.symbol.as_deref(), site.item_name()),
        });
    }
    sites
}

#[rule("rule_resolve_symbol_marks_symbol_matches")]
fn match_kind(symbol: Option<&str>, item_name: Option<&str>) -> ResolveSymbolMatchKind {
    if symbol.is_some_and(|wanted| item_name == Some(wanted)) {
        ResolveSymbolMatchKind::Symbol
    } else {
        ResolveSymbolMatchKind::File
    }
}
