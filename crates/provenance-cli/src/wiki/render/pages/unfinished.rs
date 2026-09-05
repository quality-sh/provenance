use crate::wiki::model::{PageKind, PageLink, UnfinishedPage};
use crate::wiki::routes::WikiRoute;
use provenance_core::QuestionStatus;
use std::fmt::Write as _;

use super::super::chrome::{container_html, index_breadcrumb, page_shell, title_row};
use super::super::citations::{gap_links, push_gap_citations};
use super::super::html::{escape_html, PageLinksRenderer};
use super::super::labels::counted;

pub fn render_unfinished(scope: &str, page: &UnfinishedPage) -> String {
    let mut main = String::new();
    let links = PageLinksRenderer::new(
        gap_links(&page.gaps)
            .into_iter()
            .chain(orphan_links(page))
            .chain(
                page.open_questions
                    .iter()
                    .filter_map(|question| question.requirement.as_ref()),
            ),
    );
    push_gaps(&mut main, page, &links);
    push_orphans(&mut main, page, &links);
    push_open_questions(&mut main, page, &links);
    let margin = format!(
        "<h3 class=\"margin-head\">Unfinished</h3><p class=\"prose\">{}</p>",
        counted(page.item_count(), "unfinished item", "unfinished items")
    );
    let container = container_html(
        Some((
            PageKind::ScopeIndex,
            (WikiRoute::Index.path(), scope.to_string()),
        )),
        &title_row(PageKind::Unfinished, &page.title, None, &[], &page.scope),
        &main,
        &margin,
    );
    page_shell(
        scope,
        "unfinished",
        &page.title,
        &index_breadcrumb(scope),
        &container,
        "",
    )
}

fn push_gaps(html: &mut String, page: &UnfinishedPage, links: &PageLinksRenderer) {
    html.push_str("<section><h2>Gaps</h2>\n");
    if page.gaps.is_empty() {
        html.push_str("<p class=\"empty-note\">No gaps are recorded.</p>\n");
    } else {
        push_gap_citations(html, links, &page.gaps);
    }
    html.push_str("</section>\n");
}

fn push_orphans(html: &mut String, page: &UnfinishedPage, links: &PageLinksRenderer) {
    html.push_str("<section><h2>Orphans</h2>\n");
    if orphan_links(page).is_empty() {
        html.push_str("<p class=\"empty-note\">No orphan records are present.</p>\n");
        html.push_str("</section>\n");
        return;
    }
    html.push_str("<ul class=\"link-list\">\n");
    for orphan in &page.orphans.sources {
        writeln!(
            html,
            "<li>{} — {}</li>",
            links.link(&orphan.link, None),
            escape_html(&orphan.reason)
        )
        .expect("writing to a String should not fail");
    }
    html.push_str("</ul>\n</section>\n");
}

fn orphan_links(page: &UnfinishedPage) -> Vec<&PageLink> {
    page.orphans
        .sources
        .iter()
        .map(|orphan| &orphan.link)
        .collect()
}

fn push_open_questions(html: &mut String, page: &UnfinishedPage, links: &PageLinksRenderer) {
    html.push_str("<section><h2>Open questions</h2>\n");
    if page.open_questions.is_empty() {
        html.push_str("<p class=\"empty-note\">No open questions are recorded.</p>\n");
        html.push_str("</section>\n");
        return;
    }
    html.push_str("<ul class=\"question-list\">\n");
    for question in &page.open_questions {
        let status = match question.status {
            QuestionStatus::Open => "Open",
            QuestionStatus::BlockedOnHuman => "Waiting for a person",
            QuestionStatus::Answered => "Answered",
        };
        write!(
            html,
            "<li><strong>{}</strong> <span class=\"status-badge\">{status}</span>",
            escape_html(&question.question)
        )
        .expect("writing to a String should not fail");
        if let Some(requirement) = &question.requirement {
            write!(html, "<p>For {}</p>", links.link(requirement, None))
                .expect("writing to a String should not fail");
        }
        html.push_str("</li>\n");
    }
    html.push_str("</ul>\n</section>\n");
}
