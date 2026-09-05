use crate::wiki::model::{PageKind, PageLink, RequirementPage};
use std::fmt::Write as _;

use super::super::chrome::{breadcrumb_from_lineage, container_html, page_shell, title_row};
use super::super::citations::{gap_links, push_gap_citations, push_source_citations};
use super::super::field_notes::field_notes;
use super::super::fragments::{
    push_attribution, push_classification_block, push_classification_link_row,
    push_classification_row, push_decision_sections, push_lineage, push_prose_section,
    push_rule_territory_card, push_section_open,
};
use super::super::html::{escape_attr, escape_html, icon_svg, PageLinksRenderer};
use super::super::labels::{format_confidence, requirement_status_badge, resolution_status_word};

/// Every titled link this page renders, in reading order: the breadcrumb and
/// lineage, the resolving decisions, the refinements, the related
/// requirements, the superseded and depending requirements, and the source
/// citations.
fn page_links(page: &RequirementPage) -> Vec<&PageLink> {
    let mut links: Vec<&PageLink> = page.lineage.iter().map(|entry| &entry.link).collect();
    links.extend(page.decisions.iter().map(|decision| &decision.link));
    links.extend(&page.children);
    links.extend(&page.siblings);
    links.extend(&page.supersedes);
    links.extend(&page.depends_on);
    links.extend(page.superseded_by.iter());
    links.extend(page.produced_rules.iter().map(|rule| &rule.link));
    links.extend(page.sources.iter().map(|citation| &citation.link));
    links.extend(gap_links(&page.gaps));
    links
}

/// Renders a requirement detail page.
#[allow(clippy::too_many_lines)]
pub fn render_requirement(scope: &str, page: &RequirementPage) -> String {
    let links = PageLinksRenderer::new(page_links(page));
    let mut main = String::new();
    push_prose_section(
        &mut main,
        "sh-requirement",
        None,
        "Statement",
        &page.statement,
    );
    if let Some(description) = &page.description {
        push_prose_section(
            &mut main,
            "sh-requirement",
            None,
            "Description",
            description,
        );
    }
    if let Some(fog) = &page.fog {
        push_prose_section(&mut main, "", None, "Fog", fog);
    }
    for decision in &page.decisions {
        push_decision_sections(&mut main, &links, decision);
    }
    if !page.produced_rules.is_empty() {
        push_section_open(&mut main, "sh-rule", Some("i-shield"), "Produced rules");
        main.push_str("<div class=\"territory\">\n");
        push_rule_territory_card(&mut main, &links, &page.produced_rules);
        main.push_str("</div>\n</section>\n");
    }
    if !page.children.is_empty() {
        push_section_open(
            &mut main,
            "sh-requirement",
            Some("i-git-branch"),
            "Refined Into",
        );
        main.push_str("<div class=\"territory\">\n<div class=\"territory-card requirement\">\n");
        writeln!(
            main,
            "<div class=\"card-head\">{}Refinements — {}</div>",
            icon_svg("i-git-branch"),
            page.children.len()
        )
        .expect("writing to a String should not fail");
        main.push_str(&links.link_list(&page.children));
        main.push_str("</div>\n</div>\n</section>\n");
    }
    for decision in &page.decisions {
        push_attribution(
            &mut main,
            decision.made_by.as_deref(),
            decision.approved_by.as_deref(),
            decision.approved_at,
            resolution_status_word(&decision.status),
        );
    }
    if !page.siblings.is_empty() {
        push_section_open(&mut main, "sh-requirement", Some("i-git-branch"), "Related");
        main.push_str("<div class=\"territory\">\n<div class=\"territory-card requirement\">\n");
        writeln!(
            main,
            "<div class=\"card-head\">{}Related Requirements — {}</div>",
            icon_svg("i-git-branch"),
            page.siblings.len()
        )
        .expect("writing to a String should not fail");
        main.push_str(&links.link_list(&page.siblings));
        main.push_str("</div>\n</div>\n</section>\n");
    }

    let mut margin = String::new();
    if !page.sources.is_empty() {
        margin.push_str("<h3 class=\"margin-head\">Sources</h3>\n");
        push_source_citations(&mut margin, &links, &page.sources);
    }
    if !page.gaps.is_empty() {
        margin.push_str("<h3 class=\"margin-head\">Gaps</h3>\n");
        push_gap_citations(&mut margin, &links, &page.gaps);
    }
    let mut rows = String::new();
    if let Some(domain_id) = &page.domain_id {
        if page.domain_has_anchor {
            writeln!(
                rows,
                "<div class=\"row\">{}<span class=\"k\">Domain</span>\
                 <a class=\"v mono\" href=\"{}\">{}</a></div>",
                icon_svg("i-git-branch"),
                escape_attr(&crate::wiki::routes::domain_fragment(domain_id)),
                escape_html(domain_id),
            )
            .expect("writing to a String should not fail");
        } else {
            push_classification_row(&mut rows, "i-git-branch", "Domain", domain_id, true);
        }
    }
    for link in &page.supersedes {
        push_classification_link_row(&mut rows, &links, "i-scale", "Supersedes", link);
    }
    for link in &page.depends_on {
        push_classification_link_row(&mut rows, &links, "i-git-branch", "Depends on", link);
    }
    if let Some(superseded_by) = &page.superseded_by {
        push_classification_link_row(&mut rows, &links, "i-scale", "Superseded by", superseded_by);
    }
    for decision in &page.decisions {
        if let Some(enforcement) = &decision.enforcement {
            push_classification_row(&mut rows, "i-shield", "Enforcement", enforcement, false);
        }
        if let Some(confidence) = decision.confidence {
            push_classification_row(
                &mut rows,
                "i-gauge",
                "Confidence",
                &format_confidence(confidence),
                false,
            );
        }
        push_classification_row(
            &mut rows,
            "i-scale",
            "Decision",
            &decision.link.target.record_id,
            true,
        );
    }
    push_classification_block(&mut margin, &rows);
    push_lineage(&mut margin, &links, &page.lineage);

    let back = page.back_link.as_ref().map_or_else(
        || {
            (
                crate::wiki::routes::WikiRoute::Index.path(),
                scope.to_string(),
            )
        },
        |link| {
            (
                crate::wiki::routes::WikiRoute::Record(&link.target).path(),
                link.title.clone(),
            )
        },
    );
    let container = container_html(
        Some((PageKind::Requirement, back)),
        &title_row(
            PageKind::Requirement,
            &page.title,
            Some(&requirement_status_badge(
                &page.status,
                page.decisions.len(),
                page.produced_rules.len(),
            )),
            &[],
            &page.id.record_id,
        ),
        &main,
        &margin,
    );
    page_shell(
        scope,
        "requirement",
        &page.title,
        &breadcrumb_from_lineage(&links, &page.lineage),
        &container,
        &field_notes(&page.threads, &page.id),
    )
}
