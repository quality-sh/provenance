//! Ensures a repo's `.gitignore` covers a derived, regenerable path.
//!
//! Used for output directories that should never be committed by accident
//! (the generated wiki, for example) without requiring the caller to manage
//! `.gitignore` by hand.

use crate::atomic_file::{atomic_replace, FileSnapshot};
use anyhow::Context;
use camino::Utf8Path;
use provenance_macros::rule;

/// A generated output directory gets exactly one ignore line in the repo's
/// `.gitignore`: added if it is missing, never added twice, whatever
/// whitespace or line-ending style the existing file uses.
///
/// A line already covers `pattern` when its text, with surrounding
/// whitespace (including a `\r` left by CRLF endings) removed, is exactly
/// `pattern`. Anything else is a different git pattern -- `/.provenance/wiki`
/// anchors to the repo root, `.provenance/wiki` also matches a file, and
/// `.provenance/` covers more than the output directory -- so none of them
/// count as the line being present.
///
/// The file is created when absent and appended to otherwise, so nothing
/// already in it moves or changes. `pattern` must be one line with no
/// surrounding whitespace, or the line written back would not match itself.
/// Returns whether the file was created or modified.
///
/// Only a missing `.gitignore` counts as an empty one. A file that exists but
/// cannot be read - unreadable, not UTF-8 - is an error rather than an empty
/// file, because appending to it would add a line to a file whose contents
/// were never checked for the pattern.
#[rule("rule_generated_outputs_ignored")]
pub fn ensure_ignored(repo: &Utf8Path, pattern: &str) -> anyhow::Result<bool> {
    debug_assert!(
        pattern.trim() == pattern && !pattern.contains(['\n', '\r']),
        "ignore pattern must be one line with no surrounding whitespace: {pattern:?}"
    );
    let path = repo.join(".gitignore");
    let before = FileSnapshot::read(path.as_std_path())
        .with_context(|| format!("failed to read {path} to check for {pattern}"))?;
    let updated = project_ignored(before.bytes().unwrap_or_default(), pattern)?;
    if before.bytes() == Some(updated.as_slice()) {
        return Ok(false);
    }
    atomic_replace(path.as_std_path(), &before, &updated)?;
    Ok(true)
}

