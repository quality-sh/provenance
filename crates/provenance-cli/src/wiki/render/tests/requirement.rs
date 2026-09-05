use crate::wiki::model::{LineageEntry, PageKind};
use provenance_macros::verifies;

use super::super::render_requirement;
use super::fixtures::{gappy_requirement_fixture, link, requirement_fixture, SCAN_COMMIT};

#[test]
fn requirement_page_carries_the_mockup_structure() {
    let html = render_requirement("default", &requirement_fixture());
    assert!(html.contains("class=\"accent-bar requirement\""));
    assert!(html.contains("<h1>SaveInvoice shall split each claim item into portions</h1>"));
    assert!(html.contains("type-badge requirement"));
    assert!(html.contains("status-badge discovery"));
    assert!(html.contains("<span class=\"id-chip\">req_saveinvoice_split</span>"));
    assert!(html.contains("Statement"));
    assert!(html.contains("Resolving Decision"));
    assert!(html.contains("blockquote class=\"position\""));
    assert!(html.contains("Produced rules"));
    assert!(html.contains("Discussion"));
    assert!(!html.contains("Downstream Territory"));
    assert!(!html.contains("Field Notes"));
    assert!(html.contains(
        "<a href=\"/rules/rule_sah_inv_016/\">Suppress line emission for fully zero claim items</a>"
    ));
    assert!(!html.contains(">rule_sah_inv_016</a>"));
}

/// The requirement's own `supersedes` and `depends_on`, and the reverse
/// supersession, render as classification rows like the other kinds.
#[test]
fn requirement_page_renders_supersedes_depends_on_and_superseded_by_rows() {
    let page = requirement_fixture();
    let plain = render_requirement("default", &page);
    assert!(!plain.contains(">Supersedes</span>"), "{plain}");
    assert!(!plain.contains(">Superseded by</span>"), "{plain}");

    let mut page = requirement_fixture();
    page.supersedes = vec![link(
        PageKind::Requirement,
        "req_whole_claim",
        "The whole-claim rule",
    )];
    page.depends_on = vec![link(
        PageKind::Requirement,
        "req_claim_lines",
        "Claim lines shall exist",
    )];
    page.superseded_by = Some(link(
        PageKind::Requirement,
        "req_split_v2",
        "The split, second revision",
    ));

    let html = render_requirement("default", &page);

    assert!(html.contains(">Supersedes</span>"), "{html}");
    assert!(html.contains(">Depends on</span>"), "{html}");
    assert!(html.contains(">Superseded by</span>"), "{html}");
    assert!(html.contains(">The whole-claim rule</a>"), "{html}");
    assert!(html.contains(">Claim lines shall exist</a>"), "{html}");
    assert!(html.contains(">The split, second revision</a>"), "{html}");
}

#[test]
fn produced_rule_titles_are_disambiguated_across_the_whole_page() {
    let mut page = requirement_fixture();
    let mut other = page.produced_rules[0].clone();
    other.link.target.record_id = "rule_sah_inv_017".to_string();
    page.produced_rules.push(other);

    let html = render_requirement("default", &page);

    assert!(
        html.contains("<span class=\"id-chip\">…_inv_016</span>"),
        "{html}"
    );
    assert!(
        html.contains("<span class=\"id-chip\">…_inv_017</span>"),
        "{html}"
    );
}

#[test]
fn requirement_page_links_every_code_reference() {
    let html = render_requirement("default", &requirement_fixture());
    // Rule evidence links to the host blob URL with line anchors.
    assert!(html.contains(&format!(
        "https://github.com/exampleorg/ex-api/blob/{SCAN_COMMIT}/src/UseCase.php#L153-L156"
    )));
    // Source citation reference pins to the source commit.
    assert!(html.contains("https://github.com/exampleorg/ex-api/blob/abc1234/docs/award.md"));
    // Field-note bodies linkify code refs and test-case names.
    assert!(html.contains(&format!(
        "https://github.com/exampleorg/ex-api/blob/{SCAN_COMMIT}/src/UseCase.php#L211-L233"
    )));
    assert!(html.contains(">testCreateGapInvoiceOnly</a>"));
}

