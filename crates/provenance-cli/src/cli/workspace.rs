use camino::Utf8PathBuf;

/// One `provenance docs` command and its inputs.
pub enum DocsCommand {
    Serve {
        repo: Utf8PathBuf,
        host: String,
        port: u16,
    },
}
