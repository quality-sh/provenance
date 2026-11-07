//! Renders the result of a completed repository initialization.

use provenance_porcelain::init::InitReport;
use std::io::Write;

const INTRODUCTION: &str =
    "Provenance records requirements, decisions, and the rules that connect them to code.";
const AGENT_HANDOFF: &str = "Have your agent run provenance prime to get acclimated.";

#[derive(Debug)]
pub enum InitResult {
    Already(String),
    Applied(Box<InitSummary>),
}

/// The result is printed only after all owned project files are published.
#[derive(Debug)]
pub struct InitEnding {
    result: InitResult,
    notices: Vec<String>,
}

impl InitEnding {
    pub fn already(line: String, warning: Option<String>) -> Self {
        Self {
            result: InitResult::Already(line),
            notices: warning.into_iter().collect(),
        }
    }

    pub fn applied(summary: InitSummary, warning: Option<String>) -> Self {
        Self {
            result: InitResult::Applied(Box::new(summary)),
            notices: warning.into_iter().collect(),
        }
    }

    pub fn with_notice(mut self, notice: Option<String>) -> Self {
        self.notices.extend(notice);
        self
    }

    pub fn for_cargo(
        mut self,
        applied_status: String,
        no_change_status: String,
        changes: &[(String, bool)],
    ) -> Self {
        self.result = match self.result {
            InitResult::Already(_) if changes.is_empty() => InitResult::Already(no_change_status),
            InitResult::Already(_) => {
                InitResult::Applied(Box::new(InitSummary::new(applied_status)))
            }
            InitResult::Applied(mut summary) => {
                summary.status_line = applied_status;
                InitResult::Applied(summary)
            }
        };
        if let InitResult::Applied(summary) = &mut self.result {
            for (path, existed) in changes {
                if *existed {
                    summary.push_changed(path, "added the Provenance SDK dependency");
                } else {
                    summary.push_new(path, "created by Cargo");
                }
            }
        }
        self
    }

    /// The ending as the report of the init port.
    pub fn into_report(self) -> InitReport {
        let mut summary = Vec::new();
        self.write_to(&mut summary)
            .expect("a write to memory does not fail");
        InitReport {
            notices: self.notices,
            summary: String::from_utf8(summary).expect("the summary is UTF-8"),
        }
    }

    fn write_to(&self, out: &mut impl Write) -> std::io::Result<()> {
        match &self.result {
            InitResult::Already(line) => {
                writeln!(out, "{line}")?;
                writeln!(out)?;
                writeln!(out, "{INTRODUCTION}")?;
            }
            InitResult::Applied(summary) => summary.write_to(out)?,
        }
        writeln!(out)?;
        writeln!(out, "{AGENT_HANDOFF}")
    }
}

#[derive(Debug)]
pub struct InitSummary {
    status_line: String,
    new_files: Vec<InventoryEntry>,
    changed_files: Vec<InventoryEntry>,
}

#[derive(Debug)]
struct InventoryEntry {
    path: String,
    note: String,
}

impl InitSummary {
    pub const fn new(status_line: String) -> Self {
        Self {
            status_line,
            new_files: Vec::new(),
            changed_files: Vec::new(),
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

    fn write_to(&self, out: &mut impl Write) -> std::io::Result<()> {
        writeln!(out, "{}", self.status_line)?;
        writeln!(out)?;
        writeln!(out, "{INTRODUCTION}")?;
        writeln!(out)?;
        write_inventory(out, "New", &self.new_files)?;
        write_inventory(out, "Changed", &self.changed_files)
    }
}

fn write_inventory(
    out: &mut impl Write,
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

    #[test]
    fn completed_init_report_keeps_notices_apart_from_summary() {
        let report = InitEnding::already("No change.".to_owned(), Some("Warning: one".to_owned()))
            .into_report();
        assert_eq!(report.notices, ["Warning: one"]);
        assert!(report.summary.starts_with("No change.\n\n"));
        assert!(!report.summary.contains("Warning"));
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
}
