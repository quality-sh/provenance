use crate::operations::reader::{kind_of, Live, ReadContext, SqlFront};
use provenance_core::model::relations::flow_neighbors;
use provenance_core::protocol::{
    ensure_limit, ensure_protocol_version, take_page, ImpactQuery, ImpactResult, TRACE_MAX_DEPTH,
};
use provenance_core::{ImplementationBinding, NodeType, Rule, StableId, VerificationBinding};
use provenance_macros::rule;
use std::collections::BTreeSet;

use super::super::sites;
use super::nodes::{self, Key};

/// Names every Rule a record reaches, with the code standing behind it.
///
/// The walk follows each declared relation in its flow direction, one
/// fetched hop per step, up to the depth cap `trace` uses, and never a
/// step no declaration gives: a Requirement reaches its Rules directly, a
/// Source reaches them through the Requirements that cite it, and a
/// Resolution reaches the Rules that name it, not the Requirements it
/// answers. The working-tree scan behind the sites stops at the
/// configured file count and `scan_cut` says when it did.
#[rule("rule_impact_follows_declared_flow")]
pub(super) async fn impact(
    ctx: &ReadContext,
    request: ImpactQuery,
) -> anyhow::Result<ImpactResult> {
    ensure_protocol_version(request.protocol_version)?;
    ensure_limit(request.limit)?;
    let id = StableId::new(request.id.clone())?;
    let include_retired = request.include_retired;
    let snapshot = ctx.snapshot();
    let mut rules = BTreeSet::new();
    if snapshot.table::<Rule>().live(&id, include_retired).await? {
        rules.insert(id.as_str().to_string());
    }
    let mut frontier: Vec<(NodeType, StableId)> = kind_of(snapshot, &id, include_retired)
        .await?
        .map(|node_type| vec![(node_type, id)])
        .unwrap_or_default();
    let mut seen: BTreeSet<Key> = frontier
        .iter()
        .map(|(node_type, id)| nodes::key(*node_type, id))
        .collect();
    for _ in 0..TRACE_MAX_DEPTH {
        if frontier.is_empty() {
            break;
        }
        let front = SqlFront::hop(&snapshot.relations(), &frontier).await?;
        let mut candidates = Vec::new();
        for (origin_type, origin) in &frontier {
            for step in flow_neighbors(&front, *origin_type, origin, true) {
                // Marked seen before the record is checked, so a second
                // path to a retired or dangling record is skipped too.
                if seen.insert(nodes::key(step.endpoint.node_type, &step.endpoint.id)) {
                    candidates.push((step.endpoint.node_type, step.endpoint.id));
                }
            }
        }
        frontier = nodes::counting(snapshot, &candidates, include_retired).await?;
        for (node_type, id) in &frontier {
            if *node_type == NodeType::Rule {
                rules.insert(id.as_str().to_string());
            }
        }
    }
    let wanted = rules
        .into_iter()
        .take(request.limit + 1)
        .collect::<Vec<_>>();
    let (wanted, has_more) = take_page(wanted, request.limit);
    let rule_ids: Vec<&str> = wanted.iter().map(String::as_str).collect();
    let (implementations, verifications) = if rule_ids.is_empty() {
        (Vec::new(), Vec::new())
    } else {
        (
            snapshot
                .table::<ImplementationBinding>()
                .by_field("rule_id", &rule_ids, include_retired)
                .await?,
            snapshot
                .table::<VerificationBinding>()
                .by_field("rule_id", &rule_ids, include_retired)
                .await?,
        )
    };
    let (scans, scan_cut) = ctx.live(Live::ScannedSites).scan_tree()?;
    let evidence = sites::Evidence {
        scans: &scans,
        verifications: &verifications,
        implementations: &implementations,
    };
    let repo = ctx.repo();
    let affected_rules = wanted
        .into_iter()
        .map(|rule| Ok(evidence.affected_rule(repo, StableId::new(rule)?)))
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(ImpactResult {
        id: request.id,
        limit: request.limit,
        has_more,
        affected_rules,
        scan_cut,
    })
}