#[test]
fn requirement_page_numbers_source_citations_in_the_margin() {
    let html = render_requirement("default", &requirement_fixture());
    assert!(html.contains("<span class=\"cite-num\">[1]</span>"));
    assert!(html.contains("<a href=\"/sources/source_schads/\">SCHADS Award mapping</a>"));
    assert!(html.contains("clause 10.3"));
}

#[test]
fn requirement_page_shows_lineage_with_the_current_entry_unlinked() {
    let html = render_requirement("default", &requirement_fixture());
    assert!(html.contains("<a href=\"/requirements/req_platform/\">ExampleOrg platform</a>"));
    assert!(html.contains("<li class=\"current\">SaveInvoice shall split each claim item"));
}

#[test]
#[verifies("rule_ambiguous_links_disambiguated", examples)]
fn requirement_page_disambiguates_colliding_refined_into_links() {
    let mut page = requirement_fixture();
    page.children = super::fixtures::colliding_requirement_links();

    let html = render_requirement("default", &page);
    assert!(html.contains(">Refined Into</h2>"));
    assert!(html.contains(
        "<a href=\"/requirements/req_sah_participant_budget_summary_shall_pro/\">Participant budget summary shall pro-rate services <span class=\"id-chip\">…hall_pro</span></a>"
    ));
    assert!(html.contains(
        "<a href=\"/requirements/req_sah_participant_budget_summary_shall_pro_2/\">Participant budget summary shall pro-rate services <span class=\"id-chip\">…ll_pro_2</span></a>"
    ));
}

/// Refined Into and Related are separate lists. A renderer built per list
/// would find each title uncontested and mark neither link.
#[test]
#[verifies("rule_ambiguous_links_disambiguated", examples)]
fn requirement_page_disambiguates_a_title_split_across_two_sections() {
    let mut page = requirement_fixture();
    let mut colliding = super::fixtures::colliding_requirement_links();
    page.siblings = vec![colliding.remove(1)];
    page.children = colliding;

    let html = render_requirement("default", &page);
    assert!(html.contains(">Refined Into</h2>"));
    assert!(html.contains(">Related</h2>"));
    assert!(html.contains(
        "<a href=\"/requirements/req_sah_participant_budget_summary_shall_pro/\">Participant budget summary shall pro-rate services <span class=\"id-chip\">…hall_pro</span></a>"
    ));
    assert!(html.contains(
        "<a href=\"/requirements/req_sah_participant_budget_summary_shall_pro_2/\">Participant budget summary shall pro-rate services <span class=\"id-chip\">…ll_pro_2</span></a>"
    ));
}

/// The resolving decision, the source citation and the refinements are three
/// different parts of the page; a title shared between them still reads apart.
#[test]
#[verifies("rule_ambiguous_links_disambiguated", examples)]
fn requirement_page_disambiguates_a_decision_a_source_and_a_child_sharing_a_title() {
    let mut page = requirement_fixture();
    "Shared title".clone_into(&mut page.decisions[0].link.title);
    "Shared title".clone_into(&mut page.sources[0].link.title);
    page.children = vec![link(
        PageKind::Requirement,
        "req_shared_child",
        "Shared title",
    )];

    let html = render_requirement("default", &page);
    assert!(html.contains(
        "<h3 class=\"decision-title\"><a href=\"/resolutions/res_split/\">Shared title <span class=\"id-chip\">…es_split</span></a></h3>"
    ));
    assert!(html.contains(
        "<a href=\"/sources/source_schads/\">Shared title <span class=\"id-chip\">…e_schads</span></a>"
    ));
    assert!(html.contains(
        "<a href=\"/requirements/req_shared_child/\">Shared title <span class=\"id-chip\">…ed_child</span></a>"
    ));
}

