//! Reads the texts of one file at a list of commits, one text at a time.

use super::versions::FileVersions;
use std::string::FromUtf8Error;

/// Gives the text of `path` at each commit of `commits`, in order, to `each`,
/// and keeps no text after `each` returns.
///
/// An error of a text ends the read at once and is its error, except the
/// [`FromUtf8Error`] of a text that is not UTF-8. After that error the read
/// asks for the remaining texts and gives no more texts to `each`. Another
/// error among them ends the read and wins. Else the first UTF-8 error wins.
/// The rule is the same for each adapter.
pub(super) async fn each_text(
    history: &dyn FileVersions,
    path: &str,
    commits: &[&str],
    mut each: impl FnMut(Option<String>) + Send,
) -> anyhow::Result<()> {
    let mut not_utf8 = None;
    for commit in commits {
        match history.text(path, commit).await {
            Ok(text) if not_utf8.is_none() => each(text),
            Ok(_) => {}
            Err(error) if error.is::<FromUtf8Error>() => {
                not_utf8.get_or_insert(error);
            }
            Err(error) => return Err(error),
        }
    }
    not_utf8.map_or(Ok(()), Err)
}
