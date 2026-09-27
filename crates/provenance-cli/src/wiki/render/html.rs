#[cfg(test)]
use crate::wiki::links::InlineRef;
use crate::wiki::links::{EvidenceRef, EvidenceSnippet};
use crate::wiki::model::{PageId, PageLink};
use provenance_macros::rule;
use std::collections::HashMap;
use std::fmt::Write as _;

use super::labels::{kind_icon, kind_label};
pub(in crate::wiki::render) use crate::html::{escape_attr, escape_html};

#[cfg(test)]
#[path = "tests/disambiguation.rs"]
mod disambiguation_tests;

/// Renders every titled link on one page.
///
/// Build one per page, from every link the page will render, and hand it to
/// each section. A renderer built per section only knows the titles of that
/// section, so two records sharing a title in different sections would both
/// render bare.
pub(in crate::wiki::render) struct PageLinksRenderer {
    targets_by_title: HashMap<String, Vec<PageId>>,
}

impl PageLinksRenderer {
    pub(in crate::wiki::render) fn new<'a>(links: impl IntoIterator<Item = &'a PageLink>) -> Self {
        let mut targets_by_title: HashMap<String, Vec<PageId>> = HashMap::new();
        for link in links {
            let targets = targets_by_title.entry(link.title.clone()).or_default();
            if !targets.contains(&link.target) {
                targets.push(link.target.clone());
            }
        }
        targets_by_title.retain(|_, targets| targets.len() > 1);
        Self { targets_by_title }
    }

    pub(in crate::wiki::render) fn link(&self, link: &PageLink, class: Option<&str>) -> String {
        let class = class.map_or_else(String::new, |class| {
            format!(" class=\"{}\"", escape_attr(class))
        });
        format!(
            "<a{class} href=\"{}\">{}{}</a>",
            escape_attr(&link.target.route()),
            escape_html(&link.title),
            self.collision_chip(link)
        )
    }

    pub(in crate::wiki::render) fn text(&self, link: &PageLink) -> String {
        format!("{}{}", escape_html(&link.title), self.collision_chip(link))
    }

    /// A reader must never face two visually identical links that mean
    /// different records.
    ///
    /// The whole page is one field of view, so the titles compared here are
    /// every title the page renders, not just the ones beside this link in
    /// its own list. When links on a page carry the same title, each one
    /// shows the shortest ending of its record id that tells it apart from
    /// the others, never fewer than eight characters. When the ids match as
    /// well, the record kind is shown too. A title nothing else on the page
    /// shares shows no chip at all.
    #[rule("rule_ambiguous_links_disambiguated")]
    fn collision_chip(&self, link: &PageLink) -> String {
        let mut html = String::new();
        if let Some(targets) = self.targets_by_title.get(&link.title) {
            let suffix = shortest_distinct_suffix(&link.target, targets);
            let kind = targets.iter().any(|other| {
                other.kind != link.target.kind && other.record_id == link.target.record_id
            });
            write!(
                html,
                " <span class=\"id-chip\">{}{}{}{}</span>",
                if kind {
                    kind_label(link.target.kind.into())
                } else {
                    ""
                },
                if kind { " · " } else { "" },
                if suffix.len() < link.target.record_id.len() {
                    "…"
                } else {
                    ""
                },
                escape_html(suffix)
            )
            .expect("writing to a String should not fail");
        }
        html
    }

    /// One section's links as a list. The renderer still speaks for the whole
    /// page, so a title shared with another section is marked here too.
    pub(in crate::wiki::render) fn link_list(&self, links: &[PageLink]) -> String {
        let mut html = String::from("<ul class=\"link-list\">\n");
        for link in links {
            let icon = if link.target.kind == crate::wiki::model::RecordKind::Rule {
                icon_svg(kind_icon(link.target.kind.into()))
            } else {
                String::new()
            };
            writeln!(html, "<li>{icon}{}</li>", self.link(link, None))
                .expect("writing to a String should not fail");
        }
        html.push_str("</ul>\n");
        html
    }
}

