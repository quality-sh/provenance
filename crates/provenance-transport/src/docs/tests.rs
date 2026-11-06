//! Verification of `rule_docs_links_resolve`.
//!
//! A docs tree is an open set of documents carrying an open set of link
//! destinations, so the properties below are checked against generated
//! sites rather than a fixed table. Each site is a set of published pages
//! drawn from a pool of candidate files, and each page is assembled from
//! link forms - relative, `./`-prefixed, climbing, fragment-suffixed,
//! absolute, web, `mailto:`, bare fragment, image, and every reference
//! style - aimed at targets the site publishes and targets it does not.
//!
//! The three properties are stated separately and each generates its own
//! sites: one reads every report back and checks it names a local document
//! the site does not publish, one checks that nothing resolvable and
//! nothing out of scope is ever reported, and one counts. None of them
//! calls the functions the decision is built from: scope is restated by
//! reading the destination text, and resolution by walking the destination
//! from the page's own directory the way a reader would.
//!
//! The sites are written in the markdown a `MARKDOWN_OPTIONS` parser and a
//! bare one read alike, so the generator says nothing about the extensions.
//! The one form that turns on them, a footnote, is an example instead.

use super::{route_depth, DocPage, DocsSite};
use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::PathBuf;

/// `SplitMix64`. The seeds are fixed, so every run walks the same sites and
/// a failure is reproducible from the test name.
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

    fn pick<'a, T>(&mut self, choices: &'a [T]) -> &'a T {
        &choices[self.below(choices.len())]
    }
}

/// Where the generated repo sits. Absolute, because a page's file path is.
const SITE_ROOT: &str = "/site";

/// Files a docs tree might hold. Which of them a site publishes changes
/// from site to site: a markdown file existing is not the same as the site
/// publishing a page for it.
const CANDIDATE_FILES: &[&str] = &[
    "README.md",
    "docs/index.md",
    "docs/reference.md",
    "docs/guide/index.md",
    "docs/guide/install.md",
    "docs/guide/deep/nested.md",
    "docs/notes/faq.md",
    // A colon that is not a URL scheme, because it follows a slash.
    "docs/notes/v1:draft.md",
];

/// Names no generated site publishes, so a link to one is broken wherever
/// it is written.
const ABSENT_FILES: &[&str] = &[
    "docs/missing.md",
    "docs/guide/gone.md",
    "docs/notes/typo.md",
    "ghost.md",
];

/// Destinations that name something other than a document.
const NON_DOCUMENT_DESTINATIONS: &[&str] = &[
    "assets/diagram.png",
    "../scripts/build.sh",
    "install.md.txt",
    "../CHANGELOG",
    "guide/",
    "",
];

const ANCHORS: &[&str] = &["usage", "install", "top", "why-this"];

/// How a destination is written into the document. Everything but `Image`
/// reaches the reader as a link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Writing {
    Inline,
    Image,
    Reference,
    Collapsed,
    Shortcut,
    Autolink,
}

struct Plant {
    destination: String,
    writing: Writing,
}

/// What should become of a planted destination, worked out without asking
/// the decision under test.
#[derive(Debug, PartialEq, Eq)]
enum Verdict {
    /// A local document the site does not publish, resolving to this path.
    Broken(String),
    /// A local document the site publishes.
    PublishedTarget,
    /// Not this decision's business, for the reason named.
    OutOfScope(&'static str),
}

struct Page {
    file: &'static str,
    plants: Vec<Plant>,
}

struct Corpus {
    site: DocsSite,
    published: BTreeSet<String>,
    pages: Vec<Page>,
}

impl Corpus {
    fn page(&self, source_path: &str) -> Option<&Page> {
        self.pages.iter().find(|page| page.file == source_path)
    }

