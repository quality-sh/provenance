//! Reads the texts of one file at a list of commits, one text at a time.

use super::git::{answered_by_git, Failed};
use super::versions::FileVersions;

/// Gives the text of `path` at each commit of `commits`, in order, to `each`,
/// and keeps no text after `each` returns.
///
/// The first error ends the read, except an error of the git adapter. After
/// that error the read goes on through the remaining commits and gives no
/// more texts to `each`. A failure of the Git process among them wins over
/// the first error, as when one Git process read every text of the file.
/// Else the first error wins. No other adapter can give that failure, so
/// the read asks another adapter for no text after an error.
pub(super) async fn each_text(
    history: &dyn FileVersions,
    path: &str,
    commits: &[&str],
    mut each: impl FnMut(Option<String>) + Send,
) -> anyhow::Result<()> {
    let mut first = None;
    for commit in commits {
        let (text, git) = answered_by_git(|| history.text(path, commit));
        match text.await {
            Ok(text) if first.is_none() => each(text),
            Ok(_) => {}
            Err(error) if git && !error.is::<Failed>() => {
                first.get_or_insert(error);
            }
            Err(error) => return Err(error),
        }
    }
    first.map_or(Ok(()), Err)
}