#[test]
#[verifies("rule_ambiguous_links_disambiguated", examples)]
fn requirement_page_keeps_unique_refined_into_markup_unchanged() {
    let mut page = requirement_fixture();
    page.children = super::fixtures::unique_requirement_links();

    let html = render_requirement("default", &page);
    assert!(html.contains(
        "<ul class=\"link-list\">\n<li><a href=\"/requirements/req_budget_split/\">Budget portions shall reconcile</a></li>\n<li><a href=\"/requirements/req_zero_suppression/\">Zero claim items shall be suppressed</a></li>\n</ul>"
    ));
}

#[test]
#[verifies("rule_ambiguous_links_disambiguated", examples)]
fn lineage_and_breadcrumb_disambiguate_collisions_including_current_text() {
    let mut page = requirement_fixture();
    page.lineage = vec![
        LineageEntry {
            link: link(PageKind::Requirement, "req_shared_parent", "Shared title"),
            is_current: false,
        },
        LineageEntry {
            link: link(PageKind::Requirement, "req_shared_child", "Shared title"),
            is_current: true,
        },
    ];

    let html = render_requirement("default", &page);
    assert!(html.contains(
        "<li><a href=\"/requirements/req_shared_parent/\">Shared title <span class=\"id-chip\">…d_parent</span></a></li>"
    ));
    assert!(html.contains(
        "<li class=\"current\">Shared title <span class=\"id-chip\">…ed_child</span></li>"
    ));
    assert!(html.contains(
        "<nav aria-label=\"Breadcrumb\"><a href=\"/requirements/req_shared_parent/\">Shared title <span class=\"id-chip\">…d_parent</span></a></nav>"
    ));
}

#[test]
#[verifies("rule_ambiguous_links_disambiguated", examples)]
fn unique_lineage_and_breadcrumb_markup_remains_unchanged() {
    let html = render_requirement("default", &requirement_fixture());

    assert!(
        html.contains("<li><a href=\"/requirements/req_platform/\">ExampleOrg platform</a></li>")
    );
    assert!(html.contains(
        "<li class=\"current\">SaveInvoice shall split each claim item into portions</li>"
    ));
    assert!(html.contains(
        "<nav aria-label=\"Breadcrumb\"><a href=\"/requirements/req_platform/\">ExampleOrg platform</a> <span class=\"sep\">›</span> <a href=\"/requirements/req_sah/\">Support at Home (SAH)</a></nav>"
    ));
}

#[test]
fn requirement_page_attributes_borrowed_threads_to_their_parent() {
    let html = render_requirement("default", &requirement_fixture());
    assert!(html.contains("thr_resolution_res_split_0"));
    assert!(html.contains("on resolution res_split"));
    assert!(html.contains("1 message · active"));
    assert!(html.contains(">Assistant</span>"));
}

#[test]
fn requirement_page_renders_related_sibling_requirements_after_attribution() {
    let mut page = requirement_fixture();
    page.siblings = vec![
        link(
            PageKind::Requirement,
            "req_budget_split",
            "Budget portions shall reconcile",
        ),
        link(
            PageKind::Requirement,
            "req_zero_suppression",
            "Zero claim items shall be suppressed",
        ),
    ];

    let html = render_requirement("default", &page);
    let attribution_pos = html
        .find("<section class=\"attribution\" aria-label=\"Attribution\">")
        .unwrap();
    let related_pos = html
        .find("<h2 class=\"section-head sh-requirement\"><svg class=\"icon\"><use href=\"#i-git-branch\"/></svg>Related</h2>")
        .expect("sibling requirements should render in a Related section");
    assert!(related_pos > attribution_pos);
    assert!(html.contains(
        "<div class=\"card-head\"><svg class=\"icon\"><use href=\"#i-git-branch\"/></svg>Related Requirements — 2</div>"
    ));
    assert!(html.contains("<ul class=\"link-list\">"));
    assert!(html.contains(
        "<a href=\"/requirements/req_budget_split/\">Budget portions shall reconcile</a>"
    ));
    assert!(html.contains(
        "<a href=\"/requirements/req_zero_suppression/\">Zero claim items shall be suppressed</a>"
    ));
}