    fn verdicts(&self) -> impl Iterator<Item = (&Page, &Plant, Verdict)> {
        self.pages.iter().flat_map(move |page| {
            page.plants
                .iter()
                .map(move |plant| (page, plant, verdict(plant, page.file, &self.published)))
        })
    }
}

/// Reads a destination the way a reader does, to say whether it names a
/// document in this docs tree: nothing beginning a fragment or an absolute
/// path, nothing carrying a scheme ahead of the first slash, and a name
/// ending in `.md` once any trailing fragment is cut off.
fn is_local_markdown_destination(destination: &str) -> bool {
    if destination.starts_with('#') || destination.starts_with('/') {
        return false;
    }
    let ahead_of_first_slash = destination.split('/').next().unwrap_or_default();
    if ahead_of_first_slash.contains(':') && !ahead_of_first_slash.starts_with(':') {
        return false;
    }
    let named = destination.split('#').next().unwrap_or_default();
    !named.is_empty() && named.to_ascii_lowercase().ends_with(".md")
}

/// Walks a destination from the directory of the page it was written on,
/// the way a reader following it on disk would.
fn resolve_beside(source_file: &str, destination: &str) -> String {
    let mut walked: Vec<&str> = source_file.split('/').collect();
    walked.pop();
    for part in destination.split('#').next().unwrap_or_default().split('/') {
        match part {
            "" | "." => {}
            ".." => {
                walked.pop();
            }
            name => walked.push(name),
        }
    }
    format!("{SITE_ROOT}/{}", walked.join("/"))
}

fn verdict(plant: &Plant, source_file: &str, published: &BTreeSet<String>) -> Verdict {
    if plant.writing == Writing::Image {
        return Verdict::OutOfScope("an image, not a link");
    }
    if plant.destination.starts_with('#') {
        return Verdict::OutOfScope("a fragment on the page itself");
    }
    if plant.destination.starts_with('/') {
        return Verdict::OutOfScope("an absolute path");
    }
    if !is_local_markdown_destination(&plant.destination) {
        return Verdict::OutOfScope("not a local markdown document");
    }
    let resolved = resolve_beside(source_file, &plant.destination);
    if published.contains(&resolved) {
        Verdict::PublishedTarget
    } else {
        Verdict::Broken(resolved)
    }
}

/// The destination text a link on `source_file` would carry to reach
/// `target_file`, as a path relative to the page's own directory.
fn relative_destination(source_file: &str, target_file: &str) -> String {
    let source: Vec<&str> = source_file.split('/').collect();
    let source_directories = &source[..source.len() - 1];
    let target: Vec<&str> = target_file.split('/').collect();
    let target_directories = &target[..target.len() - 1];
    let shared = source_directories
        .iter()
        .zip(target_directories)
        .take_while(|(here, there)| here == there)
        .count();
    let mut parts = vec![".."; source_directories.len() - shared];
    parts.extend(&target_directories[shared..]);
    parts.push(target[target.len() - 1]);
    parts.join("/")
}

/// A destination naming a local document, written in one of the shapes an
/// author reaches for.
fn gen_local_destination(rng: &mut Rng, source_file: &str) -> String {
    let target = if rng.below(3) == 0 {
        *rng.pick(ABSENT_FILES)
    } else {
        *rng.pick(CANDIDATE_FILES)
    };
    let relative = relative_destination(source_file, target);
    let anchor = rng.pick(ANCHORS);
    match rng.below(5) {
        1 => format!("./{relative}"),
        2 => format!("{relative}#{anchor}"),
        3 => format!("./{relative}#{anchor}"),
        // Same document, spelled with the extension an author shouted.
        4 => relative.replace(".md", ".MD"),
        _ => relative,
    }
}

fn gen_plant(rng: &mut Rng, source_file: &str) -> Plant {
    match rng.below(12) {
        0..=5 => Plant {
            destination: gen_local_destination(rng, source_file),
            writing: *rng.pick(&[
                Writing::Inline,
                Writing::Reference,
                Writing::Collapsed,
                Writing::Shortcut,
            ]),
        },
        6 => Plant {
            destination: gen_local_destination(rng, source_file),
            writing: Writing::Image,
        },
        7 => Plant {
            destination: format!("#{}", rng.pick(ANCHORS)),
            writing: Writing::Inline,
        },
        8 => Plant {
            destination: format!("/{}", rng.pick(CANDIDATE_FILES)),
            writing: Writing::Inline,
        },
        9 => Plant {
            destination: format!("https://example.com/{}", rng.pick(CANDIDATE_FILES)),
            writing: if rng.below(2) == 0 {
                Writing::Inline
            } else {
                Writing::Autolink
            },
        },
        10 => Plant {
            destination: (*rng.pick(&["mailto:docs@example.com", "http://example.org/a.md"]))
                .to_string(),
            writing: Writing::Inline,
        },
        _ => Plant {
            destination: (*rng.pick(NON_DOCUMENT_DESTINATIONS)).to_string(),
            writing: Writing::Inline,
        },
    }
}

/// Plants for one page. Destinations are kept distinct so that a report can
/// be matched back to the plant that asked for it.
fn gen_plants(rng: &mut Rng, source_file: &str) -> Vec<Plant> {
    let mut seen = BTreeSet::new();
    let mut plants = Vec::new();
    for _ in 0..rng.below(9) {
        let plant = gen_plant(rng, source_file);
        if seen.insert(plant.destination.clone()) {
            plants.push(plant);
        }
    }
    plants
}

fn write_document(title: &str, plants: &[Plant]) -> String {
    let mut body = format!("# {title}\n\nA page with links on it.\n\n");
    let mut definitions = String::new();
    for (index, plant) in plants.iter().enumerate() {
        let destination = &plant.destination;
        let label = format!("ref{index}");
        let line = match plant.writing {
            Writing::Inline => format!("See [link {index}]({destination})."),
            Writing::Image => format!("![diagram {index}]({destination})"),
            Writing::Autolink => format!("See <{destination}>."),
            Writing::Reference => format!("See [link {index}][{label}]."),
            Writing::Collapsed => format!("See [{label}][]."),
            Writing::Shortcut => format!("See [{label}]."),
        };
        writeln!(body, "{line}\n").expect("writing to a String cannot fail");
        if matches!(
            plant.writing,
            Writing::Reference | Writing::Collapsed | Writing::Shortcut
        ) {
            writeln!(definitions, "[{label}]: {destination}")
                .expect("writing to a String cannot fail");
        }
    }
    body.push_str(&definitions);
    body
}

fn route_for(file: &str) -> String {
    if file == "README.md" {
        return "/".to_string();
    }
    let trimmed = file.strip_prefix("docs/").unwrap_or(file);
    let stem = trimmed.strip_suffix(".md").unwrap_or(trimmed);
    let mut parts: Vec<&str> = stem.split('/').collect();
    if parts.last() == Some(&"index") {
        parts.pop();
    }
    if parts.is_empty() {
        "/".to_string()
    } else {
        format!("/{}/", parts.join("/"))
    }
}

/// The files one site publishes. Exactly one homepage, as the loader
/// allows: a `README.md` shadowed by `docs/index.md` stays a file on disk
/// that the site does not publish.
fn gen_published(rng: &mut Rng) -> Vec<&'static str> {
    let mut published = vec![if rng.below(2) == 0 {
        "README.md"
    } else {
        "docs/index.md"
    }];
    for file in CANDIDATE_FILES {
        if *file != "README.md" && *file != "docs/index.md" && rng.below(3) > 0 {
            published.push(file);
        }
    }
    published
}

