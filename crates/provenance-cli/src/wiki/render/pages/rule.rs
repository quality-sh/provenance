use crate::wiki::model::{CodeScan, PageKind, PageLink, RulePage};
use std::fmt::Write as _;

use super::super::chrome::{container_html, index_breadcrumb, page_shell, title_row};
use super::super::citations::{gap_links, push_gap_citations};
use super::super::field_notes::field_notes;
use super::super::fragments::{
    push_classification_block, push_classification_row, push_prose_section, push_section_open,
};
use super::super::html::{escape_html, evidence_html, PageLinksRenderer};
use super::super::labels::{rule_status_word, sev_chip, severity_word, status_badge};

/// Every titled link this page renders: the records that produce the rule,
/// the requirements upstream of those, and the sources behind them.
fn page_links(page: &RulePage) -> Vec<&PageLink> {
    let mut links: Vec<&PageLink> = page.produced_by.iter().collect();
    links.extend(&page.requirements);
    links.extend(&page.sources);
    links.extend(gap_links(&page.gaps));
    links
}

fn provenance_links(page: &RulePage) -> Vec<PageLink> {
    let mut links = page.produced_by.clone();
    for requirement in &page.requirements {
        if !links.iter().any(|link| link.target == requirement.target) {
            links.push(requirement.clone());
        }
    }
    links
}

/// The binding sections keep canonical relationships visible while making the
/// absence of scanner evidence explicit.
fn push_code_scan(main: &mut String, page: &RulePage) {
    let Some(scan) = &page.code_scan else {
        if page.implementations.is_empty() && page.verifications.is_empty() {
            main.push_str(
                "<section>\n<p class=\"scan-note\">No code scan was supplied to this build; \
                 bindings and verification are not shown.</p>\n</section>\n",
            );
        } else {
            if !page.implementations.is_empty() {
                push_implementations(main, page);
            }
            main.push_str("<section>\n<p class=\"scan-note\">No code scan was supplied to this build; scanner-discovered bindings are not shown.</p>\n</section>\n");
            if !page.verifications.is_empty() {
                push_verifications(main, page, None);
            }
        }
        return;
    };
    push_implementations(main, page);
    push_verifications(main, page, Some(scan));
}

/// Which code the sections above describe, so a reader can tell a scan of
/// today's tree from one taken several commits ago.
fn push_scan_note(main: &mut String, scan: &CodeScan) {
    if let Some(commit) = &scan.commit {
        writeln!(
            main,
            "<p class=\"scan-note\">Code scan at commit <code>{}</code></p>",
            escape_html(commit.get(..7).unwrap_or(commit)),
        )
        .expect("writing to a String should not fail");
    } else {
        main.push_str("<p class=\"scan-note\">Code scan of an uncommitted working tree</p>\n");
    }
}

fn push_implementations(main: &mut String, page: &RulePage) {
    push_section_open(main, "sh-rule", None, "Implementation");
    if page.implementations.is_empty() {
        main.push_str("<p class=\"empty-state\">No implementation bound</p>\n");
    } else {
        for binding in &page.implementations {
            writeln!(
                main,
                "<p class=\"code-site\"><code>{}</code><span class=\"site-separator\"> — </span>{}</p>",
                escape_html(binding.symbol.as_deref().unwrap_or("Unknown symbol")),
                evidence_html(&binding.location),
            )
            .expect("writing to a String should not fail");
        }
    }
    main.push_str("</section>\n");
}

fn push_verifications(main: &mut String, page: &RulePage, scan: Option<&CodeScan>) {
    push_section_open(main, "sh-rule", Some("i-check-circle"), "Verification");
    if page.verifications.is_empty() {
        main.push_str("<p class=\"empty-state\">Not verified</p>\n");
    } else {
        main.push_str("<ul class=\"verification-list\">\n");
        for site in &page.verifications {
            let outside = if site.outside_implementation_module {
                " <span class=\"site-note\">outside implementation module</span>"
            } else {
                ""
            };
            writeln!(
                main,
                "<li><span class=\"verification-method\">{}</span> <code>{}</code><span class=\"site-separator\"> — </span>{}{outside}</li>",
                escape_html(&site.method),
                escape_html(site.symbol.as_deref().unwrap_or("Unknown symbol")),
                evidence_html(&site.location),
            )
            .expect("writing to a String should not fail");
        }
        main.push_str("</ul>\n");
    }
    if let Some(scan) = scan {
        push_scan_note(main, scan);
    }
    main.push_str("</section>\n");
}

/// Renders a rule detail page.
#[allow(clippy::too_many_lines)]
pub fn render_rule(scope: &str, page: &RulePage) -> String {
    let links = PageLinksRenderer::new(page_links(page));
    let mut main = String::new();
    push_prose_section(&mut main, "sh-rule", None, "Statement", &page.statement);
    if let Some(description) = &page.description {
        push_prose_section(&mut main, "sh-rule", None, "Description", description);
    }
    push_code_scan(&mut main, page);
    let provenance = provenance_links(page);
    if !provenance.is_empty() {
        push_section_open(
            &mut main,
            "sh-requirement",
            Some("i-git-branch"),
            "Provenance",
        );
        main.push_str(&links.link_list(&provenance));
        main.push_str("</section>\n");
    }
    if !page.sources.is_empty() {
        push_section_open(&mut main, "sh-source", Some("i-book-open"), "Sources");
        main.push_str(&links.link_list(&page.sources));
        main.push_str("</section>\n");
    }

    let mut margin = String::new();
    if !page.gaps.is_empty() {
        margin.push_str("<h3 class=\"margin-head\">Gaps</h3>\n");
        for gap in &page.gaps {
            push_gap_citations(&mut margin, &links, std::slice::from_ref(gap));
        }
    }
    let mut rows = String::new();
    push_classification_row(
        &mut rows,
        "i-gauge",
        "Severity",
        severity_word(&page.severity),
        false,
    );
    push_classification_block(&mut margin, &rows);

    let chips = vec![sev_chip(
        severity_word(&page.severity),
        severity_word(&page.severity),
    )];
    let container = container_html(
        Some((
            PageKind::Rule,
            (
                crate::wiki::routes::WikiRoute::Index.path(),
                scope.to_string(),
            ),
        )),
        &title_row(
            PageKind::Rule,
            &page.title,
            Some(&status_badge(rule_status_word(&page.status))),
            &chips,
            &page.id.record_id,
        ),
        &main,
        &margin,
    );
    page_shell(
        scope,
        "rule",
        &page.title,
        &index_breadcrumb(scope),
        &container,
        &field_notes(&page.threads, &page.id),
    )
}
