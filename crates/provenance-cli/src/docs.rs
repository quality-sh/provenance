use crate::output;
use anyhow::{bail, Context};
use camino::Utf8PathBuf;
use provenance_macros::rule;
use pulldown_cmark::{Event, Options, Parser, Tag};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    fmt::Write as _,
    path::{Component, Path, PathBuf},
};
use walkdir::WalkDir;

mod web;

/// How this site reads markdown, written down once.
///
/// `docs check` and `docs serve` are two readings of one document, and a
/// document read two ways can be checked as one thing and served as
/// another: a footnote definition is a link definition to a parser without
/// `ENABLE_FOOTNOTES` and a footnote to a parser with it. Both sides parse
/// with these options, so a destination the check resolves is a
/// destination the server rewrites, and a destination the check never sees
/// is one the server never renders as a link.
const MARKDOWN_OPTIONS: Options = Options::ENABLE_TABLES
    .union(Options::ENABLE_STRIKETHROUGH)
    .union(Options::ENABLE_TASKLISTS)
    .union(Options::ENABLE_FOOTNOTES);

#[derive(Debug, Clone)]
struct DocsSite {
    source_root: String,
    pages: Vec<DocPage>,
    route_by_path: BTreeMap<PathBuf, String>,
    page_by_route: BTreeMap<String, usize>,
}

#[derive(Debug, Clone)]
struct DocPage {
    route: String,
    title: String,
    source_path: String,
    file_path: PathBuf,
    markdown: String,
    depth: usize,
}

#[derive(Serialize)]
struct DocsCheckReport {
    status: &'static str,
    source_root: String,
    homepage_route: &'static str,
    page_count: usize,
    pages: Vec<DocPageSummary>,
    broken_links: Vec<BrokenLink>,
}

#[derive(Serialize)]
struct DocPageSummary {
    route: String,
    title: String,
    source_path: String,
}

#[derive(Debug, Clone, Serialize)]
struct BrokenLink {
    source_path: String,
    link: String,
    resolved_path: String,
}

pub fn check(repo: &Utf8PathBuf) -> anyhow::Result<()> {
    let site = DocsSite::load(repo.as_std_path())?;
    site.ensure_links_valid()?;
    output::print_json(&site.check_report())?;
    Ok(())
}

pub async fn serve(repo: Utf8PathBuf, host: String, port: u16) -> anyhow::Result<()> {
    let site = DocsSite::load(repo.as_std_path())?;
    site.ensure_links_valid()?;
    let app = web::router(site);
    let listener = tokio::net::TcpListener::bind((host.as_str(), port))
        .await
        .with_context(|| format!("failed to bind docs server to {host}:{port}"))?;
    eprintln!(
        "serving Provenance docs at http://{}",
        listener.local_addr()?
    );
    axum::serve(listener, app).await?;
    Ok(())
}

