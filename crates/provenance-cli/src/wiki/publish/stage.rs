use super::manifest::OwnershipManifest;
use super::transaction::{replace_output, OutputState, StageIdentity, TransactionDirectory};
use super::{
    PublicationOutput, PublishError, PublishReport, PublishedPage, GENERATOR, MANIFEST_VERSION,
    OWNERSHIP_MANIFEST,
};
use crate::safe_fs::Directory;
use crate::wiki::model::WikiCorpus;
use crate::wiki::{render, theme};
use camino::Utf8Path;
use provenance_macros::rule;
use std::fs::File;
use std::io::Write;

pub(super) struct StageDirectory {
    root: Directory,
    identity: StageIdentity,
}

impl StageDirectory {
    pub(super) fn from_file(root: File, path: &Utf8Path) -> Result<Self, PublishError> {
        // The identity is bound to the handle publication already holds, so
        // nothing swapped in between creating the stage and recording it can
        // be vouched for later. Windows cannot read identity from that
        // handle (fs_at's mkdir_at hardcodes an access mask without
        // FILE_READ_ATTRIBUTES), so there alone the identity comes from a
        // second no-follow open of the same path.
        #[cfg(not(windows))]
        let identity = StageIdentity::from_file(&root)
            .map_err(|error| PublishError::io("record staging directory identity", path, error))?;
        #[cfg(windows)]
        let identity = {
            let handle = crate::safe_fs::Directory::open(path.as_std_path()).map_err(|error| {
                PublishError::io("open staging directory identity", path, error)
            })?;
            StageIdentity::from_file(handle.as_file()).map_err(|error| {
                PublishError::io("record staging directory identity", path, error)
            })?
        };
        Ok(Self {
            root: Directory::from_file(root, path.as_std_path().to_path_buf()),
            identity,
        })
    }

    pub(super) fn into_parts(self) -> (File, StageIdentity) {
        (self.root.into_file(), self.identity)
    }

    fn write_file(
        &self,
        relative: &[&str],
        contents: &[u8],
        display_path: &Utf8Path,
    ) -> Result<(), PublishError> {
        let mut directory = self.root.try_clone().map_err(|error| {
            PublishError::io("clone staging directory handle", display_path, error)
        })?;
        for segment in &relative[..relative.len() - 1] {
            directory = match directory.create_child(segment) {
                Ok(created) => created,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    // The attribute-bit check inside this open independently
                    // refuses a reparse point planted at the segment.
                    directory.open_child(segment).map_err(|error| {
                        PublishError::io("open staged directory", display_path, error)
                    })?
                }
                Err(error) => {
                    return Err(PublishError::io(
                        "create staged directory",
                        display_path,
                        error,
                    ))
                }
            };
        }
        let mut options = fs_at::OpenOptions::default();
        options
            .write(fs_at::OpenOptionsWriteMode::Write)
            .create_new(true)
            .follow(false);
        let mut file = options
            .open_at(directory.as_file(), relative[relative.len() - 1])
            .map_err(|error| PublishError::io("create staged file", display_path, error))?;
        file.write_all(contents)
            .map_err(|error| PublishError::io("write staged file", display_path, error))
    }
}

