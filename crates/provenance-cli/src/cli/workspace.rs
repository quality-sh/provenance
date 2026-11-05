use camino::Utf8PathBuf;

/// One `provenance docs` command and its inputs.
pub enum DocsCommand {
    Check {
        repo: Utf8PathBuf,
    },
    Serve {
        repo: Utf8PathBuf,
        host: String,
        port: u16,
    },
}

/// One `provenance skills` command and its inputs.
pub enum SkillsCommand {
    List,
    Show {
        name: String,
    },
    Install {
        global: bool,
        copy: bool,
        force: bool,
    },
}
