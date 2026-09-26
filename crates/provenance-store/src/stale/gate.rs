use super::git::{ChangedFile, RevisionFile};
use crate::cache::GraphEvidence;
use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::coverage::{
    AnchorState, EvidenceDiffReport, EvidenceDiffSite, EvidenceDiffState, EvidenceDiffSummary,
    EvidenceSiteKind,
};
use provenance_scanner::{CoverageBaseline, FileScanWithContent, ScannedCoverage};
use std::collections::BTreeSet;

mod source_refs;
mod typed_bindings;

pub fn report(
    repo: &Utf8Path,
    base: String,
    head: String,
    base_files: Vec<RevisionFile>,
    head_files: Vec<RevisionFile>,
    changes: &[ChangedFile],
    graph: &GraphEvidence,
) -> EvidenceDiffReport {
    let base_scan = scan_revision(&base, base_files, None);
    let baseline = CoverageBaseline {
        scan: &base_scan,
        repo,
        scan_path: repo,
        validate_rules: false,
    };
    let head_scan = scan_revision(&head, head_files, Some(baseline));
    let mut sites = marker_sites(&base_scan, &head_scan, changes, &graph.rule_ids);
    let typed_sites = typed_bindings::sites(&sites, &head_scan, graph, changes);
    sites.extend(typed_sites);
    sites.extend(
        graph
            .references
            .iter()
            .flat_map(|reference| source_refs::sites(reference, &head_scan.bindings, changes)),
    );
    sites.sort_by(|left, right| {
        (
            &left.file_path,
            left.line,
            kind_rank(left.kind),
            &left.subject_id,
        )
            .cmp(&(
                &right.file_path,
                right.line,
                kind_rank(right.kind),
                &right.subject_id,
            ))
    });
    let summary = summarize(&sites);
    EvidenceDiffReport {
        base,
        head,
        files_changed: changes.len(),
        summary,
        sites,
    }
}

fn scan_revision(
    commit: &str,
    files: Vec<RevisionFile>,
    baseline: Option<CoverageBaseline<'_>>,
) -> ScannedCoverage {
    let files = files
        .into_iter()
        .map(|file| {
            let language = file
                .path
                .extension()
                .and_then(provenance_scanner::Language::from_extension)
                .expect("revision files were filtered by scanner language");
            FileScanWithContent {
                scan: provenance_scanner::scan_file(&file.path, language, &file.content),
                content: file.content,
            }
        })
        .collect::<Vec<_>>();
    provenance_scanner::scan_to_coverage(&files, Some(commit.to_string()), Vec::new(), baseline)
}

fn marker_sites(
    base: &ScannedCoverage,
    head: &ScannedCoverage,
    changes: &[ChangedFile],
    known_rules: &BTreeSet<String>,
) -> Vec<EvidenceDiffSite> {
    head.annotations
        .iter()
        .filter(|site| known_rules.contains(&site.rule_id))
        .map(|site| {
            let kind = if site.verification.is_some() {
                EvidenceSiteKind::Verification
            } else {
                EvidenceSiteKind::RuleBinding
            };
            marker_site(
                kind,
                &site.rule_id,
                &site.file_path,
                site.line,
                site.anchor_state,
                site.is_current(),
                site.original_file_path.clone(),
                site.original_line,
                base,
                head,
                changes,
            )
        })
        .chain(
            head.bindings
                .iter()
                .filter(|site| known_rules.contains(&site.rule_id))
                .map(|site| {
                    marker_site(
                        if site.verification.is_some() {
                            EvidenceSiteKind::Verification
                        } else {
                            EvidenceSiteKind::RuleBinding
                        },
                        &site.rule_id,
                        &site.file_path,
                        site.line,
                        site.anchor_state,
                        site.is_current(),
                        site.original_file_path.clone(),
                        site.original_line,
                        base,
                        head,
                        changes,
                    )
                }),
        )
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn marker_site(
    kind: EvidenceSiteKind,
    subject_id: &str,
    path: &Utf8Path,
    line: usize,
    anchor_state: AnchorState,
    current: bool,
    original_file_path: Option<Utf8PathBuf>,
    original_line: Option<usize>,
    base: &ScannedCoverage,
    head: &ScannedCoverage,
    changes: &[ChangedFile],
) -> EvidenceDiffSite {
    let spans = if current { head } else { base };
    let end_line = spans.site_end_line(path, line);
    let state = match anchor_state {
        AnchorState::Moved => {
            if moved_site_content_changed(
                path,
                line,
                end_line,
                original_file_path.as_deref(),
                original_line,
                base,
                head,
            ) {
                EvidenceDiffState::Touched
            } else {
                EvidenceDiffState::Moved
            }
        }
        AnchorState::Gone => EvidenceDiffState::Gone,
        AnchorState::Unchanged | AnchorState::New => {
            if changed_range(changes, path, line, end_line.unwrap_or(line), current) {
                EvidenceDiffState::Touched
            } else {
                EvidenceDiffState::Untouched
            }
        }
    };
    EvidenceDiffSite {
        kind,
        subject_id: subject_id.to_string(),
        file_path: path.to_path_buf(),
        line: Some(line),
        end_line,
        state,
        original_file_path,
        original_line,
    }
}

fn changed_range(
    changes: &[ChangedFile],
    path: &Utf8Path,
    start: usize,
    end: usize,
    current: bool,
) -> bool {
    changes.iter().any(|change| {
        let relevant_path = if current {
            &change.new_path
        } else {
            &change.old_path
        };
        relevant_path == path
            && if current {
                &change.new_lines
            } else {
                &change.old_lines
            }
            .iter()
            .any(|span| span.intersects(start, end))
    })
}

#[allow(clippy::too_many_arguments)]
fn moved_site_content_changed(
    current_path: &Utf8Path,
    current_line: usize,
    current_end: Option<usize>,
    original_path: Option<&Utf8Path>,
    original_line: Option<usize>,
    base: &ScannedCoverage,
    head: &ScannedCoverage,
) -> bool {
    let Some(original_line) = original_line else {
        return false;
    };
    let original_path = original_path.unwrap_or(current_path);
    let original_end = base.site_end_line(original_path, original_line);
    site_text(base, original_path, original_line, original_end)
        != site_text(head, current_path, current_line, current_end)
}

fn site_text(
    scan: &ScannedCoverage,
    path: &Utf8Path,
    start: usize,
    end: Option<usize>,
) -> Option<String> {
    let content = scan
        .scanned_files
        .iter()
        .find(|file| file.file_path == path)?
        .content
        .lines()
        .skip(start.saturating_sub(1))
        .take(end.unwrap_or(start).saturating_sub(start) + 1)
        .collect::<Vec<_>>()
        .join("\n");
    Some(content)
}

fn summarize(sites: &[EvidenceDiffSite]) -> EvidenceDiffSummary {
    let mut summary = EvidenceDiffSummary {
        total_sites: sites.len(),
        ..EvidenceDiffSummary::default()
    };
    for site in sites {
        match site.state {
            EvidenceDiffState::Untouched => summary.untouched += 1,
            EvidenceDiffState::Touched => summary.touched += 1,
            EvidenceDiffState::Moved => summary.moved += 1,
            EvidenceDiffState::Gone => summary.gone += 1,
        }
    }
    summary
}

const fn kind_rank(kind: EvidenceSiteKind) -> u8 {
    match kind {
        EvidenceSiteKind::RuleBinding => 0,
        EvidenceSiteKind::Verification => 1,
        EvidenceSiteKind::Annotation => 2,
        EvidenceSiteKind::SourceReference => 3,
    }
}
