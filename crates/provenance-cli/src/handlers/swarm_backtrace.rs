use super::validate::{
    validate_contribution_record, validate_proposal_card_record, validate_synthesis_packet_record,
};
use crate::{
    cli::ideation::SwarmBacktraceCommand,
    output::{self, OutputFormat},
};
use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::{
    packet_qualifies_proposal, AssertionRecord, Contribution, DispositionRecord, ProposalCard,
    ScopeId, StableId, SynthesisPacket,
};
use provenance_store::{
    layout::ProvenanceLayout,
    state_store::{IdeationLandingBatch, StateStore},
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

pub(super) fn handle(
    command: SwarmBacktraceCommand,
    actor_claim: Option<&provenance_core::RbacClaim>,
) -> anyhow::Result<()> {
    match command {
        SwarmBacktraceCommand::Land {
            repo,
            scope,
            run_dir,
            replace,
            format,
        } => land(repo, scope, &run_dir, replace, format, actor_claim),
    }
}

#[derive(Serialize)]
struct LandReport {
    run_dir: String,
    contributions: usize,
    synthesis_packets: usize,
    proposals: usize,
    assertions: usize,
    replace: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MergeOutput {
    #[serde(default)]
    synthesis_packet: Option<SynthesisPacket>,
    #[serde(default, alias = "synthesis")]
    synthesis_packets: Vec<SynthesisPacket>,
    #[serde(default, alias = "proposal_cards")]
    proposals: Vec<ProposalCard>,
    #[serde(default)]
    assertions: Vec<AssertionRecord>,
    #[serde(default)]
    dispositions: Vec<DispositionRecord>,
}

type MergeRecords = (
    Vec<SynthesisPacket>,
    Vec<ProposalCard>,
    Vec<AssertionRecord>,
    Vec<DispositionRecord>,
);

fn land(
    repo: Utf8PathBuf,
    scope: String,
    run_dir: &Utf8Path,
    replace: bool,
    format: OutputFormat,
    actor_claim: Option<&provenance_core::RbacClaim>,
) -> anyhow::Result<()> {
    anyhow::ensure!(run_dir.is_dir(), "--run-dir must be an existing directory");
    let scope_id = ScopeId::new(scope)?;
    let contributions = read_contributions(run_dir)?;
    let (synthesis_packets, proposals, assertions, dispositions) = read_merge_outputs(run_dir)?;
    anyhow::ensure!(
        dispositions.is_empty(),
        "swarm merge output cannot contain dispositions"
    );

    anyhow::ensure!(
        !contributions.is_empty()
            || !synthesis_packets.is_empty()
            || !proposals.is_empty()
            || !assertions.is_empty(),
        "swarm run contains no landable records"
    );

    for contribution in &contributions {
        validate_contribution_record(contribution)?;
        ensure_scope(
            &scope_id,
            &contribution.scope_id,
            "contribution",
            &contribution.id,
        )?;
    }
    for synthesis_packet in &synthesis_packets {
        validate_synthesis_packet_record(synthesis_packet)?;
        ensure_scope(
            &scope_id,
            &synthesis_packet.scope_id,
            "synthesis packet",
            &synthesis_packet.id,
        )?;
    }
    for proposal in &proposals {
        validate_proposal_card_record(proposal)?;
        ensure_scope(&scope_id, &proposal.scope_id, "proposal", &proposal.id)?;
    }
    for assertion in &assertions {
        ensure_scope(
            &scope_id,
            &assertion.scope_id,
            "assertion",
            assertion.id.as_stable_id(),
        )?;
    }
    for disposition in &dispositions {
        ensure_scope(
            &scope_id,
            &disposition.scope_id,
            "disposition",
            &disposition.id,
        )?;
    }

    let contribution_count = contributions.len();
    let synthesis_count = synthesis_packets.len();
    let proposal_count = proposals.len();
    let assertion_count = assertions.len();
    let store = StateStore::new(ProvenanceLayout::new(repo));
    preflight_land(
        &store,
        &scope_id,
        &contributions,
        &synthesis_packets,
        &proposals,
        &assertions,
        replace,
    )?;

    store.land_ideation_batch(
        actor_claim,
        &scope_id,
        IdeationLandingBatch {
            contributions,
            synthesis_packets,
            proposals,
            assertions,
            dispositions,
        },
        replace,
    )?;

    output::print(
        format,
        &LandReport {
            run_dir: run_dir.to_string(),
            contributions: contribution_count,
            synthesis_packets: synthesis_count,
            proposals: proposal_count,
            assertions: assertion_count,
            replace,
        },
    )
}

fn read_contributions(run_dir: &Utf8Path) -> anyhow::Result<Vec<Contribution>> {
    let mut contributions = Vec::new();
    for directory in ["extractors", "refuters", "contributions"] {
        for path in json_files(&run_dir.join(directory))? {
            contributions.extend(read_contribution_file(&path)?);
        }
    }
    Ok(contributions)
}

fn read_contribution_file(path: &Utf8Path) -> anyhow::Result<Vec<Contribution>> {
    let value: Value = read_json(path)?;
    if let Some(contribution) = value.get("contribution") {
        return deserialize_landing_value(path, "contribution", contribution.clone())
            .map(|contribution| vec![contribution]);
    }
    if let Some(contributions) = value.get("contributions") {
        return deserialize_landing_value(path, "contributions", contributions.clone());
    }
    if value.is_array() {
        return deserialize_landing_value(path, "contributions", value);
    }
    deserialize_landing_value(path, "contribution", value).map(|contribution| vec![contribution])
}

fn read_merge_outputs(run_dir: &Utf8Path) -> anyhow::Result<MergeRecords> {
    let mut paths = json_files(&run_dir.join("merge"))?;
    for file_name in ["merged.json", "merge.json"] {
        let path = run_dir.join(file_name);
        if path.exists() {
            paths.push(path);
        }
    }
    paths.sort();
    paths.dedup();

    let mut synthesis_packets = Vec::new();
    let mut proposals = Vec::new();
    let mut assertions = Vec::new();
    let mut dispositions = Vec::new();
    for path in paths {
        let merge_output: MergeOutput = read_json(&path)?;
        if let Some(synthesis_packet) = merge_output.synthesis_packet {
            synthesis_packets.push(synthesis_packet);
        }
        synthesis_packets.extend(merge_output.synthesis_packets);
        proposals.extend(merge_output.proposals);
        assertions.extend(merge_output.assertions);
        dispositions.extend(merge_output.dispositions);
    }
    Ok((synthesis_packets, proposals, assertions, dispositions))
}

fn json_files(directory: &Utf8Path) -> anyhow::Result<Vec<Utf8PathBuf>> {
    if !directory.exists() {
        return Ok(Vec::new());
    }
    let mut paths = Vec::new();
    for entry in
        std::fs::read_dir(directory).with_context(|| format!("failed to read {directory}"))?
    {
        let path = utf8_path(entry?.path())?;
        if path.extension() == Some("json") {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

fn read_json<T: DeserializeOwned>(path: &Utf8Path) -> anyhow::Result<T> {
    let json = std::fs::read_to_string(path).with_context(|| format!("failed to read {path}"))?;
    serde_json::from_str(&json).with_context(|| format!("{path} must be valid landing JSON"))
}

fn deserialize_landing_value<T: DeserializeOwned>(
    path: &Utf8Path,
    field: &str,
    value: Value,
) -> anyhow::Result<T> {
    serde_json::from_value(value)
        .with_context(|| format!("{path} {field} must be valid landing JSON"))
}

fn preflight_land(
    store: &StateStore,
    scope_id: &ScopeId,
    contributions: &[Contribution],
    synthesis_packets: &[SynthesisPacket],
    proposals: &[ProposalCard],
    assertions: &[AssertionRecord],
    replace: bool,
) -> anyhow::Result<()> {
    ensure_unique_run_ids(
        "contribution",
        contributions.iter().map(|record| &record.id),
    )?;
    ensure_unique_run_ids(
        "synthesis packet",
        synthesis_packets.iter().map(|record| &record.id),
    )?;
    ensure_unique_run_ids("proposal", proposals.iter().map(|record| &record.id))?;
    // Same decision the store applies when it validates the merged aggregate,
    // run early over the run's own records so a bad run fails before the
    // landing lock is taken.
    for proposal in proposals {
        let qualifying = synthesis_packets
            .iter()
            .any(|packet| packet_qualifies_proposal(packet, proposal, assertions));
        anyhow::ensure!(
            !qualifying
                || assertions
                    .iter()
                    .any(|assertion| assertion.proposal_id == proposal.id),
            "qualifying proposal {} requires an assertion",
            proposal.id.as_str()
        );
    }

    if replace {
        for proposal in proposals {
            store.ensure_proposal_card_replaceable(scope_id, &proposal.id)?;
        }
        return Ok(());
    }

    let existing_contributions = store.list_contributions(scope_id)?;
    ensure_no_existing_ids(
        "contribution",
        existing_contributions.iter().map(|record| &record.id),
        contributions.iter().map(|record| &record.id),
    )?;
    let existing_synthesis_packets = store.list_synthesis_packets(scope_id)?;
    ensure_no_existing_ids(
        "synthesis packet",
        existing_synthesis_packets.iter().map(|record| &record.id),
        synthesis_packets.iter().map(|record| &record.id),
    )
}

fn ensure_unique_run_ids<'a>(
    artifact: &str,
    ids: impl IntoIterator<Item = &'a StableId>,
) -> anyhow::Result<()> {
    let mut seen = BTreeSet::new();
    for id in ids {
        anyhow::ensure!(
            seen.insert(id.as_str().to_string()),
            "duplicate {artifact} id {} in run",
            id.as_str()
        );
    }
    Ok(())
}

fn ensure_no_existing_ids<'existing, 'incoming>(
    artifact: &str,
    existing_ids: impl IntoIterator<Item = &'existing StableId>,
    incoming_ids: impl IntoIterator<Item = &'incoming StableId>,
) -> anyhow::Result<()> {
    let existing_ids = existing_ids
        .into_iter()
        .map(|id| id.as_str().to_string())
        .collect::<BTreeSet<_>>();
    for id in incoming_ids {
        anyhow::ensure!(
            !existing_ids.contains(id.as_str()),
            "{artifact} {} already exists; rerun with --replace to replace generated records",
            id.as_str()
        );
    }
    Ok(())
}

fn utf8_path(path: std::path::PathBuf) -> anyhow::Result<Utf8PathBuf> {
    Utf8PathBuf::from_path_buf(path)
        .map_err(|path| anyhow::anyhow!("path is not valid UTF-8: {}", path.display()))
}

fn ensure_scope(
    expected: &ScopeId,
    actual: &ScopeId,
    artifact: &str,
    id: &StableId,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        actual == expected,
        "{artifact} {} scope_id must match --scope {}",
        id.as_str(),
        expected.as_str()
    );
    Ok(())
}