#[test]
fn requirement_page_omits_related_section_without_siblings() {
    let html = render_requirement("default", &gappy_requirement_fixture());
    assert!(!html.contains(">Related</h2>"));
    assert!(!html.contains("Related Requirements"));
}

#[test]
fn field_notes_who_shows_a_readable_role_not_the_raw_message_id() {
    let html = render_requirement("default", &requirement_fixture());
    assert!(
        !html.contains("msg_000001"),
        "the internal message id should never be shown as if it were an author name"
    );
    assert!(html.contains("<span class=\"who\">Assistant</span>"));
}

#[test]
fn field_notes_use_recorded_roles_without_guessing_actor_type() {
    let mut page = requirement_fixture();
    page.threads[0].messages[0].role = provenance_core::MessageRole::User;

    let html = render_requirement("default", &page);

    assert!(html.contains("<span class=\"who\">User</span>"));
    assert!(html.contains(">User</span>"));
    assert!(!html.contains(">Human</span>"));
}

#[test]
fn discussion_notes_render_numbered_findings_as_a_real_list() {
    let mut page = requirement_fixture();
    page.threads[0].messages[0].body = "Investigation complete.\n\nFindings:\n1. First finding\n2) Second finding\n\nConclusion: Keep exact words."
        .to_string();
    page.threads[0].messages[0].refs.clear();

    let html = render_requirement("default", &page);

    assert!(html.contains("<ol class=\"fn-list-block\">"), "{html}");
    assert!(html.contains("<li>First finding</li>"), "{html}");
    assert!(html.contains("<li>Second finding</li>"), "{html}");
    assert!(html.contains("<p class=\"fn-takeaway\">Investigation complete.</p>"));
}

#[test]
fn discussion_lists_keep_authored_numbers_when_the_sequence_skips() {
    let mut page = requirement_fixture();
    page.threads[0].messages[0].body =
        "Numbering:\n1. first\n3. third as written\n7) seventh as written\n8. eighth".to_string();
    page.threads[0].messages[0].refs.clear();

    let html = render_requirement("default", &page);

    assert!(html.contains("<li>first</li>"), "{html}");
    assert!(
        html.contains("<li value=\"3\">third as written</li>"),
        "{html}"
    );
    assert!(
        html.contains("<li value=\"7\">seventh as written</li>"),
        "{html}"
    );
    assert!(html.contains("<li>eighth</li>"), "{html}");
}

#[test]
fn discussion_notes_render_fenced_and_bare_json_as_code() {
    let mut page = requirement_fixture();
    page.threads[0].messages[0].body =
        "Payloads:\n\n```json\n{\"fenced\": true}\n```\n\n{\"bare\": [1, 2]}".to_string();
    page.threads[0].messages[0].refs.clear();

    let html = render_requirement("default", &page);

    assert!(html.contains(
        "<pre class=\"fn-code\"><code class=\"language-json\">{\"fenced\": true}</code></pre>"
    ));
    assert!(html.contains("<pre class=\"fn-code\"><code>{\"bare\": [1, 2]}</code></pre>"));
}

#[test]
fn discussion_takeaway_uses_an_explicit_final_conclusion_when_no_lead_is_derivable() {
    let mut page = requirement_fixture();
    page.threads[0].messages[0].body =
        "Findings:\n1. One observed fact\n2. Another observed fact\n\nConclusion: Preserve this line exactly."
            .to_string();
    page.threads[0].messages[0].refs.clear();

    let html = render_requirement("default", &page);

    assert!(html.contains("<p class=\"fn-takeaway\">Conclusion: Preserve this line exactly.</p>"));
}

