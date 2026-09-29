//! Property verification for `rule_ambiguous_links_disambiguated`.
//!
//! A deterministic generator builds pages of links: titles carried by two to
//! five different records, ids that share long endings so the suffix search
//! has to walk most of the id before it finds a difference, ids repeated
//! across record kinds, ids shorter than the eight-character floor, ids with
//! combining marks, and singleton controls that must stay unmarked.
//!
//! Every page is split into sections, the way a rendered page splits into
//! Refined Into, Related, Lineage and the rest, and a colliding group is as
//! likely to be dealt across sections as to sit inside one. The renderer under
//! test is built once per page, so the properties below are page-wide.
//!
//! Each property is restated here from the decision. Nothing below calls the
//! renderer's own suffix search or label table; the tests read only what a
//! reader would see.

use crate::wiki::model::{PageId, PageLink, RecordKind};
use provenance_macros::verifies;

use super::PageLinksRenderer;

const PAGES: usize = 512;
const FLOOR: usize = 8;

const KINDS: [RecordKind; 4] = [
    RecordKind::Requirement,
    RecordKind::Resolution,
    RecordKind::Rule,
    RecordKind::Source,
];

/// The words a chip may put in front of the id, restated so the tests do not
/// lean on the renderer's own label table.
const KIND_WORDS: [(RecordKind, &str); 4] = [
    (RecordKind::Requirement, "Requirement"),
    (RecordKind::Resolution, "Resolution"),
    (RecordKind::Rule, "Rule"),
    (RecordKind::Source, "Source"),
];

/// Ids are a head followed by a tail. Tails are shared, so two ids drawn from
/// one tail agree on their last dozen characters or more.
const TAILS: &[&str] = &[
    "_participant_budget_summary_shall_pro_rate",
    "_shall_reconcile_against_the_claim",
    "_budget_split",
    "_x",
    "_e\u{301}te\u{301}",
];

/// Heads differ early, or not at all. The empty head makes the whole id the
/// shared tail, so that id is an ending of every sibling drawn from it.
const HEADS: &[&str] = &[
    "req_sah",
    "req_sah_revised",
    "req",
    "req_sah_2024",
    "",
    "re\u{301}q",
];

const TITLES: &[&str] = &[
    "Participant budget summary shall pro-rate services",
    "Zero claim items shall be suppressed",
    "Shared title",
];

struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut word = self.0;
        word = (word ^ (word >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        word = (word ^ (word >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        word ^ (word >> 31)
    }

    fn below(&mut self, bound: usize) -> usize {
        usize::try_from(self.next_u64() % bound as u64).unwrap()
    }

    fn word(&mut self, choices: &[&'static str]) -> &'static str {
        choices[self.below(choices.len())]
    }

    fn kind(&mut self) -> RecordKind {
        KINDS[self.below(KINDS.len())]
    }
}

fn page_link(kind: RecordKind, record_id: &str, title: &str) -> PageLink {
    PageLink {
        target: PageId::new(kind, record_id),
        title: title.to_string(),
    }
}

/// Two to five records under one title, their ids sharing a long ending.
fn shared_ending_group(rng: &mut Rng, title: &str, links: &mut Vec<PageLink>) {
    let tail = rng.word(TAILS);
    let records = 2 + rng.below(4);
    for _ in 0..records {
        let record_id = format!("{}{tail}", rng.word(HEADS));
        links.push(page_link(rng.kind(), &record_id, title));
    }
}

/// One id, several kinds: nothing in the id can tell these apart.
fn repeated_id_group(rng: &mut Rng, title: &str, links: &mut Vec<PageLink>) {
    let record_id = format!("{}{}", rng.word(HEADS), rng.word(TAILS));
    let kinds = 2 + rng.below(3);
    for kind in KINDS.iter().take(kinds) {
        links.push(page_link(*kind, &record_id, title));
    }
}

/// Ids that differ only in their last character, as generated ids do.
fn near_twin_group(rng: &mut Rng, title: &str, links: &mut Vec<PageLink>) {
    let stem = format!("{}{}", rng.word(HEADS), rng.word(TAILS));
    let records = 2 + rng.below(4);
    for index in 0..records {
        let record_id = format!("{stem}_{index}");
        links.push(page_link(rng.kind(), &record_id, title));
    }
}

/// One page as the renderer sees it: several link lists, rendered together.
struct Page {
    sections: Vec<Vec<PageLink>>,
}

impl Page {
    /// Every link the page renders, in reading order.
    fn links(&self) -> Vec<&PageLink> {
        self.sections.iter().flatten().collect()
    }

    /// The renderer the page is rendered with: one for the whole page.
    fn renderer(&self) -> PageLinksRenderer {
        PageLinksRenderer::new(self.links())
    }
}

/// Deals one group of links into the page's sections: whole, spread one per
/// section, or cut in two.
fn deal(rng: &mut Rng, group: Vec<PageLink>, sections: &mut [Vec<PageLink>]) {
    match rng.below(3) {
        0 => {
            let section = rng.below(sections.len());
            sections[section].extend(group);
        }
        1 => {
            let first = rng.below(sections.len());
            for (offset, link) in group.into_iter().enumerate() {
                sections[(first + offset) % sections.len()].push(link);
            }
        }
        _ => {
            let cut = 1 + rng.below(group.len().max(2) - 1);
            let (left, right) = (rng.below(sections.len()), rng.below(sections.len()));
            for (index, link) in group.into_iter().enumerate() {
                sections[if index < cut { left } else { right }].push(link);
            }
        }
    }
}

fn generated_page(rng: &mut Rng, page: usize) -> Page {
    let mut sections = vec![Vec::new(); 2 + rng.below(3)];
    sections[0].push(page_link(
        RecordKind::Requirement,
        &format!("req_control_{page}"),
        &format!("Control title {page}"),
    ));
    let groups = 1 + rng.below(3);
    for _ in 0..groups {
        let title = rng.word(TITLES);
        let mut group = Vec::new();
        match rng.below(4) {
            0 => repeated_id_group(rng, title, &mut group),
            1 => near_twin_group(rng, title, &mut group),
            _ => shared_ending_group(rng, title, &mut group),
        }
        deal(rng, group, &mut sections);
    }
    Page { sections }
}

fn generated_pages() -> Vec<Page> {
    let mut rng = Rng(0x5eed_1d0c);
    (0..PAGES)
        .map(|page| generated_page(&mut rng, page))
        .collect()
}

/// The distinct records, other than this link's own, that carry its title.
fn rivals<'a>(links: &[&'a PageLink], link: &PageLink) -> Vec<&'a PageId> {
    let mut targets: Vec<&PageId> = Vec::new();
    for other in links {
        if other.title == link.title
            && other.target != link.target
            && !targets.contains(&&other.target)
        {
            targets.push(&other.target);
        }
    }
    targets
}

fn unescape(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

fn strip_markup(html: &str) -> String {
    let mut text = String::new();
    let mut inside_tag = false;
    for character in html.chars() {
        match character {
            '<' => inside_tag = true,
            '>' => inside_tag = false,
            _ if !inside_tag => text.push(character),
            _ => {}
        }
    }
    unescape(&text)
}

/// What a reader sees for one link. Both surfaces the renderer offers - the
/// anchor and the plain text used for the current lineage entry - must read
/// the same, or the two would disagree about which record is which.
fn shown(renderer: &PageLinksRenderer, link: &PageLink) -> String {
    let anchored = strip_markup(&renderer.link(link, None));
    let plain = strip_markup(&renderer.text(link));
    assert_eq!(
        anchored, plain,
        "the linked and unlinked surfaces read differently for {link:?}"
    );
    anchored
}

fn chip(renderer: &PageLinksRenderer, link: &PageLink) -> Option<String> {
    const OPEN: &str = "<span class=\"id-chip\">";
    let html = renderer.link(link, None);
    let start = html.find(OPEN)? + OPEN.len();
    let end = start + html[start..].find("</span>").expect("a chip must close");
    Some(unescape(&html[start..end]))
}

/// Splits a chip into the kind word it may lead with and the id ending.
fn split_chip(chip: &str) -> (Option<&'static str>, &str) {
    for (_, word) in KIND_WORDS {
        if let Some(rest) = chip
            .strip_prefix(word)
            .and_then(|rest| rest.strip_prefix(" · "))
        {
            return (Some(word), rest);
        }
    }
    (None, chip)
}

fn tail_of(text: &str, count: usize) -> String {
    let total = text.chars().count();
    text.chars().skip(total - count).collect()
}

/// (a) Two links to different records that share a title never read the same.
#[test]
#[verifies("rule_ambiguous_links_disambiguated", property)]
fn links_to_different_records_under_one_title_never_read_alike() {
    let mut compared = 0_usize;
    for page in generated_pages() {
        let renderer = page.renderer();
        let links = page.links();
        for (index, left) in links.iter().enumerate() {
            for right in &links[index + 1..] {
                if left.title != right.title || left.target == right.target {
                    continue;
                }
                compared += 1;
                let (left_text, right_text) = (shown(&renderer, left), shown(&renderer, right));
                assert_ne!(
                    left_text, right_text,
                    "{:?} and {:?} both read as {left_text:?}",
                    left.target, right.target
                );
            }
        }
    }
    assert!(compared > 0, "no page ever put two records under one title");
}

/// (b) Every chip shown is a genuine ending of its own record's id, at least
/// eight characters long - or the whole id, when the id is shorter than that.
#[test]
#[verifies("rule_ambiguous_links_disambiguated", property)]
fn every_chip_is_a_genuine_ending_of_its_own_id() {
    let (mut with_kind, mut cut, mut whole, mut short) = (0_usize, 0_usize, 0_usize, 0_usize);
    for page in generated_pages() {
        let renderer = page.renderer();
        for link in page.links() {
            let Some(chip) = chip(&renderer, link) else {
                continue;
            };
            let record_id = &link.target.record_id;
            let (kind_word, rest) = split_chip(&chip);
            if let Some(word) = kind_word {
                with_kind += 1;
                assert_eq!(
                    Some(word),
                    KIND_WORDS
                        .iter()
                        .find(|(kind, _)| *kind == link.target.kind)
                        .map(|(_, word)| *word),
                    "chip {chip:?} names a kind {:?} does not have",
                    link.target
                );
            }
            let ending = rest.strip_prefix('…').map_or(rest, |ending| {
                cut += 1;
                ending
            });
            assert!(
                record_id.ends_with(ending),
                "chip {chip:?} is not an ending of {record_id:?}"
            );
            assert_eq!(
                ending == record_id.as_str(),
                rest.strip_prefix('…').is_none(),
                "chip {chip:?} marks a cut it did not make, or hides one it did"
            );
            let length = ending.chars().count();
            let id_length = record_id.chars().count();
            if length == id_length {
                whole += 1;
            }
            if id_length < FLOOR {
                short += 1;
            }
            assert!(
                length >= FLOOR.min(id_length),
                "chip {chip:?} shows {length} characters of {record_id:?}"
            );
        }
    }
    assert!(with_kind > 0, "no chip ever had to name a record kind");
    assert!(cut > 0, "no chip was ever shorter than the whole id");
    assert!(whole > 0, "no chip ever fell back to the whole id");
    assert!(
        short > 0,
        "no id was ever shorter than the eight-character floor"
    );
}

/// (c) A title no other record on the page carries gets no chip.
#[test]
#[verifies("rule_ambiguous_links_disambiguated", property)]
fn a_title_no_other_record_carries_gets_no_chip() {
    let mut alone = 0_usize;
    for page in generated_pages() {
        let renderer = page.renderer();
        let links = page.links();
        for link in &links {
            if !rivals(&links, link).is_empty() {
                continue;
            }
            alone += 1;
            assert_eq!(
                chip(&renderer, link),
                None,
                "{:?} is the only record titled {:?}, yet it was marked",
                link.target,
                link.title
            );
        }
    }
    assert!(alone > 0, "no page ever held an uncontested title");
}

/// (d) Chips are as short as they can be: drop one character from a chip that
/// still has room above the floor, and some rival's id ends the same way.
#[test]
#[verifies("rule_ambiguous_links_disambiguated", property)]
fn no_chip_could_be_one_character_shorter() {
    let mut checked = 0_usize;
    for page in generated_pages() {
        let renderer = page.renderer();
        let links = page.links();
        for link in &links {
            let Some(chip) = chip(&renderer, link) else {
                continue;
            };
            let record_id = &link.target.record_id;
            let (_, rest) = split_chip(&chip);
            let ending = rest.strip_prefix('…').unwrap_or(rest);
            let length = ending.chars().count();
            if length <= FLOOR.min(record_id.chars().count()) {
                continue;
            }
            checked += 1;
            let shorter = tail_of(record_id, length - 1);
            assert!(
                rivals(&links, link)
                    .iter()
                    .any(|rival| rival.record_id.ends_with(&shorter)),
                "chip {chip:?} could have stopped at {shorter:?} without meeting a rival"
            );
        }
    }
    assert!(
        checked > 0,
        "no chip ever grew past the eight-character floor"
    );
}

/// (e) A title shared across two sections of one page is disambiguated too.
/// Each section is its own list; a renderer built per list would see the
/// record alone under its title and leave both links bare.
#[test]
#[verifies("rule_ambiguous_links_disambiguated", property)]
fn a_title_shared_only_across_sections_is_still_disambiguated() {
    let mut crossings = 0_usize;
    for page in generated_pages() {
        let renderer = page.renderer();
        let whole_page = page.links();
        for section in &page.sections {
            let alongside: Vec<&PageLink> = section.iter().collect();
            for link in section {
                if !rivals(&alongside, link).is_empty() || rivals(&whole_page, link).is_empty() {
                    continue;
                }
                crossings += 1;
                assert!(
                    chip(&renderer, link).is_some(),
                    "{:?} shares the title {:?} with a record in another section, yet reads bare",
                    link.target,
                    link.title
                );
            }
        }
    }
    assert!(
        crossings > 0,
        "no page ever put two records under one title in different sections"
    );
}
