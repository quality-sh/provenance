//! The fields of a command input, and the relations between them.
//!
//! A field has one name in `snake_case`. Each consumer shows the field under that name,
//! spelled as its own standard asks.

/// The value type of a field, so a consumer refuses a bad value before the command runs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FieldValue {
    /// True when given, false when not.
    Switch,
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
    /// Name and value pairs, each given as the name, this separator, and the value.
    Pairs(char),
    /// One JSON object.
    Object,
}

/// One field of a command input, in the order that the command documents it.
///
/// The JSON Schema of a typed input does not keep field order or the facts below, so
/// each command declares its fields here.
// Each bool is one independent fact of the field.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Field {
    /// The field name, in `snake_case`.
    pub name: &'static str,
    pub value: FieldValue,
    /// The field takes zero or more values.
    pub repeated: bool,
    pub doc: Option<&'static str>,
    pub default: Option<&'static str>,
    /// The accepted values, when the field takes one of a fixed set.
    pub values: &'static [&'static str],
    /// The value can start with a hyphen.
    pub leading_hyphen: bool,
    /// Programs give this field, so a consumer does not document it for people.
    pub internal: bool,
    /// The command refuses to run without this field.
    pub required: bool,
    /// Giving this field changes state outside the records that the command reads.
    pub writes: bool,
}

impl Field {
    const fn new(name: &'static str, value: FieldValue, repeated: bool) -> Self {
        Self {
            name,
            value,
            repeated,
            doc: None,
            default: None,
            values: &[],
            leading_hyphen: false,
            internal: false,
            required: false,
            writes: false,
        }
    }

    /// A field that is true when given.
    pub const fn switch(name: &'static str) -> Self {
        Self::new(name, FieldValue::Switch, false)
    }

    /// A field with one text value.
    pub const fn text(name: &'static str) -> Self {
        Self::new(name, FieldValue::Text, false)
    }

    /// A field with zero or more text values.
    pub const fn repeated(name: &'static str) -> Self {
        Self::new(name, FieldValue::Text, true)
    }

    #[must_use]
    pub const fn of(mut self, value: FieldValue) -> Self {
        self.value = value;
        self
    }

    #[must_use]
    pub const fn doc(mut self, doc: &'static str) -> Self {
        self.doc = Some(doc);
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
    pub const fn leading_hyphen(mut self) -> Self {
        self.leading_hyphen = true;
        self
    }

    #[must_use]
    pub const fn internal(mut self) -> Self {
        self.internal = true;
        self
    }

    #[must_use]
    pub const fn writes(mut self) -> Self {
        self.writes = true;
        self
    }

    #[must_use]
    pub const fn required(mut self) -> Self {
        self.required = true;
        self
    }
}

/// A relation between two fields of one command, by field name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Relation {
    /// The first field needs the second field.
    Requires(&'static str, &'static str),
    /// The two fields cannot be given together.
    Conflicts(&'static str, &'static str),
}

/// The input of a command as fields.
///
/// It holds their order, the fields that name the target of the command, the relations
/// between them, and the schema of the typed input that the fields make, when the
/// command reads one.
#[derive(Clone, Copy, Debug)]
pub struct Fields {
    pub fields: &'static [Field],
    /// The fields that name what the command acts on, in order.
    pub target: &'static [&'static str],
    pub relations: &'static [Relation],
    /// The JSON Schema of the typed input. It has one property for each field.
    pub schema: Option<fn() -> serde_json::Value>,
}

impl Fields {
    /// These fields of a typed input, with no target fields and no relations.
    pub const fn new(fields: &'static [Field], schema: fn() -> serde_json::Value) -> Self {
        Self {
            fields,
            target: &[],
            relations: &[],
            schema: Some(schema),
        }
    }

    /// These fields, read one by one, with no target fields and no relations.
    pub const fn untyped(fields: &'static [Field]) -> Self {
        Self {
            fields,
            target: &[],
            relations: &[],
            schema: None,
        }
    }

    #[must_use]
    pub const fn target(mut self, target: &'static [&'static str]) -> Self {
        self.target = target;
        self
    }

    #[must_use]
    pub const fn relations(mut self, relations: &'static [Relation]) -> Self {
        self.relations = relations;
        self
    }

    /// Whether the field names what the command acts on.
    pub fn is_target(&self, field: &Field) -> bool {
        self.target.contains(&field.name)
    }
}

/// The repository root that a command opens.
pub const REPO: Field = Field::text("repo").default(".");
/// The scope that a command opens.
pub const SCOPE: Field = Field::text("scope").default("default");

/// The `format` field of a command whose result is only JSON.
pub const fn json_format(default: Option<&'static str>) -> Field {
    let field = Field::text("format").values(&["json"]);
    match default {
        Some(default) => field.default(default),
        None => field,
    }
}
