//! The properties themselves. The site generator, the independent scope
//! restatement, and the independent resolver they read are in the parent
//! module.

use provenance_macros::verifies;

use super::{
    gen_corpus, is_local_markdown_destination, resolve_beside, DocPage, DocsSite, Rng, Verdict,
};
use std::path::PathBuf;

#[test]
#[verifies("rule_docs_links_resolve", property)]
fn every_reported_link_names_a_local_document_the_site_does_not_publish() {
    let mut rng = Rng(0x000d_0c51_0001);
    let mut reported = 0;
    for _ in 0..256 {
        let corpus = gen_corpus(&mut rng);
        for report in corpus.site.broken_links() {
            let page = corpus
                .page(&report.source_path)
                .unwrap_or_else(|| panic!("report came from no page: {report:?}"));
            assert!(
                page.plants
                    .iter()
                    .any(|plant| plant.destination == report.link),
                "report names a link that is not on {}: {report:?}",
                page.file
            );
            assert!(
                is_local_markdown_destination(&report.link),
                "reported a destination that names no local document: {report:?}"
            );
            let resolved = resolve_beside(page.file, &report.link);
            assert_eq!(
                report.resolved_path, resolved,
                "report resolved {} somewhere a reader would not",
                report.link
            );
            assert!(
                !corpus.published.contains(&resolved),
                "reported a link the site publishes a page for: {report:?}"
            );
            reported += 1;
        }
    }
    assert!(
        reported >= 512,
        "only {reported} links were reported; the property proves too little"
    );
}

#[test]
#[verifies("rule_docs_links_resolve", property)]
fn no_resolvable_or_out_of_scope_link_is_ever_reported() {
    let mut rng = Rng(0x000d_0c51_0002);
    let mut spared = 0;
    for _ in 0..256 {
        let corpus = gen_corpus(&mut rng);
        let broken = corpus.site.broken_links();
        for (page, plant, verdict) in corpus.verdicts() {
            let spare = match verdict {
                Verdict::Broken(_) => continue,
                Verdict::PublishedTarget => "the site publishes a page for it",
                Verdict::OutOfScope(reason) => reason,
            };
            assert!(
                !broken
                    .iter()
                    .any(|report| report.source_path == page.file
                        && report.link == plant.destination),
                "reported {} on {}, though {spare}",
                plant.destination,
                page.file
            );
            spared += 1;
        }
    }
    assert!(
        spared >= 512,
        "only {spared} links were left alone; the property proves too little"
    );
}

#[test]
#[verifies("rule_docs_links_resolve", property)]
fn a_site_gets_one_report_for_each_link_planted_broken() {
    let mut rng = Rng(0x000d_0c51_0003);
    let mut planted = 0;
    for _ in 0..256 {
        let corpus = gen_corpus(&mut rng);
        let expected = corpus
            .verdicts()
            .filter(|(_, _, verdict)| matches!(verdict, Verdict::Broken(_)))
            .count();
        let reports = corpus.site.broken_links();
        assert_eq!(
            reports.len(),
            expected,
            "planted {expected} broken links, got {} reports: {reports:?}",
            reports.len()
        );
        planted += expected;
    }
    assert!(
        planted >= 512,
        "only {planted} broken links were planted; the property proves too little"
    );
}

/// A one-page site whose only published page is `docs/index.md`, so every
/// destination naming another document is broken and the report says which
/// destinations were read as links at all.
fn one_page_site(markdown: &str) -> DocsSite {
    DocsSite {
        route_by_path: std::iter::once((PathBuf::from("/site/docs/index.md"), "/".to_string()))
            .collect(),
        page_by_route: std::iter::once(("/".to_string(), 0)).collect(),
        pages: vec![DocPage {
            route: "/".to_string(),
            title: "Page".to_string(),
            source_path: "docs/index.md".to_string(),
            file_path: PathBuf::from("/site/docs/index.md"),
            markdown: markdown.to_string(),
            depth: 0,
        }],
    }
}

fn reported_links(site: &DocsSite) -> Vec<String> {
    site.broken_links()
        .into_iter()
        .map(|link| link.link)
        .collect()
}

#[test]
#[verifies("rule_docs_links_resolve", examples)]
fn sees_reference_style_links_and_leaves_images_alone() {
    let site = one_page_site(
        "# Page\n\nSee [install][ref], [gone][], and ![picture](missing.md).\n\n\
         [ref]: guide/install.md\n[gone]: guide/gone.md\n",
    );

    assert_eq!(
        reported_links(&site),
        vec!["guide/install.md".to_string(), "guide/gone.md".to_string()],
        "reference-style links are read through their definitions, images are not read at all"
    );
}

/// The check and the server read one document one way.
///
/// A footnote is where the two readings could part. To a parser without
/// `ENABLE_FOOTNOTES`, `[^1]` is a shortcut reference link and the block
/// `[^1]: guide/missing.md` below it is its definition, so the check would
/// report a broken link on a page where the server, which does enable
/// footnotes, renders no link at all. Both sides parse with
/// `MARKDOWN_OPTIONS`, so the footnote is a footnote to both and only the
/// ordinary definition beside it is reported.
#[test]
#[verifies("rule_docs_links_resolve", examples)]
fn reads_a_footnote_as_a_footnote_and_not_as_a_link() {
    let site = one_page_site(
        "# Page\n\nA claim.[^1] And [gone][ref].\n\n\
         [^1]: guide/missing.md\n\n[ref]: guide/gone.md\n",
    );

    assert_eq!(
        reported_links(&site),
        vec!["guide/gone.md".to_string()],
        "a footnote definition is a footnote, not a link definition"
    );
}
