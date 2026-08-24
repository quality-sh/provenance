use provenance_store::code_refs::{CodeRef, LineRange};
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum GitHost {
    GitHub,
    GitLab,
    Bitbucket,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitRemote {
    pub host: GitHost,
    pub owner: String,
    pub repo: String,
}

/// Parses an `origin` remote URL into a known git host, owner, and repo.
///
/// Accepts `https://`, `http://`, `ssh://`, and scp-style `git@host:path`
/// forms. Unknown hosts and local paths return `None`.
pub fn parse_git_remote(url: &str) -> Option<GitRemote> {
    let url = url.trim();
    let (host, path) = if let Some(rest) = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .or_else(|| url.strip_prefix("ssh://"))
    {
        let rest = rest.split_once('@').map_or(rest, |(_, tail)| tail);
        rest.split_once('/')?
    } else {
        url.strip_prefix("git@")?.split_once(':')?
    };
    let host = match host {
        "github.com" | "www.github.com" => GitHost::GitHub,
        "gitlab.com" | "www.gitlab.com" => GitHost::GitLab,
        "bitbucket.org" | "www.bitbucket.org" => GitHost::Bitbucket,
        _ => return None,
    };
    let path = path.trim_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let (owner, repo) = path.rsplit_once('/')?;
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    Some(GitRemote {
        host,
        owner: owner.to_string(),
        repo: repo.to_string(),
    })
}

/// Builds a blob URL on the remote's host, anchored to the first line range.
pub fn blob_url(remote: &GitRemote, reference: &str, code_ref: &CodeRef) -> String {
    let anchor = code_ref
        .lines
        .first()
        .map_or_else(String::new, |range| line_anchor(remote.host, *range));
    let GitRemote { owner, repo, .. } = remote;
    let path = &code_ref.path;
    match remote.host {
        GitHost::GitHub => {
            format!("https://github.com/{owner}/{repo}/blob/{reference}/{path}{anchor}")
        }
        GitHost::GitLab => {
            format!("https://gitlab.com/{owner}/{repo}/-/blob/{reference}/{path}{anchor}")
        }
        GitHost::Bitbucket => {
            format!("https://bitbucket.org/{owner}/{repo}/src/{reference}/{path}{anchor}")
        }
    }
}

fn line_anchor(host: GitHost, range: LineRange) -> String {
    let LineRange { start, end } = range;
    match (host, end) {
        (GitHost::GitHub, Some(end)) => format!("#L{start}-L{end}"),
        (GitHost::GitLab, Some(end)) => format!("#L{start}-{end}"),
        (GitHost::Bitbucket, Some(end)) => format!("#lines-{start}:{end}"),
        (GitHost::GitHub | GitHost::GitLab, None) => format!("#L{start}"),
        (GitHost::Bitbucket, None) => format!("#lines-{start}"),
    }
}

/// Reads the `origin` remote URL of the repo at `repo`, if any.
pub fn detect_remote_url(repo: &Path) -> Option<String> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["config", "--get", "remote.origin.url"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let url = String::from_utf8(output.stdout).ok()?.trim().to_string();
    (!url.is_empty()).then_some(url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use provenance_macros::verifies;
    use provenance_store::code_refs::parse_code_ref;

    /// Derived from an exhaustive match so that adding a `GitHost` variant
    /// fails compilation until the new host joins the chain, keeping the
    /// proof below complete.
    fn all_hosts() -> Vec<GitHost> {
        let mut all = vec![GitHost::GitHub];
        while let Some(next) = match all.last().unwrap() {
            GitHost::GitHub => Some(GitHost::GitLab),
            GitHost::GitLab => Some(GitHost::Bitbucket),
            GitHost::Bitbucket => None,
        } {
            all.push(next);
        }
        all
    }

    /// The anchor each host's web UI understands, written out here rather
    /// than derived, so the proof does not restate `line_anchor` by calling
    /// it. Columns: no lines, a single line, a line range.
    const ANCHOR_TABLE: [(GitHost, [&str; 3]); 3] = [
        (GitHost::GitHub, ["", "#L153", "#L153-L156"]),
        (GitHost::GitLab, ["", "#L153", "#L153-156"]),
        (GitHost::Bitbucket, ["", "#lines-153", "#lines-153:156"]),
    ];

    const LINE_SHAPES: [&str; 3] = ["", ":153", ":153-156"];

    fn github_remote() -> GitRemote {
        GitRemote {
            host: GitHost::GitHub,
            owner: "exampleorg".to_string(),
            repo: "ex-api".to_string(),
        }
    }

    #[test]
    fn parse_git_remote_reads_https_github_urls() {
        let remote = parse_git_remote("https://github.com/exampleorg/ex-api.git").unwrap();
        assert_eq!(remote.host, GitHost::GitHub);
        assert_eq!(remote.owner, "exampleorg");
        assert_eq!(remote.repo, "ex-api");
    }

    #[test]
    fn parse_git_remote_accepts_missing_git_suffix_and_trailing_slash() {
        let remote = parse_git_remote("https://github.com/exampleorg/ex-api/").unwrap();
        assert_eq!(remote.owner, "exampleorg");
        assert_eq!(remote.repo, "ex-api");
    }

    #[test]
    fn parse_git_remote_reads_scp_style_ssh_urls() {
        let remote = parse_git_remote("git@github.com:exampleorg/ex-api.git").unwrap();
        assert_eq!(remote.host, GitHost::GitHub);
        assert_eq!(remote.owner, "exampleorg");
        assert_eq!(remote.repo, "ex-api");
    }

    #[test]
    fn parse_git_remote_keeps_gitlab_subgroups_in_the_owner() {
        let remote = parse_git_remote("ssh://git@gitlab.com/group/subgroup/repo.git").unwrap();
        assert_eq!(remote.host, GitHost::GitLab);
        assert_eq!(remote.owner, "group/subgroup");
        assert_eq!(remote.repo, "repo");
    }

    #[test]
    fn parse_git_remote_reads_bitbucket_urls() {
        let remote = parse_git_remote("https://bitbucket.org/team/repo.git").unwrap();
        assert_eq!(remote.host, GitHost::Bitbucket);
        assert_eq!(remote.owner, "team");
        assert_eq!(remote.repo, "repo");
    }

    /// Every host x line-shape pair the address builder can be asked for:
    /// 3 hosts on the allowlist, each with a range, a single line, and no
    /// lines at all. The domain is finite, so this is a proof, not a sample.
    #[test]
    #[verifies("rule_wiki_reference_links", exhaustion)]
    fn blob_urls_anchor_every_host_and_line_shape() {
        assert_eq!(
            ANCHOR_TABLE.len(),
            all_hosts().len(),
            "the anchor table and the host list disagree on how many hosts there are"
        );
        for host in all_hosts() {
            let (_, anchors) = ANCHOR_TABLE
                .iter()
                .find(|(row, _)| *row == host)
                .unwrap_or_else(|| panic!("{host:?} has no row in the anchor table"));
            let remote = GitRemote {
                host,
                owner: "exampleorg".to_string(),
                repo: "ex-api".to_string(),
            };
            for (shape, expected) in LINE_SHAPES.iter().zip(anchors) {
                let code_ref = parse_code_ref(&format!("src/UseCase.php{shape}"))
                    .unwrap_or_else(|| panic!("`src/UseCase.php{shape}` is a code reference"));
                let url = blob_url(&remote, "HEAD", &code_ref);
                assert!(
                    url.starts_with("https://") && url.contains("src/UseCase.php"),
                    "{host:?} with `{shape}` built `{url}`, which is not an address for the file"
                );
                let anchor = url
                    .split_once('#')
                    .map_or_else(String::new, |(_, anchor)| format!("#{anchor}"));
                assert_eq!(anchor, *expected, "{host:?} with `{shape}` built `{url}`");
            }
        }
    }

    #[test]
    #[verifies("rule_wiki_reference_links", examples)]
    fn parse_git_remote_rejects_unknown_hosts_and_local_paths() {
        assert!(parse_git_remote("https://git.internal.example/owner/repo.git").is_none());
        assert!(parse_git_remote("/srv/git/repo.git").is_none());
        assert!(parse_git_remote("").is_none());
    }

    #[test]
    fn blob_url_anchors_github_line_ranges() {
        let code_ref = parse_code_ref("src/UseCase.php:153-156").unwrap();
        assert_eq!(
            blob_url(&github_remote(), "HEAD", &code_ref),
            "https://github.com/exampleorg/ex-api/blob/HEAD/src/UseCase.php#L153-L156"
        );
    }

    #[test]
    fn blob_url_anchors_github_single_lines() {
        let code_ref = parse_code_ref("src/UseCase.php:153").unwrap();
        assert_eq!(
            blob_url(&github_remote(), "abc1234", &code_ref),
            "https://github.com/exampleorg/ex-api/blob/abc1234/src/UseCase.php#L153"
        );
    }

    #[test]
    fn blob_url_uses_gitlab_dash_blob_layout() {
        let remote = GitRemote {
            host: GitHost::GitLab,
            owner: "group/subgroup".to_string(),
            repo: "repo".to_string(),
        };
        let code_ref = parse_code_ref("src/UseCase.php:59-69").unwrap();
        assert_eq!(
            blob_url(&remote, "HEAD", &code_ref),
            "https://gitlab.com/group/subgroup/repo/-/blob/HEAD/src/UseCase.php#L59-69"
        );
    }

    #[test]
    fn blob_url_uses_bitbucket_src_layout() {
        let remote = GitRemote {
            host: GitHost::Bitbucket,
            owner: "team".to_string(),
            repo: "repo".to_string(),
        };
        let code_ref = parse_code_ref("src/UseCase.php:153-156").unwrap();
        assert_eq!(
            blob_url(&remote, "HEAD", &code_ref),
            "https://bitbucket.org/team/repo/src/HEAD/src/UseCase.php#lines-153:156"
        );
    }

    #[test]
    fn blob_url_omits_the_anchor_without_lines() {
        let code_ref = parse_code_ref("docs/save-invoice.md").unwrap();
        assert_eq!(
            blob_url(&github_remote(), "HEAD", &code_ref),
            "https://github.com/exampleorg/ex-api/blob/HEAD/docs/save-invoice.md"
        );
    }

    #[test]
    fn detect_remote_url_reads_the_origin_remote() {
        let dir = tempfile::tempdir().unwrap();
        let run = |args: &[&str]| {
            let status = std::process::Command::new("git")
                .arg("-C")
                .arg(dir.path())
                .args(args)
                .status()
                .unwrap();
            assert!(status.success());
        };
        run(&["init", "--quiet"]);
        run(&[
            "remote",
            "add",
            "origin",
            "git@github.com:exampleorg/ex-api.git",
        ]);
        assert_eq!(
            detect_remote_url(dir.path()).as_deref(),
            Some("git@github.com:exampleorg/ex-api.git")
        );
    }

    #[test]
    fn detect_remote_url_returns_none_outside_a_repo() {
        let dir = tempfile::tempdir().unwrap();
        assert!(detect_remote_url(dir.path()).is_none());
    }
}
