use super::git::{ChangedFile, RevisionFile};
use crate::handlers::coverage::anchors;
use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::coverage::{
    AnchorState, AnnotationResult, BindingResult, CoverageReport, CoverageScan, EvidenceDiffReport,
    EvidenceDiffSite, EvidenceDiffState, EvidenceDiffSummary, EvidenceSiteKind, ScannedFile,
};
use provenance_store::cache::GraphEvidence;
use std::collections::{BTreeMap, BTreeSet};

mod source_refs;
mod typed_bindings;

struct RevisionScan {
    coverage: CoverageScan,
    spans: BTreeMap<(Utf8PathBuf, usize), usize>,
}

pub fn report(
    repo: &Utf8Path,
    base: String,
    head: String,
    base_files: Vec<RevisionFile>,
    head_files: Vec<RevisionFile>,
    changes: &[ChangedFile],
    graph: &GraphEvidence,
) -> EvidenceDiffReport {
    let base_scan = scan_revision(&base, base_files);
    let mut head_scan = scan_revision(&head, head_files);
    anchors::reconcile(
        &mut head_scan.coverage,
        &base_scan.coverage,
        repo,
        repo,
        false,
    );
    let mut sites = marker_sites(&base_scan, &head_scan, changes, &graph.rule_ids);
    let typed_sites = typed_bindings::sites(&sites, &head_scan.coverage, graph, changes);
    sites.extend(typed_sites);
    sites.extend(graph.references.iter().flat_map(|reference| {
        source_refs::sites(reference, &head_scan.coverage.bindings, changes)
    }));
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

fn scan_revision(commit: &str, files: Vec<RevisionFile>) -> RevisionScan {
    let scans = files
        .iter()
        .map(|file| {
            let language = file
                .path
                .extension()
                .and_then(provenance_scanner::Language::from_extension)
                .expect("revision files were filtered by scanner language");
            provenance_scanner::scan_file(&file.path, language, &file.content)
        })
        .collect::<Vec<_>>();
    let annotations = scans
        .iter()
        .flat_map(|scan| &scan.annotations)
        .map(|site| AnnotationResult {
            rule_id: site.annotation.rule.clone(),
            file_path: site.file_path.clone(),
            line: site.line,
            function_name: site.function_name.clone(),
            coverage: site.annotation.coverage.to_string(),
            confidence: site.annotation.confidence,
            verification: site
                .annotation
                .verification
                .map(|method| method.to_string()),
            anchor: Some(site.anchor.clone()),
            anchor_state: AnchorState::New,
            original_line: None,
            original_file_path: None,
        })
        .collect();
    let bindings = scans
        .iter()
        .flat_map(|scan| &scan.bindings)
        .map(|site| BindingResult {
            rule_id: site.rule_id.clone(),
            file_path: site.file_path.clone(),
            line: site.line,
            item_name: site.item_name.clone(),
            verification: site.verification.map(|method| method.to_string()),
            anchor: Some(site.anchor.clone()),
            anchor_state: AnchorState::New,
            original_line: None,
            original_file_path: None,
        })
        .collect();
    let extents = site_spans(&files, &scans);
    let scanned_files = files
        .into_iter()
        .map(|file| ScannedFile {
            file_path: file.path,
            content: file.content,
        })
        .collect();
    RevisionScan {
        coverage: CoverageScan {
            report: CoverageReport::new(
                Some(commit.to_string()),
                scans.len(),
                annotations,
                bindings,
                Vec::new(),
            ),
            scanned_files,
        },
        spans: extents,
    }
}

fn site_spans(
    files: &[RevisionFile],
    scans: &[provenance_scanner::FileScan],
) -> BTreeMap<(Utf8PathBuf, usize), usize> {
    let contents = files
        .iter()
        .map(|file| {
            (
                file.path.as_path(),
                file.content.lines().collect::<Vec<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut extents = BTreeMap::new();
    for scan in scans {
        let lines = &contents[scan.file_path.as_path()];
        for site in &scan.annotations {
            let end = symbol_end(
                lines,
                site.line,
                site.function_name.as_deref(),
                scan.language,
            );
            extents.insert((site.file_path.clone(), site.line), end);
        }
        for site in &scan.bindings {
            let end = symbol_end(lines, site.line, site.item_name.as_deref(), scan.language);
            extents.insert((site.file_path.clone(), site.line), end);
        }
    }
    extents
}

fn symbol_end(
    lines: &[&str],
    marker_line: usize,
    symbol: Option<&str>,
    language: provenance_scanner::Language,
) -> usize {
    let Some(symbol) = symbol else {
        return marker_line;
    };
    let marker_index = marker_line.saturating_sub(1);
    let declaration = lines
        .iter()
        .enumerate()
        .skip(marker_index)
        .take(8)
        .find(|(_, line)| line.contains(symbol))
        .map(|(index, _)| index);
    let Some(declaration) = declaration else {
        return marker_line;
    };
    if language == provenance_scanner::Language::Python {
        return python_symbol_end(lines, marker_line, declaration);
    }
    brace_symbol_end(lines, marker_line, declaration)
}

fn brace_symbol_end(lines: &[&str], marker_line: usize, declaration: usize) -> usize {
    let mut depth = 0usize;
    let mut opened = false;
    for (index, line) in lines.iter().enumerate().skip(declaration) {
        for character in line.chars() {
            match character {
                '{' => {
                    opened = true;
                    depth += 1;
                }
                '}' if opened => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
        if (opened && depth == 0) || (!opened && line.trim_end().ends_with(';')) {
            return index + 1;
        }
    }
    marker_line.max(declaration + 1)
}

fn python_symbol_end(lines: &[&str], marker_line: usize, declaration: usize) -> usize {
    let indentation = lines[declaration]
        .chars()
        .take_while(|character| character.is_whitespace())
        .count();
    let mut end = declaration + 1;
    for (index, line) in lines.iter().enumerate().skip(declaration + 1) {
        if line.trim().is_empty() {
            continue;
        }
        let current = line
            .chars()
            .take_while(|character| character.is_whitespace())
            .count();
        if current <= indentation {
            break;
        }
        end = index + 1;
    }
    marker_line.max(end)
}

fn marker_sites(
    base: &RevisionScan,
    head: &RevisionScan,
    changes: &[ChangedFile],
    known_rules: &BTreeSet<String>,
) -> Vec<EvidenceDiffSite> {
    head.coverage
        .annotations
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
                site.original_file_path.clone(),
                site.original_line,
                base,
                head,
                changes,
            )
        })
        .chain(
            head.coverage
                .bindings
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
    original_file_path: Option<Utf8PathBuf>,
    original_line: Option<usize>,
    base: &RevisionScan,
    head: &RevisionScan,
    changes: &[ChangedFile],
) -> EvidenceDiffSite {
    let current = anchor_state != AnchorState::Gone;
    let spans = if current { &head.spans } else { &base.spans };
    let end_line = spans.get(&(path.to_path_buf(), line)).copied();
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
    base: &RevisionScan,
    head: &RevisionScan,
) -> bool {
    let Some(original_line) = original_line else {
        return false;
    };
    let original_path = original_path.unwrap_or(current_path);
    let original_end = base
        .spans
        .get(&(original_path.to_path_buf(), original_line))
        .copied();
    site_text(base, original_path, original_line, original_end)
        != site_text(head, current_path, current_line, current_end)
}

fn site_text(
    scan: &RevisionScan,
    path: &Utf8Path,
    start: usize,
    end: Option<usize>,
) -> Option<String> {
    let content = scan
        .coverage
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