pub(super) fn generate_and_replace(
    corpus: &WikiCorpus,
    output: PublicationOutput,
    transaction: &TransactionDirectory,
    output_state: OutputState,
    stage: &StageDirectory,
) -> Result<PublishReport, PublishError> {
    let paths = &transaction.paths;
    let pages = render::render_corpus(corpus);
    for page in &pages {
        write_page_in(stage, &paths.stage, &page.route, &page.html)?;
    }
    let stylesheet = paths.stage.join("assets/provenance-wiki.css");
    stage.write_file(
        &["assets", "provenance-wiki.css"],
        theme::WIKI_CSS.as_bytes(),
        &stylesheet,
    )?;
    let manifest = serde_json::to_vec_pretty(&OwnershipManifest {
        generator: GENERATOR.into(),
        version: MANIFEST_VERSION,
    })
    .expect("ownership manifest is always serializable");
    let manifest_path = paths.stage.join(OWNERSHIP_MANIFEST);
    stage.write_file(&[OWNERSHIP_MANIFEST], &manifest, &manifest_path)?;
    transaction.validate_output(
        output.path.file_name().expect("validated output leaf"),
        &output.path,
        output.policy,
    )?;
    let cleanup_warnings = replace_output(
        &output.path,
        output.policy,
        transaction,
        output_state,
        &stage.identity,
    )?;

    Ok(PublishReport {
        status: "ok",
        scope: corpus.scope.clone(),
        out: output.path,
        page_count: pages.len(),
        pages: pages
            .into_iter()
            .map(|page| PublishedPage {
                route: page.route,
                title: page.title,
            })
            .collect(),
        cleanup_warnings,
    })
}

#[cfg(test)]
pub(super) fn write_page(stage: &Utf8Path, route: &str, html: &str) -> Result<(), PublishError> {
    let root = crate::safe_fs::Directory::open(stage.as_std_path())
        .map(crate::safe_fs::Directory::into_file)
        .map_err(|error| PublishError::io("open staging directory", stage, error))?;
    let stage_directory = StageDirectory::from_file(root, stage)?;
    write_page_in(&stage_directory, stage, route, html)
}

/// What the publisher may do with a route it was asked to write.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum RouteVerdict<'a> {
    /// Safe to stage, and these are the directory names it names, in order.
    Safe(Vec<&'a str>),
    /// The route does not begin and end with a slash.
    Unbounded,
    /// A segment cannot be used as a name under the staging directory.
    UnsafeSegment(&'a str),
}

/// Decides whether a generated page may be written to a route.
///
/// A page's route becomes the directories the publisher creates under the
/// staging root, so a hostile or buggy route must not be able to name
/// anything outside it. A route is safe only when it begins and ends with a
/// slash and every segment between those slashes is a plain name: never `.`
/// or `..`, never empty, and free of the separators (`\`) and terminators
/// (`\0`) another platform or a syscall could read as further path
/// structure. Nothing here reshapes a route: a route is either used exactly
/// as given or refused.
#[rule("rule_page_route_safety")]
pub(super) fn classify_route(route: &str) -> RouteVerdict<'_> {
    if route == "/" {
        return RouteVerdict::Safe(Vec::new());
    }
    let Some(inner) = route
        .strip_prefix('/')
        .and_then(|rest| rest.strip_suffix('/'))
    else {
        return RouteVerdict::Unbounded;
    };
    let mut segments = Vec::new();
    for segment in inner.split('/') {
        if segment.is_empty() || matches!(segment, "." | "..") || segment.contains(['\\', '\0']) {
            return RouteVerdict::UnsafeSegment(segment);
        }
        segments.push(segment);
    }
    RouteVerdict::Safe(segments)
}

fn write_page_in(
    stage_directory: &StageDirectory,
    stage: &Utf8Path,
    route: &str,
    html: &str,
) -> Result<(), PublishError> {
    let mut relative = match classify_route(route) {
        RouteVerdict::Safe(segments) => segments,
        RouteVerdict::Unbounded => {
            return Err(PublishError::InvalidRoute {
                route: route.to_string(),
                detail: "routes must begin and end with '/'".to_string(),
            })
        }
        RouteVerdict::UnsafeSegment(segment) => {
            return Err(PublishError::InvalidRoute {
                route: route.to_string(),
                detail: format!("unsafe path segment {segment:?}"),
            })
        }
    };
    let mut path = stage.to_path_buf();
    for segment in &relative {
        path.push(segment);
    }
    relative.push("index.html");
    path.push("index.html");
    stage_directory.write_file(&relative, html.as_bytes(), &path)
}
