//! Ensures a repo's `.gitignore` covers a derived, regenerable path.
//!
//! Used for output directories that should never be committed by accident
//! (the Provenance cache, for example) without requiring the caller to manage
//! `.gitignore` by hand.

use anyhow::Context;
use provenance_macros::rule;

/// A generated output directory gets exactly one ignore line in the repo's
/// `.gitignore`: added if it is missing, never added twice, whatever
/// whitespace or line-ending style the existing file uses.
///
/// A line already covers `pattern` when its text, with surrounding
/// whitespace (including a `\r` left by CRLF endings) removed, is exactly
/// `pattern`. Anything else is a different git pattern -- `/.provenance/cache`
/// anchors to the repo root, `.provenance/cache` also matches a file, and
/// `.provenance/` covers more than the output directory -- so none of them
/// count as the line being present.
///
/// The projection appends to `existing`, so nothing already in it moves or
/// changes. The caller passes empty bytes for a missing file. Bytes that are
/// not UTF-8 are an error rather than an empty file, because appending to
/// them would add a line to a file whose contents were never checked for the
/// pattern.
#[rule("rule_generated_outputs_ignored")]
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
    use provenance_macros::verifies;

    /// The output directory that `init` passes in.
    const TARGET: &str = ".provenance/cache/";

    fn project(existing: &str) -> String {
        String::from_utf8(project_ignored(existing.as_bytes(), TARGET).unwrap()).unwrap()
    }

    #[test]
    #[verifies("rule_generated_outputs_ignored", examples)]
    fn adds_the_pattern_to_an_empty_file() {
        assert_eq!(project(""), ".provenance/cache/\n");
    }

    #[test]
    #[verifies("rule_generated_outputs_ignored", examples)]
    fn appends_to_an_existing_gitignore_without_disturbing_its_content() {
        assert_eq!(
            project("target/\n*.db\n"),
            "target/\n*.db\n.provenance/cache/\n"
        );
    }

    #[test]
    #[verifies("rule_generated_outputs_ignored", examples)]
    fn adds_a_newline_before_appending_when_the_file_lacks_a_trailing_one() {
        assert_eq!(project("target/"), "target/\n.provenance/cache/\n");
    }

    #[test]
    #[verifies("rule_generated_outputs_ignored", examples)]
    fn is_idempotent_when_the_pattern_is_already_present() {
        assert_eq!(project(".provenance/cache/\n"), ".provenance/cache/\n");
    }

    #[test]
    #[verifies("rule_generated_outputs_ignored", examples)]
    fn matches_the_pattern_regardless_of_surrounding_whitespace() {
        assert_eq!(
            project("  .provenance/cache/  \n"),
            "  .provenance/cache/  \n"
        );
    }

    /// Bytes that are not UTF-8 are not an empty file: the projection fails
    /// rather than appending a line to a file it never managed to search for
    /// the pattern.
    #[test]
    #[verifies("rule_generated_outputs_ignored", examples)]
    fn refuses_to_append_to_a_gitignore_it_cannot_read() {
        let error = project_ignored(b"target/\n\xff\xfe not utf-8\n", TARGET).unwrap_err();

        assert!(
            error.to_string().contains(".gitignore"),
            "unhelpful error: {error}"
        );
    }

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
        "  .provenance/cache/  ",
        "\t.provenance/cache/ ",
        "/.provenance/cache/",
        ".provenance/cache",
        ".provenance/cache/*",
        "#.provenance/cache/",
        ".provenance/",
        "target/",
        "*.db",
        "!keep.txt",
        "",
    ];

    /// The state of `.gitignore` before the call: empty when the file is
    /// absent, otherwise its exact bytes. Composes a random run of pooled
    /// lines under one line-ending style, with or without the final
    /// terminator, so the empty file and the unterminated file both show up.
    fn prior_state(rng: &mut Rng) -> String {
        if rng.below(6) == 0 {
            return String::new();
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
        contents
    }

    fn prior_states(seed: u64, count: usize) -> Vec<String> {
        let mut rng = Rng(seed);
        (0..count).map(|_| prior_state(&mut rng)).collect()
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
        for (case, before) in prior_states(0x6017_0001, 512).into_iter().enumerate() {
            let projected = project(&before);
            let changed = projected != before;
            let after = ignore_lines(&projected);

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
        for (case, before) in prior_states(0x6017_0002, 512).into_iter().enumerate() {
            let after = project(&before);

            assert!(
                keeps_every_line(&before, &after),
                "case {case}: {before:?} was not left intact, became {after:?}"
            );
        }
    }

    #[test]
    #[verifies("rule_generated_outputs_ignored", property)]
    fn a_second_call_changes_nothing_the_first_did_not() {
        for (case, before) in prior_states(0x6017_0003, 512).into_iter().enumerate() {
            let once = project(&before);
            let twice = project(&once);

            assert_eq!(twice, once, "case {case}: the second call changed {once:?}");
        }
    }
}