impl DocsSite {
    fn load(repo: &Path) -> anyhow::Result<Self> {
        let repo = repo
            .canonicalize()
            .with_context(|| format!("repo path does not exist: {}", repo.display()))?;
        let docs_dir = repo.join("docs");
        let mut source_files = Vec::new();

        if docs_dir.is_dir() {
            for entry in WalkDir::new(&docs_dir).sort_by_file_name() {
                let entry = entry?;
                if entry.file_type().is_file() && is_markdown_file(entry.path()) {
                    source_files.push(entry.path().to_path_buf());
                }
            }
        }

        let has_docs_index = source_files
            .iter()
            .any(|path| path == &docs_dir.join("index.md"));
        let readme_path = repo.join("README.md");
        if !has_docs_index && readme_path.is_file() {
            source_files.push(readme_path);
        }

        if source_files.is_empty() {
            bail!("no docs found; create docs/index.md or README.md");
        }

        let source_root =
            if docs_dir.is_dir() && source_files.iter().any(|path| path.starts_with(&docs_dir)) {
                "docs"
            } else {
                "README.md"
            }
            .to_string();

        let mut pages = Vec::new();
        for file_path in source_files {
            let markdown = std::fs::read_to_string(&file_path)
                .with_context(|| format!("failed to read {}", file_path.display()))?;
            let source_path = slash_path(
                file_path
                    .strip_prefix(&repo)
                    .context("docs source was outside the repo")?,
            );
            let route = route_for_source(&repo, &docs_dir, &file_path)?;
            let title = infer_title(&markdown, &file_path);
            let depth = route_depth(&route);
            pages.push(DocPage {
                route,
                title,
                source_path,
                file_path: normalize_path(&file_path),
                markdown,
                depth,
            });
        }

        if pages.iter().all(|page| page.route != "/") {
            bail!("docs homepage not found; create docs/index.md or README.md");
        }

        pages.sort_by_key(page_sort_key);

        let mut route_to_source = BTreeMap::new();
        for page in &pages {
            if let Some(existing) =
                route_to_source.insert(page.route.clone(), page.source_path.clone())
            {
                bail!(
                    "docs route conflict for {} between {} and {}",
                    page.route,
                    existing,
                    page.source_path
                );
            }
        }

        let route_by_path = pages
            .iter()
            .map(|page| (page.file_path.clone(), page.route.clone()))
            .collect();
        let page_by_route = pages
            .iter()
            .enumerate()
            .map(|(index, page)| (page.route.clone(), index))
            .collect();

        Ok(Self {
            source_root,
            pages,
            route_by_path,
            page_by_route,
        })
    }

    fn check_report(&self) -> DocsCheckReport {
        DocsCheckReport {
            status: "ok",
            source_root: self.source_root.clone(),
            homepage_route: "/",
            page_count: self.pages.len(),
            pages: self
                .pages
                .iter()
                .map(|page| DocPageSummary {
                    route: page.route.clone(),
                    title: page.title.clone(),
                    source_path: page.source_path.clone(),
                })
                .collect(),
            broken_links: Vec::new(),
        }
    }

    fn ensure_links_valid(&self) -> anyhow::Result<()> {
        let broken_links = self.broken_links();
        if broken_links.is_empty() {
            return Ok(());
        }

        let mut message = format!("{} broken docs link(s)", broken_links.len());
        for link in broken_links.iter().take(10) {
            write!(
                message,
                "\n{} -> {} (resolved to {})",
                link.source_path, link.link, link.resolved_path
            )?;
        }
        bail!(message);
    }

    /// Decides which of the docs tree's markdown links do not resolve.
    ///
    /// A link that names a local document is a promise about this site: the
    /// document it names has to be a page the site actually publishes.
    /// Every destination that names a `.md` file relative to the page it is
    /// written on is joined onto that page's directory, normalized, and
    /// looked up among the pages loaded from the repo. The site's own page
    /// list is the authority: a markdown file that exists on disk but that
    /// the site does not publish (a `README.md` shadowed by `docs/index.md`,
    /// say) is not a target.
    ///
    /// Deliberately out of scope, and never reported: a bare `#anchor`, an
    /// absolute path, any destination carrying a URL scheme, a destination
    /// that does not end in `.md`, the fragment hanging off a local
    /// destination (the page has to exist, the heading inside it need not),
    /// and images.
    ///
    /// The server resolves a link with the same two functions
    /// (`local_markdown_path` and `resolve_markdown_link`) before rewriting
    /// it to a route, so what this decision accepts is exactly what `docs
    /// serve` can turn into a link a reader can follow.
    #[rule("rule_docs_links_resolve")]
    fn broken_links(&self) -> Vec<BrokenLink> {
        let mut broken = Vec::new();
        for page in &self.pages {
            for link in markdown_links(&page.markdown) {
                let Some(target_path) = resolve_markdown_link(page, &link) else {
                    continue;
                };
                if !self.route_by_path.contains_key(&target_path) {
                    broken.push(BrokenLink {
                        source_path: page.source_path.clone(),
                        link,
                        resolved_path: slash_path(&target_path),
                    });
                }
            }
        }
        broken
    }
}

