use serde::Serialize;

/// The one format a command without a human renderer renders. The flag
/// stays, so scripts that pass `--format json` keep working.
#[derive(Debug, Clone, Copy)]
pub enum JsonFormat {
    Json,
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
