//! Sends the Provenance state files of a repository to the
//! `provenance-jsonl` merge driver through its `.gitattributes`.

use provenance_macros::rule;

/// The `.gitattributes` line that makes git merge each state file with the
/// `provenance-jsonl` merge driver.
pub const STATE_MERGE_LINE: &str = ".provenance/state/**/*.jsonl merge=provenance-jsonl";

/// Returns the bytes of `.gitattributes` with [`STATE_MERGE_LINE`] appended,
/// or `existing` unchanged when a line, without surrounding whitespace, is
/// exactly that line.
///
/// The caller passes empty bytes for a missing file. Bytes that are not
/// UTF-8 are an error, so init stops before its first write.
#[rule("rule_init_routes_state_merges")]
pub fn project_state_merge(existing: &[u8]) -> anyhow::Result<Vec<u8>> {
    crate::gitignore::append_line(existing, STATE_MERGE_LINE, ".gitattributes")
}

#[cfg(test)]
mod tests {
    use super::*;
    use provenance_macros::verifies;

    fn project(existing: &str) -> String {
        String::from_utf8(project_state_merge(existing.as_bytes()).unwrap()).unwrap()
    }

    #[test]
    #[verifies("rule_init_routes_state_merges", examples)]
    fn a_missing_file_gets_only_the_line() {
        assert_eq!(project(""), format!("{STATE_MERGE_LINE}\n"));
    }

    #[test]
    #[verifies("rule_init_routes_state_merges", examples)]
    fn appends_after_the_existing_lines() {
        assert_eq!(
            project("*.png binary\r\nskills/** text eol=lf"),
            format!("*.png binary\r\nskills/** text eol=lf\n{STATE_MERGE_LINE}\n")
        );
    }

    #[test]
    #[verifies("rule_init_routes_state_merges", examples)]
    fn keeps_a_file_that_has_the_line() {
        let existing = format!("# state\n  {STATE_MERGE_LINE}\r\n*.png binary\n");
        assert_eq!(project(&existing), existing);
    }

    #[test]
    #[verifies("rule_init_routes_state_merges", examples)]
    fn a_different_attribute_for_the_state_files_is_not_the_line() {
        let existing = ".provenance/state/**/*.jsonl merge=union\n";
        assert_eq!(project(existing), format!("{existing}{STATE_MERGE_LINE}\n"));
    }

    #[test]
    #[verifies("rule_init_routes_state_merges", examples)]
    fn refuses_bytes_that_are_not_utf8() {
        let error = project_state_merge(b"*.png binary\n\xff\n").unwrap_err();
        assert_eq!(error.to_string(), ".gitattributes is not valid UTF-8");
    }

    /// Each file that this test makes from the lines below: absent, or one
    /// to four lines with LF or CRLF endings, with or without a last ending.
    fn files() -> Vec<String> {
        let lines = [
            STATE_MERGE_LINE.to_owned(),
            format!("\t{STATE_MERGE_LINE}  "),
            ".provenance/state/**/*.jsonl merge=union".to_owned(),
            "*.png binary".to_owned(),
            String::new(),
        ];
        let mut files = vec![String::new()];
        for count in 1..=4u32 {
            for mut index in 0..lines.len().pow(count) {
                let mut picked = Vec::new();
                for _ in 0..count {
                    picked.push(lines[index % lines.len()].as_str());
                    index /= lines.len();
                }
                for ending in ["\n", "\r\n"] {
                    let body = picked.join(ending);
                    files.push(body.clone());
                    files.push(format!("{body}{ending}"));
                }
            }
        }
        files
    }

    fn line_count(text: &str) -> usize {
        text.lines()
            .filter(|line| line.trim() == STATE_MERGE_LINE)
            .count()
    }

    #[test]
    #[verifies("rule_init_routes_state_merges", property)]
    fn each_file_gets_the_line_once_and_keeps_its_bytes() {
        for before in files() {
            let after = project(&before);
            assert_eq!(line_count(&after), line_count(&before).max(1), "{before:?}");
            if line_count(&before) > 0 {
                assert_eq!(after, before);
            } else {
                assert!(after.starts_with(&before), "{before:?} became {after:?}");
            }
            assert_eq!(project(&after), after, "a second call changed {after:?}");
        }
    }
}