/// Collects the destinations of every link on a page that names a local
/// document.
///
/// Reach: inline links (`[text](x.md)`) and every reference form
/// (`[text][ref]`, `[ref][]`, `[ref]`), which the parser hands back already
/// resolved to the destination in the definition. Images are a different
/// tag and are not collected, so a picture is never a broken docs link.
///
/// The page is parsed with `MARKDOWN_OPTIONS`, the options the server
/// renders with, so what counts as a link here is what the reader will be
/// offered as one. A footnote is the case where that matters: `[^1]` with a
/// `[^1]: guide/gone.md` block below it is a footnote to both sides, and
/// neither reports nor renders a link.
fn markdown_links(markdown: &str) -> Vec<String> {
    let mut links = Vec::new();
    for event in Parser::new_ext(markdown, MARKDOWN_OPTIONS) {
        if let Event::Start(Tag::Link { dest_url, .. }) = event {
            if local_markdown_path(&dest_url).is_some() {
                links.push(dest_url.to_string());
            }
        }
    }
    links
}

fn resolve_markdown_link(page: &DocPage, destination: &str) -> Option<PathBuf> {
    let (path_part, _suffix) = local_markdown_path(destination)?;
    let target_path = page.file_path.parent()?.join(path_part);
    Some(normalize_path(&target_path))
}

/// Splits a destination that names a local document into the path it names
/// and the fragment trailing it, or `None` when the destination is out of
/// the docs link decision's scope.
fn local_markdown_path(destination: &str) -> Option<(&str, &str)> {
    if destination.starts_with('#') || destination.starts_with('/') || has_url_scheme(destination) {
        return None;
    }

    let (path_part, suffix) = destination.find('#').map_or((destination, ""), |index| {
        (&destination[..index], &destination[index..])
    });
    if path_part.is_empty() || !path_part.to_ascii_lowercase().ends_with(".md") {
        return None;
    }
    Some((path_part, suffix))
}

fn has_url_scheme(destination: &str) -> bool {
    let scheme_end = destination.find(':');
    let slash = destination.find('/');
    matches!((scheme_end, slash), (Some(colon), None) if colon > 0)
        || matches!((scheme_end, slash), (Some(colon), Some(slash)) if colon > 0 && colon < slash)
}

fn route_for_source(repo: &Path, docs_dir: &Path, file_path: &Path) -> anyhow::Result<String> {
    if file_path == repo.join("README.md") {
        return Ok("/".to_string());
    }

    let relative = file_path
        .strip_prefix(docs_dir)
        .with_context(|| format!("docs source is outside docs/: {}", file_path.display()))?;
    let mut parts = relative
        .with_extension("")
        .components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>();

    if parts.last().is_some_and(|part| part == "index") {
        parts.pop();
    }

    if parts.is_empty() {
        Ok("/".to_string())
    } else {
        Ok(format!("/{}/", parts.join("/")))
    }
}

fn page_sort_key(page: &DocPage) -> (u8, String) {
    if page.route == "/" {
        (0, String::new())
    } else {
        (1, page.source_path.clone())
    }
}

fn route_depth(route: &str) -> usize {
    route
        .trim_matches('/')
        .split('/')
        .filter(|part| !part.is_empty())
        .count()
        .saturating_sub(1)
}

fn infer_title(markdown: &str, file_path: &Path) -> String {
    for line in markdown.lines() {
        if let Some(title) = line.strip_prefix("# ") {
            let title = title.trim().trim_end_matches('#').trim();
            if !title.is_empty() {
                return title.to_string();
            }
        }
    }

    file_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map_or_else(|| "Untitled".to_string(), titleize_slug)
}

fn titleize_slug(slug: &str) -> String {
    slug.split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            chars.next().map_or_else(String::new, |first| {
                format!("{}{}", first.to_uppercase(), chars.as_str())
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_markdown_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::Normal(part) => normalized.push(part),
        }
    }
    normalized
}

fn slash_path(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_string_lossy().to_string()),
            Component::RootDir => Some(String::new()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests;