#[test]
fn discussion_takeaway_uses_a_standalone_leading_line_without_inventing_punctuation() {
    let mut page = requirement_fixture();
    page.threads[0].messages[0].body =
        "Research complete: Exact title\n\nEvidence follows without a conclusion".to_string();
    page.threads[0].messages[0].refs.clear();

    let html = render_requirement("default", &page);

    assert!(html.contains("<p class=\"fn-takeaway\">Research complete: Exact title</p>"));
    assert!(!html.contains("Research complete: Exact title.</p>"));
}

#[test]
fn discussion_note_omits_takeaway_when_none_is_derivable() {
    let mut page = requirement_fixture();
    page.threads[0].messages[0].body =
        "Findings:\n1. One observed fact\n2. Another observed fact".to_string();
    page.threads[0].messages[0].refs.clear();

    let html = render_requirement("default", &page);

    assert!(!html.contains("class=\"fn-takeaway\""), "{html}");
}

#[test]
fn long_discussion_notes_collapse_behind_their_derived_first_line() {
    let mut page = requirement_fixture();
    page.threads[0].messages[0].body = format!(
        "Opening conclusion.\n\n{}",
        "Supporting detail remains verbatim. ".repeat(20)
    );
    page.threads[0].messages[0].refs.clear();

    let html = render_requirement("default", &page);

    assert!(
        html.contains("<details class=\"fn-collapsible\">"),
        "{html}"
    );
    assert!(html.contains(
        "<summary><span class=\"fn-takeaway\">Opening conclusion.</span><span class=\"fn-expand\">Expand note</span></summary>"
    ));
    assert!(html.contains("Supporting detail remains verbatim."));
}

#[test]
fn produced_rule_cards_show_display_names_and_confine_ids_to_chips() {
    let html = render_requirement("default", &requirement_fixture());

    assert!(html.contains(
        "<a href=\"/rules/rule_sah_inv_016/\">Suppress line emission for fully zero claim items</a>"
    ));
    assert!(html.contains("<span class=\"id-chip\">rule_sah_inv_016</span>"));
    assert!(!html.contains(">rule_sah_inv_016</a>"));
}

#[test]
fn gaps_render_as_dashed_citations_and_are_never_suppressed() {
    let html = render_requirement("default", &gappy_requirement_fixture());
    assert_eq!(html.matches("citation gap").count(), 3);
    assert!(html.contains("This requirement points to a source that is missing."));
    assert!(html.contains("This requirement has no source references."));
    assert!(html.contains("This requirement is marked resolved but has no resolving decision."));
}

#[test]
fn requirement_margin_puts_plain_gaps_after_source_citations() {
    let mut page = requirement_fixture();
    page.gaps = gappy_requirement_fixture().gaps;

    let html = render_requirement("default", &page);
    let source = html.find("SCHADS Award mapping").unwrap();
    let gaps = html.find(">Gaps</h3>").unwrap();
    let first_gap = html.find("citation gap").unwrap();

    assert!(source < gaps && gaps < first_gap, "{html}");
    assert!(!html.contains('`'), "{html}");
}

#[test]
fn gappy_page_keeps_the_fog_visible() {
    let html = render_requirement("default", &gappy_requirement_fixture());
    assert!(html.contains("Which award clauses apply is still unclear."));
}

#[test]
#[verifies("rule_requirement_badge", examples)]
fn resolved_requirement_without_decisions_or_rules_is_marked_unbacked() {
    let html = render_requirement("default", &gappy_requirement_fixture());

    assert!(html.contains("status-badge resolved-unbacked"));
    assert!(html.contains("Resolved (no decisions or rules)"));
    assert!(!html.contains("status-badge resolved\""));
}
