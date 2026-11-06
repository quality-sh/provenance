//! The CLI flags of a command, and the relations between them.

/// A relation between two CLI flags of one command, by input field name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Relation {
    /// The first flag needs the second flag.
    Requires(&'static str, &'static str),
    /// The two flags cannot be given together.
    Conflicts(&'static str, &'static str),
}

/// How a CLI flag takes its value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlagKind {
    /// `--name` without a value.
    Switch,
    /// `--name <value>`, at most once.
    Value,
    /// `--name <value>`, zero or more times.
    Repeated,
    /// A value by position.
    Positional,
}

/// The value type of a CLI flag, so the CLI refuses a bad value before the command runs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlagValue {
    Text,
    /// A whole number from zero.
    Count,
    /// One record kind.
    RecordKind,
    /// One api method.
    ApiMethod,
    /// An api catalog page size from 1 through 200.
    CatalogLimit,
    /// A TCP port number.
    Port,
}

/// One CLI flag of a command, in the order that help shows it.
///
/// The JSON Schema of a typed input does not keep field order or the CLI-only facts
/// below, so each command declares its flags here.
// Each bool is one independent CLI fact of the flag.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Flag {
    /// The CLI flag name, without `--`.
    pub name: &'static str,
    /// The input field, when it is not the name with `_` for `-`.
    pub field_name: Option<&'static str>,
    pub kind: FlagKind,
    pub value: FlagValue,
    pub help: Option<&'static str>,
    pub default: Option<&'static str>,
    /// The accepted values, when the flag takes one of a fixed set.
    pub values: &'static [&'static str],
    /// The value can start with `-`.
    pub hyphen_values: bool,
    pub hidden: bool,
    /// The command refuses to run without this flag.
    pub required: bool,
    /// The subcommands after this flag also take it.
    pub global: bool,
}

impl Flag {
    /// A flag with no help, default, or value set.
    pub const fn new(name: &'static str, kind: FlagKind) -> Self {
        Self {
            name,
            field_name: None,
            kind,
            value: FlagValue::Text,
            help: None,
            default: None,
            values: &[],
            hyphen_values: false,
            hidden: false,
            required: false,
            global: false,
        }
    }

    #[must_use]
    pub const fn hidden(mut self) -> Self {
        self.hidden = true;
        self
    }

    #[must_use]
    pub const fn required(mut self) -> Self {
        self.required = true;
        self
    }

    #[must_use]
    pub const fn global(mut self) -> Self {
        self.global = true;
        self
    }

    #[must_use]
    pub const fn value(mut self, value: FlagValue) -> Self {
        self.value = value;
        self
    }

    #[must_use]
    pub const fn help(mut self, help: &'static str) -> Self {
        self.help = Some(help);
        self
    }

    #[must_use]
    pub const fn default(mut self, default: &'static str) -> Self {
        self.default = Some(default);
        self
    }

    #[must_use]
    pub const fn values(mut self, values: &'static [&'static str]) -> Self {
        self.values = values;
        self
    }

    #[must_use]
    pub const fn hyphen_values(mut self) -> Self {
        self.hyphen_values = true;
        self
    }

    #[must_use]
    pub const fn field(mut self, field: &'static str) -> Self {
        self.field_name = Some(field);
        self
    }

    /// The input field name. It is also the CLI argument ID.
    pub fn field_id(&self) -> String {
        self.field_name
            .map_or_else(|| self.name.replace('-', "_"), str::to_owned)
    }
}

pub const REPO: Flag = Flag::new("repo", FlagKind::Value).default(".");
pub const SCOPE: Flag = Flag::new("scope", FlagKind::Value).default("default");

/// The description of each CLI command group that has one, by its words.
pub const GROUP_DESCRIPTIONS: &[(&str, &str)] = &[
    (
        "report",
        "Deterministic report rendering from a versioned report envelope",
    ),
    (
        "dogfood",
        "Dev-build-only: record pain-point notes about provenance itself",
    ),
    (
        "dogfood triage",
        "Dev-build-only triage of captured notes: mark them handled or reopen them. State \
         lives beside the spool; the capture history is never rewritten",
    ),
];

/// The repository options that the porcelain commands and the commands after them take.
pub const GLOBAL_FLAGS: &[Flag] = &[
    REPO.hyphen_values().global(),
    SCOPE.hyphen_values().global(),
    json_format_flag(None).global(),
    Flag::new("quiet", FlagKind::Switch).global(),
];

/// The `--format` flag of a command that renders only JSON.
pub const fn json_format_flag(default: Option<&'static str>) -> Flag {
    let flag = Flag::new("format", FlagKind::Value).values(&["json"]);
    match default {
        Some(default) => flag.default(default),
        None => flag,
    }
}
