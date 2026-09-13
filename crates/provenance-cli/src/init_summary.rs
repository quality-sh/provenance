//! The one coherent ending `provenance init` prints after a completed run:
//! a status line, a what-Provenance-is tagline, a new-versus-changed file
//! inventory, numbered next steps, a docs link, and the dictionary report
//! with its required attribution.

const DOCS_LINK: &str = "https://github.com/quality-sh/provenance/tree/main/docs";
const TAGLINE: &str = "Never lose the why behind your decisions.";

/// What `provenance init` prints after the writes are committed.
#[derive(Debug)]
pub enum InitEnding {
    /// Nothing would change; one short line says so. A pending dictionary
    /// decision still gets its guidance text.
    Already {
        line: String,
        dictionary: Option<String>,
    },
    /// A full summary of what the run did.
    Applied(Box<InitSummary>),
}

impl InitEnding {
    pub fn print(&self) {
        match self {
            Self::Already { line, dictionary } => {
                println!("{line}");
                if let Some(dictionary) = dictionary {
                    println!();
                    println!("{dictionary}");
                }
            }
            Self::Applied(summary) => summary.print(),
        }
    }

    /// Prints only the dictionary part. The Cargo initializer prints its own
    /// status line and keeps only this part of the summary.
    pub fn print_dictionary(&self) {
        match self {
            Self::Already { dictionary, .. } => {
                if let Some(dictionary) = dictionary {
                    println!("{dictionary}");
                }
            }
            Self::Applied(summary) => {
                if let Some(dictionary) = &summary.dictionary {
                    println!("{dictionary}");
                }
            }
        }
    }
}

/// The full summary of a completed init run.
#[derive(Debug)]
pub struct InitSummary {
    status_line: String,
    new_files: Vec<InventoryEntry>,
    changed_files: Vec<InventoryEntry>,
    steps: Vec<String>,
    dictionary: Option<String>,
}

#[derive(Debug)]
struct InventoryEntry {
    path: String,
    note: String,
}

impl InitSummary {
    pub const fn new(status_line: String, dictionary: Option<String>) -> Self {
        Self {
            status_line,
            new_files: Vec::new(),
            changed_files: Vec::new(),
            steps: Vec::new(),
            dictionary,
        }
    }

    pub fn push_new(&mut self, path: impl Into<String>, note: impl Into<String>) {
        self.new_files.push(InventoryEntry {
            path: path.into(),
            note: note.into(),
        });
    }

    pub fn push_changed(&mut self, path: impl Into<String>, note: impl Into<String>) {
        self.changed_files.push(InventoryEntry {
            path: path.into(),
            note: note.into(),
        });
    }

    pub fn push_step(&mut self, command: String) {
        self.steps.push(command);
    }

    fn print(&self) {
        let mut stdout = std::io::stdout().lock();
        let _ignored = self.write_to(&mut stdout);
    }

    pub fn write_to(&self, out: &mut impl std::io::Write) -> std::io::Result<()> {
        writeln!(out, "{}", self.status_line)?;
        writeln!(out)?;
        writeln!(out, "{TAGLINE}")?;
        writeln!(out)?;
        write_inventory(out, "New", &self.new_files)?;
        write_inventory(out, "Changed", &self.changed_files)?;
        writeln!(out)?;
        writeln!(out, "Next steps")?;
        for (index, step) in self.steps.iter().enumerate() {
            writeln!(out, "  {}. {step}", index + 1)?;
        }
        writeln!(out)?;
        writeln!(out, "Docs: {DOCS_LINK}")?;
        if let Some(dictionary) = &self.dictionary {
            writeln!(out)?;
            writeln!(out, "{dictionary}")?;
        }
        Ok(())
    }
}

fn write_inventory(
    out: &mut impl std::io::Write,
    label: &str,
    entries: &[InventoryEntry],
) -> std::io::Result<()> {
    if entries.is_empty() {
        return Ok(());
    }
    writeln!(out, "{label}")?;
    for entry in entries {
        writeln!(out, "  {} ({})", entry.path, entry.note)?;
    }
    Ok(())
}

/// Joins counted skill changes into one inventory note, for example
/// "added 4 skills" or "updated 1 skill, removed 1 file".
pub fn skill_note(installed: usize, updated: usize, removed: usize, noun: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    if installed > 0 {
        parts.push(counted("added", installed, noun));
    }
    if updated > 0 {
        parts.push(counted("updated", updated, noun));
    }
    if removed > 0 {
        parts.push(counted("removed", removed, "file"));
    }
    parts.join(", ")
}

fn counted(verb: &str, count: usize, noun: &str) -> String {
    let plural = if count == 1 { "" } else { "s" };
    format!("{verb} {count} {noun}{plural}")
}