pub fn project_ignored(existing: &[u8], pattern: &str) -> anyhow::Result<Vec<u8>> {
    let existing = std::str::from_utf8(existing).context(".gitignore is not valid UTF-8")?;
    if existing.lines().any(|line| line.trim() == pattern) {
        return Ok(existing.as_bytes().to_vec());
    }
    let mut updated = existing.as_bytes().to_vec();
    if !updated.is_empty() && !updated.ends_with(b"\n") {
        updated.push(b'\n');
    }
    updated.extend_from_slice(pattern.as_bytes());
    updated.push(b'\n');
    Ok(updated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;
    use provenance_macros::verifies;

    fn repo_dir() -> (tempfile::TempDir, Utf8PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let repo = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        (dir, repo)
    }

    #[test]
    #[verifies("rule_generated_outputs_ignored", examples)]
    fn creates_gitignore_when_missing_and_adds_the_pattern() {
        let (_dir, repo) = repo_dir();
        let changed = ensure_ignored(&repo, ".provenance/wiki/").unwrap();
        assert!(changed);
        let contents = std::fs::read_to_string(repo.join(".gitignore")).unwrap();
        assert_eq!(contents, ".provenance/wiki/\n");
    }

    #[test]
    #[verifies("rule_generated_outputs_ignored", examples)]
    fn appends_to_an_existing_gitignore_without_disturbing_its_content() {
        let (_dir, repo) = repo_dir();
        std::fs::write(repo.join(".gitignore"), "target/\n*.db\n").unwrap();
        let changed = ensure_ignored(&repo, ".provenance/wiki/").unwrap();
        assert!(changed);
        let contents = std::fs::read_to_string(repo.join(".gitignore")).unwrap();
        assert_eq!(contents, "target/\n*.db\n.provenance/wiki/\n");
    }

    #[test]
    #[verifies("rule_generated_outputs_ignored", examples)]
    fn adds_a_newline_before_appending_when_the_file_lacks_a_trailing_one() {
        let (_dir, repo) = repo_dir();
        std::fs::write(repo.join(".gitignore"), "target/").unwrap();
        ensure_ignored(&repo, ".provenance/wiki/").unwrap();
        let contents = std::fs::read_to_string(repo.join(".gitignore")).unwrap();
        assert_eq!(contents, "target/\n.provenance/wiki/\n");
    }

    #[test]
    #[verifies("rule_generated_outputs_ignored", examples)]
    fn is_idempotent_when_the_pattern_is_already_present() {
        let (_dir, repo) = repo_dir();
        std::fs::write(repo.join(".gitignore"), ".provenance/wiki/\n").unwrap();
        let changed = ensure_ignored(&repo, ".provenance/wiki/").unwrap();
        assert!(!changed);
        let contents = std::fs::read_to_string(repo.join(".gitignore")).unwrap();
        assert_eq!(contents, ".provenance/wiki/\n");
    }

    #[test]
    #[verifies("rule_generated_outputs_ignored", examples)]
    fn matches_the_pattern_regardless_of_surrounding_whitespace() {
        let (_dir, repo) = repo_dir();
        std::fs::write(repo.join(".gitignore"), "  .provenance/wiki/  \n").unwrap();
        let changed = ensure_ignored(&repo, ".provenance/wiki/").unwrap();
        assert!(!changed);
    }

    /// A `.gitignore` that cannot be read is not an empty one: the call fails
    /// and leaves the file exactly as it found it, rather than appending a
    /// line to a file it never managed to search for the pattern.
    #[test]
    #[verifies("rule_generated_outputs_ignored", examples)]
    fn refuses_to_append_to_a_gitignore_it_cannot_read() {
        let (_dir, repo) = repo_dir();
        let path = repo.join(".gitignore");
        let unreadable = b"target/\n\xff\xfe not utf-8\n";
        std::fs::write(&path, unreadable).unwrap();

        let error = ensure_ignored(&repo, ".provenance/wiki/").unwrap_err();

        assert!(
            error.to_string().contains(".gitignore"),
            "unhelpful error: {error}"
        );
        assert_eq!(std::fs::read(&path).unwrap(), unreadable);
    }

    /// The output directory the wiki builder passes in.
    const TARGET: &str = ".provenance/wiki/";

    /// `SplitMix64`. The seeds below are fixed, so every run walks the same
    /// generated files and a failure is reproducible from the test name.
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

        fn pick(&mut self, choices: &[&'static str]) -> &'static str {
            choices[self.below(choices.len())]
        }
    }

    /// Lines a `.gitignore` might already hold. The first three are the
    /// target under the whitespace and CRLF padding a real file carries; the
    /// next four are near-misses that read as different git patterns and so
    /// must not be taken for the target; the rest are ordinary neighbours,
    /// including a blank line.
    const LINE_POOL: &[&str] = &[
        TARGET,
        "  .provenance/wiki/  ",
        "\t.provenance/wiki/ ",
        "/.provenance/wiki/",
        ".provenance/wiki",
        ".provenance/wiki/*",
        "#.provenance/wiki/",
        ".provenance/",
        "target/",
        "*.db",
        "!keep.txt",
        "",
    ];

    /// The state of `.gitignore` before the call: `None` when the file is
    /// absent, otherwise its exact bytes. Composes a random run of pooled
    /// lines under one line-ending style, with or without the final
    /// terminator, so the empty file and the unterminated file both show up.
    fn prior_state(rng: &mut Rng) -> Option<String> {
        if rng.below(6) == 0 {
            return None;
        }
        let separator = if rng.below(2) == 0 { "\n" } else { "\r\n" };
        let mut contents = String::new();
        for index in 0..rng.below(6) {
            if index > 0 {
                contents.push_str(separator);
            }
            contents.push_str(rng.pick(LINE_POOL));
        }
        if !contents.is_empty() && rng.below(4) != 0 {
            contents.push_str(separator);
        }
        Some(contents)
    }

    fn prior_states(seed: u64, count: usize) -> Vec<Option<String>> {
        let mut rng = Rng(seed);
        (0..count).map(|_| prior_state(&mut rng)).collect()
    }

    fn write_prior(repo: &Utf8Path, prior: Option<&String>) {
        if let Some(prior) = prior {
            std::fs::write(repo.join(".gitignore"), prior).unwrap();
        }
    }

    fn read_back(repo: &Utf8Path) -> String {
        std::fs::read_to_string(repo.join(".gitignore")).unwrap()
    }

    /// Independent restatement of "a line ignoring the target", kept apart
    /// from the function's own reading: git takes one pattern per line, and
    /// neither surrounding blanks nor the `\r` of a CRLF ending belong to it.
    fn ignore_lines(contents: &str) -> usize {
        contents
            .split('\n')
            .filter(|line| line.trim_matches([' ', '\t', '\r']) == TARGET)
            .count()
    }

    /// Whether every line already in `before` is still in `after`, in order
    /// and byte for byte. A terminating newline ends the last line rather
    /// than starting another, so it is not counted as one.
    fn keeps_every_line(before: &str, after: &str) -> bool {
        let mut prior: Vec<&str> = before.split('\n').collect();
        if prior.last() == Some(&"") {
            prior.pop();
        }
        let after: Vec<&str> = after.split('\n').collect();
        after.len() >= prior.len() && after[..prior.len()] == prior[..]
    }

    #[test]
    #[verifies("rule_generated_outputs_ignored", property)]
    fn leaves_exactly_one_line_ignoring_the_target() {
        for (case, prior) in prior_states(0x6017_0001, 512).into_iter().enumerate() {
            let (_dir, repo) = repo_dir();
            write_prior(&repo, prior.as_ref());
            let before = prior.unwrap_or_default();

            let changed = ensure_ignored(&repo, TARGET).unwrap();
            let after = ignore_lines(&read_back(&repo));

            // One line where there was none, and not a second where there
            // was one already. A prior file holding several stays as it is:
            // the rule is that this call never adds a duplicate, not that it
            // tidies up lines it did not write.
            assert_eq!(
                after,
                ignore_lines(&before).max(1),
                "case {case}: wrong number of ignore lines for {before:?}"
            );
            assert_eq!(
                changed,
                ignore_lines(&before) == 0,
                "case {case}: reported change does not match what was needed for {before:?}"
            );
        }
    }

    #[test]
    #[verifies("rule_generated_outputs_ignored", property)]
    fn keeps_every_pre_existing_line_byte_identical() {
        for (case, prior) in prior_states(0x6017_0002, 512).into_iter().enumerate() {
            let (_dir, repo) = repo_dir();
            write_prior(&repo, prior.as_ref());
            let before = prior.unwrap_or_default();

            ensure_ignored(&repo, TARGET).unwrap();
            let after = read_back(&repo);

            assert!(
                keeps_every_line(&before, &after),
                "case {case}: {before:?} was not left intact, became {after:?}"
            );
        }
    }

    #[test]
    #[verifies("rule_generated_outputs_ignored", property)]
    fn a_second_call_changes_nothing_the_first_did_not() {
        for (case, prior) in prior_states(0x6017_0003, 512).into_iter().enumerate() {
            let (_dir, repo) = repo_dir();
            write_prior(&repo, prior.as_ref());

            ensure_ignored(&repo, TARGET).unwrap();
            let once = read_back(&repo);
            let changed_again = ensure_ignored(&repo, TARGET).unwrap();
            let twice = read_back(&repo);

            assert!(
                !changed_again,
                "case {case}: reported a change on the second call over {once:?}"
            );
            assert_eq!(twice, once, "case {case}: the second call rewrote the file");
        }
    }
}
