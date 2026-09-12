use serde::Serialize;

/// Every output format any command renders. Only commands with a real
/// renderer for each variant accept the full set: `export`, `wiki build`,
/// and `report render`.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum OutputFormat {
    Json,
    Jsonl,
    Markdown,
    Table,
    Toon,
}

/// The one format a command without a human renderer renders. The flag
/// stays, so scripts that pass `--format json` keep working.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum JsonFormat {
    Json,
}

/// JSON for the machine, or one command's Markdown report for the reader.
/// The Markdown arm names a real renderer.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum ReportFormat {
    Json,
    Markdown,
}

/// Pretty-prints one JSON document as the command's whole answer.
pub fn print_json<T: Serialize>(value: &T) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