fn gen_corpus(rng: &mut Rng) -> Corpus {
    let published_files = gen_published(rng);
    let published: BTreeSet<String> = published_files
        .iter()
        .map(|file| format!("{SITE_ROOT}/{file}"))
        .collect();

    let mut pages = Vec::new();
    let mut doc_pages = Vec::new();
    for file in published_files {
        let plants = gen_plants(rng, file);
        let route = route_for(file);
        doc_pages.push(DocPage {
            depth: route_depth(&route),
            route,
            title: file.to_string(),
            source_path: file.to_string(),
            file_path: PathBuf::from(format!("{SITE_ROOT}/{file}")),
            markdown: write_document(file, &plants),
        });
        pages.push(Page { file, plants });
    }

    let site = DocsSite {
        route_by_path: doc_pages
            .iter()
            .map(|page| (page.file_path.clone(), page.route.clone()))
            .collect(),
        page_by_route: doc_pages
            .iter()
            .enumerate()
            .map(|(index, page)| (page.route.clone(), index))
            .collect(),
        pages: doc_pages,
    };
    Corpus {
        site,
        published,
        pages,
    }
}

/// The site infers each page's route and title from its file, and uses README.md as
/// the homepage when docs/index.md is absent.
#[test]
fn the_site_infers_routes_and_titles() {
    let pages = |files: &[(&str, &str)]| {
        let dir = tempfile::tempdir().unwrap();
        for (file, markdown) in files {
            let path = dir.path().join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, markdown).unwrap();
        }
        DocsSite::load(dir.path())
            .unwrap()
            .pages
            .into_iter()
            .map(|page| (page.route, page.title, page.source_path))
            .collect::<Vec<_>>()
    };
    let owned = |route: &str, title: &str, source: &str| {
        (route.to_owned(), title.to_owned(), source.to_owned())
    };
    assert_eq!(
        pages(&[
            ("docs/index.md", "# Agent Handbook\n"),
            ("docs/guide/install.md", "# Installation\n"),
        ]),
        [
            owned("/", "Agent Handbook", "docs/index.md"),
            owned("/guide/install/", "Installation", "docs/guide/install.md"),
        ]
    );
    assert_eq!(
        pages(&[
            ("README.md", "# Project Handbook\n"),
            ("docs/guide/install.md", "# Installation\n"),
        ]),
        [
            owned("/", "Project Handbook", "README.md"),
            owned("/guide/install/", "Installation", "docs/guide/install.md"),
        ]
    );
}

mod link_resolution;
