use crate::output::OutputFormat;
use camino::Utf8PathBuf;
use clap::Args;

#[derive(Args)]
pub struct UpdateArgs {
    #[arg(long, default_value = ".")]
    pub repo: Utf8PathBuf,
    #[arg(long)]
    pub scope: String,
    #[arg(long)]
    pub id: String,
    /// Changed fields as JSON or @file. Use `clear_fields` to clear nullable fields.
    #[arg(long)]
    pub fields_json: String,
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}