/// Formats the scope part of the status line: `scope "default"`, or
/// `scopes "a" and "b"` when a repository keeps several.
pub fn scope_phrase(scopes: &[String]) -> String {
    match scopes {
        [one] => format!("scope \"{one}\""),
        [first, second] => format!("scopes \"{first}\" and \"{second}\""),
        many => {
            let listed: Vec<String> = many.iter().map(|scope| format!("\"{scope}\"")).collect();
            format!("scopes {}", listed.join(", "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    fn render(print: impl FnOnce(&mut Vec<u8>)) -> String {
        let mut buffer = Vec::new();
        print(&mut buffer);
        String::from_utf8(buffer).unwrap()
    }

    #[test]
    fn skill_note_names_each_counted_change() {
        assert_eq!(skill_note(4, 0, 0, "skill"), "added 4 skills");
        assert_eq!(skill_note(1, 0, 0, "link"), "added 1 link");
        assert_eq!(skill_note(0, 1, 0, "skill"), "updated 1 skill");
        assert_eq!(skill_note(0, 0, 2, "skill"), "removed 2 files");
        assert_eq!(
            skill_note(3, 1, 0, "skill"),
            "added 3 skills, updated 1 skill"
        );
        assert_eq!(skill_note(0, 0, 0, "skill"), "");
    }

    #[test]
    fn scope_phrase_covers_one_and_many_scopes() {
        assert_eq!(scope_phrase(&["default".to_owned()]), "scope \"default\"");
        assert_eq!(
            scope_phrase(&["a".to_owned(), "b".to_owned()]),
            "scopes \"a\" and \"b\""
        );
        assert_eq!(
            scope_phrase(&["a".to_owned(), "b".to_owned(), "c".to_owned()]),
            "scopes \"a\", \"b\", \"c\""
        );
    }

    #[test]
    fn the_applied_summary_prints_every_section_in_order() {
        let mut summary = InitSummary::new(
            "Initialized Provenance for scope \"default\" in /repo".to_owned(),
            Some("Dictionary: ASD-STE100 Issue 9 is already imported.".to_owned()),
        );
        summary.push_new(".provenance/state", "scope \"default\" and manifest");
        summary.push_changed("AGENTS.md", "updated the Provenance section");
        summary.push_step("provenance prime --quiet".to_owned());

        let rendered = render(|out| {
            summary.write_to(out).unwrap();
        });

        assert_eq!(
            rendered,
            format!(
                "Initialized Provenance for scope \"default\" in /repo\n\
                 \n\
                 {TAGLINE}\n\
                 \n\
                 New\n\
                 \x20 .provenance/state (scope \"default\" and manifest)\n\
                 Changed\n\
                 \x20 AGENTS.md (updated the Provenance section)\n\
                 \n\
                 Next steps\n\
                 \x20 1. provenance prime --quiet\n\
                 \n\
                 Docs: {DOCS_LINK}\n\
                 \n\
                 Dictionary: ASD-STE100 Issue 9 is already imported.\n"
            )
        );
    }

    #[test]
    fn an_empty_inventory_section_is_dropped() {
        let mut summary = InitSummary::new(
            "Initialized Provenance for scope \"s\" in /repo".to_owned(),
            None,
        );
        summary.push_new(".provenance/state", "scope \"s\" and manifest");
        summary.push_step("provenance check --quiet".to_owned());

        let rendered = render(|out| {
            summary.write_to(out).unwrap();
        });

        assert!(!rendered.contains("Changed"));
        assert!(rendered.contains("New\n  .provenance/state (scope \"s\" and manifest)\n"));
        assert!(
            rendered.ends_with("Docs: https://github.com/quality-sh/provenance/tree/main/docs\n")
        );
    }

    #[test]
    fn the_already_ending_prints_one_line() {
        let ending = InitEnding::Already {
            line: "Provenance is already set up in /repo. No change.".to_owned(),
            dictionary: None,
        };

        let rendered = render(|out| match &ending {
            InitEnding::Already { line, .. } => {
                writeln!(out, "{line}").unwrap();
            }
            InitEnding::Applied(summary) => summary.write_to(out).unwrap(),
        });

        assert_eq!(
            rendered,
            "Provenance is already set up in /repo. No change.\n"
        );
    }

    #[test]
    fn the_already_ending_still_carries_pending_dictionary_guidance() {
        let ending = InitEnding::Already {
            line: "Provenance is already set up in /repo. No change.".to_owned(),
            dictionary: Some("Dictionary: the request page".to_owned()),
        };

        let rendered = render(|out| match &ending {
            InitEnding::Already { line, dictionary } => {
                writeln!(out, "{line}").unwrap();
                if let Some(dictionary) = dictionary {
                    writeln!(out).unwrap();
                    writeln!(out, "{dictionary}").unwrap();
                }
            }
            InitEnding::Applied(summary) => summary.write_to(out).unwrap(),
        });

        assert_eq!(
            rendered,
            "Provenance is already set up in /repo. No change.\n\nDictionary: the request page\n"
        );
    }
}