fn shortest_distinct_suffix<'a>(target: &'a PageId, colliding_targets: &[PageId]) -> &'a str {
    let id = &target.record_id;
    let boundaries: Vec<usize> = id.char_indices().map(|(index, _)| index).collect();
    if boundaries.is_empty() {
        return id;
    }
    let minimum_length = 8.min(boundaries.len());
    for length in minimum_length..=boundaries.len() {
        let suffix = &id[boundaries[boundaries.len() - length]..];
        if colliding_targets
            .iter()
            .all(|other| other == target || !other.record_id.ends_with(suffix))
        {
            return suffix;
        }
    }
    id
}

pub(in crate::wiki::render) fn evidence_html(evidence: &EvidenceRef) -> String {
    let mut reference = evidence.href.as_ref().map_or_else(
        || escape_html(&evidence.label),
        |href| {
            format!(
                "<a href=\"{}\">{}</a>",
                escape_attr(href),
                escape_html(&evidence.label)
            )
        },
    );
    if let Some(note) = &evidence.note {
        write!(
            reference,
            " <span class=\"reference-note\">({})</span>",
            escape_html(note)
        )
        .expect("writing to a String should not fail");
    }
    evidence.snippet.as_ref().map_or_else(
        || reference.clone(),
        |snippet| format!("{reference}{}", snippet_html(snippet)),
    )
}

pub(in crate::wiki::render) fn snippet_html(snippet: &EvidenceSnippet) -> String {
    let content = escape_html(&snippet.content).replace('\n', "<br>");
    format!(
        "<span class=\"local-snippet\"><br><span class=\"snippet-label\">Local snippet · {}</span><br><code>{content}</code></span>",
        escape_html(&snippet.label),
    )
}

pub(in crate::wiki::render) fn icon_svg(symbol: &str) -> String {
    format!("<svg class=\"icon\"><use href=\"#{symbol}\"/></svg>")
}

/// Escapes a field-note body while wrapping each [`InlineRef`] span in an
/// anchor. Spans are byte offsets into `body`, non-overlapping and sorted.
#[cfg(test)]
pub(in crate::wiki::render) fn linkify_body(body: &str, refs: &[InlineRef]) -> String {
    let mut html = String::new();
    let mut cursor = 0;
    for inline in refs {
        if inline.start < cursor || inline.end > body.len() {
            continue;
        }
        html.push_str(&escape_html(&body[cursor..inline.start]));
        if let Some(href) = &inline.href {
            write!(
                html,
                "<a class=\"src\" href=\"{}\">{}</a>",
                escape_attr(href),
                escape_html(&inline.label)
            )
            .expect("writing to a String should not fail");
        } else {
            html.push_str(&escape_html(&inline.label));
        }
        cursor = inline.end;
    }
    html.push_str(&escape_html(&body[cursor..]));
    html
}

#[cfg(test)]
mod evidence_tests {
    use super::*;
    use crate::wiki::links::LinkResolver;
    use provenance_core::coverage::{CoverageReport, CoverageScan};

    #[test]
    fn unresolved_reference_renders_as_plain_text_with_an_honest_note() {
        let scan = CoverageScan {
            report: CoverageReport::new(Some("deadbee".into()), 0, vec![], vec![], vec![]),
            scanned_files: vec![],
        };
        let evidence = LinkResolver::new(Some("https://github.com/example/repo.git"))
            .with_coverage(&scan)
            .resolve_at("docs/removed.md", Some("deadbee"));

        let html = evidence_html(&evidence);

        assert_eq!(
            html,
            "docs/removed.md <span class=\"reference-note\">(path not found in the pinned tree)</span>"
        );
        assert!(!html.contains("<a "));
    }

    #[test]
    fn local_file_url_never_renders_as_an_anchor() {
        let evidence = LinkResolver::new(None).resolve("file://docs/guide.md");

        let html = evidence_html(&evidence);

        assert!(html.starts_with("file://docs/guide.md "));
        assert!(!html.contains("<a "));
    }
}
