use crate::output::{self, OutputFormat};
use camino::Utf8Path;
use provenance_core::coverage::{EvidenceDiffReport, EvidenceDiffState, EvidenceSiteKind};
use provenance_core::ScopeId;
use provenance_store::{cache, layout::ProvenanceLayout};
use std::fmt::Write;

pub(super) use provenance_store::stale::{gate, git};

pub(super) fn handle(
    repo: &Utf8Path,
    scope: String,
    base: Option<String>,
    head: Option<String>,
    since: Option<String>,
    strict: bool,
    format: OutputFormat,
) -> anyhow::Result<()> {
    let (base, head) = git::resolve_range(repo, base, head, since)?;
    let graph = cache::graph_evidence(&ProvenanceLayout::new(repo), &ScopeId::new(scope)?, false)?;
    let base_files = git::revision_files(repo, &base)?;
    let head_files = git::revision_files(repo, &head)?;
    let changes = git::changed_files(repo, &base, &head)?;
    let report = gate::report(repo, base, head, base_files, head_files, &changes, &graph);
    match format {
        OutputFormat::Json | OutputFormat::Jsonl => output::print(format, &report)?,
        OutputFormat::Markdown | OutputFormat::Table | OutputFormat::Toon => {
            print!("{}", render_human(&report)?);
        }
    }
    if strict && (report.summary.touched > 0 || report.summary.gone > 0) {
        anyhow::bail!(
            "evidence diff found {} touched and {} gone site(s); rerun without --strict to inspect",
            report.summary.touched,
            report.summary.gone
        );
    }
    Ok(())
}

fn render_human(report: &EvidenceDiffReport) -> anyhow::Result<String> {
    let mut output = String::from("# Evidence Diff\n\n");
    writeln!(output, "- Range: `{}`..`{}`", report.base, report.head)?;
    writeln!(output, "- Files changed: {}", report.files_changed)?;
    writeln!(output, "- Evidence sites: {}", report.summary.total_sites)?;
    writeln!(output, "- Untouched: {}", report.summary.untouched)?;
    writeln!(output, "- Touched: {}", report.summary.touched)?;
    writeln!(output, "- Moved: {}", report.summary.moved)?;
    writeln!(output, "- Gone: {}\n", report.summary.gone)?;
    for site in &report.sites {
        let coordinate = site.line.map_or_else(
            || format!("`{}`", site.file_path),
            |line| format!("`{}`:{line}", site.file_path),
        );
        let original = match (&site.original_file_path, site.original_line) {
            (Some(path), Some(line)) => format!(" from `{path}`:{line}"),
            (Some(path), None) => format!(" from `{path}`"),
            (None, Some(line)) => format!(" from line {line}"),
            (None, None) => String::new(),
        };
        writeln!(
            output,
            "- **{}** {} `{}` at {}{}",
            state_word(site.state),
            kind_word(site.kind),
            site.subject_id,
            coordinate,
            original
        )?;
    }
    Ok(output)
}

const fn state_word(state: EvidenceDiffState) -> &'static str {
    match state {
        EvidenceDiffState::Untouched => "untouched",
        EvidenceDiffState::Touched => "touched (re-verify wanted)",
        EvidenceDiffState::Moved => "moved",
        EvidenceDiffState::Gone => "gone",
    }
}

const fn kind_word(kind: EvidenceSiteKind) -> &'static str {
    match kind {
        EvidenceSiteKind::RuleBinding => "rule binding",
        EvidenceSiteKind::Verification => "verification",
        EvidenceSiteKind::Annotation => "annotation",
        EvidenceSiteKind::SourceReference => "source reference",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use provenance_core::coverage::{EvidenceDiffSite, EvidenceDiffSummary};

    #[test]
    fn human_report_names_reverification_without_firing_it() {
        let report = EvidenceDiffReport {
            base: "base".into(),
            head: "head".into(),
            files_changed: 1,
            summary: EvidenceDiffSummary {
                total_sites: 1,
                touched: 1,
                ..EvidenceDiffSummary::default()
            },
            sites: vec![EvidenceDiffSite {
                kind: EvidenceSiteKind::RuleBinding,
                subject_id: "rule_guard".into(),
                file_path: "src/guard.rs".into(),
                line: Some(4),
                end_line: Some(8),
                state: EvidenceDiffState::Touched,
                original_file_path: None,
                original_line: None,
            }],
        };

        let rendered = render_human(&report).unwrap();
        assert!(rendered.contains("touched (re-verify wanted)"));
        assert!(!rendered.contains("agent"));
    }
}
