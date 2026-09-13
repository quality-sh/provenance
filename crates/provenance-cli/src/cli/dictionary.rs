use crate::output::JsonFormat;
use camino::Utf8PathBuf;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum DictionaryCommand {
    /// Import the ASD-STE100 Issue 9 dictionary from one local PDF file.
    Import {
        /// Path to the local official Issue 9 PDF file.
        #[arg(long)]
        pdf: Utf8PathBuf,
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long, value_enum, default_value_t = JsonFormat::Json)]
        format: JsonFormat,
    },
}
