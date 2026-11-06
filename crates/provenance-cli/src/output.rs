use serde::Serialize;

/// Every output format any command renders. Only `export` has a real
/// renderer for each variant and accepts the full set.
#[derive(Debug, Clone, Copy)]
pub enum OutputFormat {
    Json,
    Jsonl,
    Markdown,
    Table,
    Toon,
}

/// The one format a command without a human renderer renders. The flag
/// stays, so scripts that pass `--format json` keep working.
#[derive(Debug, Clone, Copy)]
pub enum JsonFormat {
    Json,
}

pub use provenance_transport::coverage::ReportFormat;

impl OutputFormat {
    /// The format that one `--format` value names.
    pub fn from_flag(value: &str) -> Option<Self> {
        Some(match value {
            "json" => Self::Json,
            "jsonl" => Self::Jsonl,
            "markdown" => Self::Markdown,
            "table" => Self::Table,
            "toon" => Self::Toon,
            _ => return None,
        })
    }
}

impl JsonFormat {
    /// The format that one `--format` value names.
    pub fn from_flag(value: &str) -> Option<Self> {
        (value == "json").then_some(Self::Json)
    }
}

/// Pretty-prints one JSON document as the command's whole answer.
pub fn print_json<T: Serialize>(value: &T) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
