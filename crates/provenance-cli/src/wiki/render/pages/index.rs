use crate::wiki::model::{PageKind, ScopeIndexPage, HOMEPAGE_DOMAIN_ROW_CAP};
use crate::wiki::routes::{domain_fragment, WikiRoute};
use provenance_macros::rule;
use std::fmt::Write as _;

use super::super::chrome::{container_html, page_shell, title_row};
use super::super::fragments::push_classification_row;
use super::super::html::{escape_attr, escape_html};
use super::super::labels::counted;

const RATIFIED_VIEWPORTS: [(u16, u16); 2] = [(1440, 900), (390, 844)];

/// Decides whether homepage HTML keeps the ratified entry order at a ratified viewport.
pub fn homepage_entry_order_is_valid(html: &str, width: u16, height: u16) -> bool {
    if !RATIFIED_VIEWPORTS.contains(&(width, height)) {
        return false;
    }
    let Some(search) = html.find("role=\"search\"") else {
        return false;
    };
    let Some(domains) = html.find("data-homepage-domains") else {
        return false;
    };
    let Some(traceability) = html.find("data-homepage-traceability") else {
        return false;
    };
    search < domains
        && domains < traceability
        && html
            .match_indices("data-homepage-domain-row")
            .all(|(position, _)| domains < position && position < traceability)
}

/// Checks rendered homepage HTML for the ratified everyday reader labels.
pub fn homepage_plain_language_is_valid(html: &str) -> bool {
    html.contains("<h2>Search the documentation</h2>")
        && html.contains("<h3 class=\"margin-head\">Project records</h3>")
        && html.contains("<section data-homepage-traceability><h2>Unfinished</h2>")
        && !html.contains("<h2>Atlas</h2>")
        && !html.contains("<h3 class=\"margin-head\">Corpus</h3>")
}

fn traceability_summary_html(unfinished_count: usize) -> String {
    format!(
        "<section data-homepage-traceability><h2>Unfinished</h2>\n\
         <p>See all gaps, orphans, and open questions in one place.</p>\n\
         <a class=\"traceability-link\" href=\"{}\">Review {}</a></section>\n",
        WikiRoute::Unfinished.path(),
        counted(unfinished_count, "unfinished item", "unfinished items"),
    )
}

/// Renders homepage content with one exact unfinished-items link and no individual gap cards.
#[rule("rule_wiki_homepage_traceability_summary")]
#[rule("rule_wiki_homepage_entry_order")]
fn homepage_content_html(page: &ScopeIndexPage) -> String {
    let mut main = String::new();
    push_search(
        &mut main,
        &page.search_coverage,
        page.search_example.as_deref(),
    );
    push_domains(&mut main, page);
    main.push_str(&traceability_summary_html(page.unfinished_count));
    main
}

/// Renders the search-first scope homepage using everyday reader language.
#[rule("rule_wiki_homepage_plain_language")]
pub fn render_index(scope: &str, page: &ScopeIndexPage) -> String {
    let main = homepage_content_html(page);

    let mut margin = String::new();
    margin.push_str(
        "<div class=\"classification\" data-record-counts>\n\
         <h3 class=\"margin-head\">Project records</h3>\n",
    );
    push_classification_row(
        &mut margin,
        "i-git-branch",
        "Requirements",
        &page.counts.requirements.to_string(),
        false,
    );
    push_classification_row(
        &mut margin,
        "i-scale",
        "Decisions",
        &page.counts.resolutions.to_string(),
        false,
    );
    push_classification_row(
        &mut margin,
        "i-shield",
        "Rules",
        &page.counts.rules.to_string(),
        false,
    );
    push_classification_row(
        &mut margin,
        "i-book-open",
        "Sources",
        &page.counts.sources.to_string(),
        false,
    );
    margin.push_str("</div>\n");

    let container = container_html(
        None,
        &title_row(PageKind::ScopeIndex, &page.title, None, &[], &page.scope),
        &main,
        &margin,
    );
    let html = page_shell(scope, "scope-index", &page.title, "", &container, "");
    debug_assert!(homepage_plain_language_is_valid(&html));
    debug_assert!(RATIFIED_VIEWPORTS
        .iter()
        .all(|&(width, height)| homepage_entry_order_is_valid(&html, width, height)));
    html
}

fn push_search(html: &mut String, coverage: &str, example: Option<&str>) {
    let placeholder = example.map_or_else(String::new, |example| {
        format!(" placeholder=\"e.g. {}\"", escape_attr(example))
    });
    write!(
        html,
        "<section class=\"homepage-search\"><h2>Search the documentation</h2>\n\
         <p>Find the project records you need without browsing every area.</p>\n\
         <form class=\"search-box\" role=\"search\" action=\"{}\" method=\"get\">\n\
         <label for=\"homepage-search\">Search by title or text</label>\n\
         <div><input id=\"homepage-search\" name=\"q\" type=\"search\" autocomplete=\"off\"{}><button type=\"submit\">Search</button></div>\n\
         </form><p class=\"search-summary\">{}</p></section>\n",
        WikiRoute::Search.path(),
        placeholder,
        escape_html(coverage),
    )
    .expect("writing to a String should not fail");
}

fn push_domains(html: &mut String, page: &ScopeIndexPage) {
    html.push_str("<section data-homepage-domains><h2>Browse by area</h2>\n");
    if page.domains.is_empty() {
        html.push_str("<p class=\"empty-note\">No project areas have been described yet.</p>\n");
    }
    for domain in page.domains.iter().take(HOMEPAGE_DOMAIN_ROW_CAP) {
        writeln!(
            html,
            "<article class=\"domain-group\" data-homepage-domain-row><h3><a href=\"{}\">{}</a></h3>",
            escape_attr(&domain_fragment(&domain.id)),
            escape_html(&domain.name),
        )
        .expect("writing to a String should not fail");
        if let Some(description) = &domain.description {
            writeln!(html, "<p class=\"prose\">{}</p>", escape_html(description))
                .expect("writing to a String should not fail");
        }
        writeln!(
            html,
            "<p class=\"entry-counts\">{} · {}</p></article>",
            counted(domain.requirements, "requirement", "requirements"),
            counted(domain.rules, "rule", "rules"),
        )
        .expect("writing to a String should not fail");
    }
    // With no authored areas the browse link would promise content the
    // domains page does not hold, so the empty note stands alone.
    let browse_label = match page.authored_domain_count {
        0 => None,
        1 => Some("Browse 1 area".to_string()),
        count => Some(format!("Browse all {count} areas")),
    };
    if let Some(browse_label) = browse_label {
        writeln!(
            html,
            "<a class=\"browse-all\" href=\"{}\">{}</a></section>",
            WikiRoute::Domains.path(),
            escape_html(&browse_label),
        )
        .expect("writing to a String should not fail");
    } else {
        html.push_str("</section>\n");
    }
}
