use crate::output::OutputFormat;
use camino::Utf8PathBuf;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum ThreadCommand {
    Post {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long)]
        scope: String,
        #[arg(long)]
        parent_type: String,
        #[arg(long)]
        parent_id: String,
        #[arg(long)]
        role: String,
        body: String,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    List {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long)]
        scope: String,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
}

#[derive(Subcommand)]
pub enum TopicsCommand {
    Create {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long)]
        scope: String,
        #[arg(long)]
        id: String,
        #[arg(long)]
        requirement_id: String,
        #[arg(long)]
        title: String,
        #[arg(long, default_value = "open")]
        status: String,
        #[arg(long, default_value = "[]")]
        links_json: String,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    List {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long)]
        scope: String,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    /// Claim a topic so concurrent sessions skip it. Claiming an
    /// already-claimed topic is an error showing who holds it.
    Claim {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long)]
        scope: String,
        #[arg(long)]
        id: String,
        /// Actor name recorded on the claim.
        #[arg(long)]
        actor: String,
        /// Repository-relative changed path known by the claiming caller.
        #[arg(long)]
        changed_path: Vec<String>,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    /// Release a claimed topic without closing it.
    Release {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long)]
        scope: String,
        #[arg(long)]
        id: String,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    /// Close a topic. Closing clears any claim on it.
    Close {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long)]
        scope: String,
        #[arg(long)]
        id: String,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
}

#[derive(Subcommand)]
pub enum QuestionsCommand {
    /// Create a question. A question should be resolvable in one agent
    /// session; otherwise it is fog or needs decomposition.
    Create {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long)]
        scope: String,
        #[arg(long)]
        id: String,
        #[arg(long)]
        topic_id: String,
        #[arg(long)]
        question: String,
        /// Resolution method: grill, prototype, research, verify, or task.
        #[arg(long)]
        method: String,
        #[arg(long, default_value = "open")]
        status: String,
        #[arg(long)]
        answer: Option<String>,
        #[arg(long, default_value = "[]")]
        links_json: String,
        #[arg(long)]
        resolution_id: Option<String>,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    List {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long)]
        scope: String,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    /// Update mutable question state after creation.
    Update {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long)]
        scope: String,
        #[arg(long)]
        id: String,
        /// Resolution method: grill, prototype, research, verify, or task.
        #[arg(long)]
        method: Option<String>,
        /// Status: open, `blocked_on_human`, or answered. Hyphens are accepted.
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        links_json: Option<String>,
        #[arg(long)]
        resolution_id: Option<String>,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    /// Claim a question so concurrent sessions skip it. Claiming an
    /// already-claimed question is an error showing who holds it.
    Claim {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long)]
        scope: String,
        #[arg(long)]
        id: String,
        /// Actor name recorded on the claim.
        #[arg(long)]
        actor: String,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    /// Release a claimed question without answering it.
    Release {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long)]
        scope: String,
        #[arg(long)]
        id: String,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    /// Record the answer to a question. Answering clears any claim on it.
    Answer {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long)]
        scope: String,
        #[arg(long)]
        id: String,
        #[arg(long)]
        answer: String,
        #[arg(long)]
        resolution_id: Option<String>,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
}
