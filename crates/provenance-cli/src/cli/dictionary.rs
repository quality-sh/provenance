use camino::Utf8PathBuf;

/// One `provenance dictionary` command and its paths.
pub enum DictionaryCommand {
    /// Import the ASD-STE100 Issue 9 dictionary from one local PDF file.
    Import { pdf: Utf8PathBuf, repo: Utf8PathBuf },
    /// Report the committed dictionary reference and the analyzer it selects.
    Status { repo: Utf8PathBuf },
}
